//! 本文件实现统一 Completion 抽象、Ollama 本地 LLM、流式输出以及云端 Provider。
use std::{future::Future, pin::Pin};

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

/// 表示 Completion 当前执行的生成任务类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionTask {
    /// 根据检索片段回答问题。
    Answer,
    /// 从选定记忆中生成知识卡片。
    Card,
}

/// 表示传给 Completion 的一条最小化上下文片段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionContext {
    /// 提示词内用于引用的稳定标签，例如 `[1]`。
    pub label: String,
    /// 片段来源标题；无标题时为空。
    pub title: String,
    /// 本地检索命中的必要文本，不包含整库或 API Key。
    pub text: String,
}

/// 表示一次问答或卡片生成请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// 任务类别。
    pub task: CompletionTask,
    /// 定义回答边界与输出格式的系统提示。
    pub system: String,
    /// 用户问题或生成指令。
    pub prompt: String,
    /// 已在本地筛选并截断的最小上下文。
    pub context: Vec<CompletionContext>,
    /// 最大输出 token 数。
    pub max_tokens: u32,
    /// 生成温度。
    pub temperature: f32,
}

/// 表示 Completion Provider 返回的完整文本和实际 Provider 标识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Provider 生成或本地抽取的文本。
    pub text: String,
    /// 实际完成请求的 Provider 标识。
    pub provider: String,
}

/// 表示 Provider 逐段返回的增量文本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionDelta {
    /// 本次新到达的文本，不重复此前已发送的内容。
    pub text: String,
    /// 产生该增量的 Provider 标识。
    pub provider: String,
}

/// 表示 Completion 配置、网络调用或响应解析错误。
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// Provider 配置缺少必要字段或端点协议不安全。
    #[error("Completion Provider 配置无效: {0}")]
    InvalidConfiguration(String),
    /// 请求未能到达 Provider。
    #[error("Completion Provider 请求失败: {0}")]
    Request(String),
    /// Provider 返回非成功状态；正文经过长度限制且不包含请求密钥。
    #[error("Completion Provider 返回 HTTP {status}: {message}")]
    Remote {
        /// HTTP 状态码。
        status: u16,
        /// 截断后的远程错误正文。
        message: String,
    },
    /// Provider 成功响应不包含预期文本。
    #[error("Completion Provider 响应无效: {0}")]
    InvalidResponse(String),
}

/// 表示对象安全 Completion trait 返回的完整异步结果。
pub type CompletionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CompletionResponse, CompletionError>> + Send + 'a>>;

/// 表示对象安全 Completion trait 返回的逐段异步结果。
pub type CompletionStream<'a> =
    Pin<Box<dyn Stream<Item = Result<CompletionDelta, CompletionError>> + Send + 'a>>;

/// 抽象本地与远程问答、总结和卡片生成 Provider。
pub trait Completion: Send + Sync {
    /// 执行一次完整文本生成。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a>;

    /// 执行逐段文本生成；未覆盖流式实现的 Provider 会以一个完整增量安全回退。
    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        Box::pin(async_stream::try_stream! {
            let response = self.complete(request).await?;
            yield CompletionDelta {
                text: response.text,
                provider: response.provider,
            };
        })
    }

    /// 返回 UI 和审计信息使用的稳定 Provider 标识。
    fn provider_name(&self) -> &str;

    /// 表示该实现是否会把最小上下文发送到远程服务。
    fn sends_data_remote(&self) -> bool;
}

/// 提供无需模型和网络的本地抽取式问答与单卡生成回退。
#[derive(Debug, Default)]
pub struct LocalExtractiveCompletion;

impl Completion for LocalExtractiveCompletion {
    /// 仅重组调用方已筛选的片段，不访问网络或本地整库。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let text = match request.task {
                CompletionTask::Answer => local_answer(&request),
                CompletionTask::Card => local_card(&request)?,
            };
            Ok(CompletionResponse {
                text,
                provider: self.provider_name().into(),
            })
        })
    }

    fn provider_name(&self) -> &str {
        "local"
    }

    fn sends_data_remote(&self) -> bool {
        false
    }
}

/// 调用本机 Ollama `/api/chat` 的本地 LLM Provider。
pub struct OllamaCompletion {
    client: reqwest::Client,
    model: String,
    endpoint: String,
}

impl OllamaCompletion {
    /// 使用本机 Ollama 模型和可选回环端点创建 Provider，默认连接 `127.0.0.1:11434`。
    pub fn new(
        model: impl Into<String>,
        endpoint: Option<String>,
    ) -> Result<Self, CompletionError> {
        let endpoint = normalized_ollama_endpoint(
            endpoint.unwrap_or_else(|| "http://127.0.0.1:11434/api/chat".into()),
        );
        validate_local_endpoint(&endpoint)?;
        Ok(Self {
            client: reqwest::Client::new(),
            model: required_value(model.into(), "Ollama 模型")?,
            endpoint,
        })
    }

    /// 构造 Ollama 所需的本地请求体，系统提示与最小上下文保持和云端一致。
    fn request_body(&self, request: &CompletionRequest, stream: bool) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": render_user_content(request)}
            ],
            "stream": stream,
            "options": {"temperature": request.temperature.clamp(0.0, 2.0)}
        })
    }
}

impl Completion for OllamaCompletion {
    /// 请求 Ollama 的非流式接口，用于卡片生成和不启用实时输出的问答。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(&self.endpoint)
                .json(&self.request_body(&request, false))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let payload = checked_json(response).await?;
            let text = payload["message"]["content"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CompletionError::InvalidResponse("Ollama 未返回 message.content".into())
                })?;
            Ok(CompletionResponse {
                text: text.into(),
                provider: self.provider_name().into(),
            })
        })
    }

    /// 读取 Ollama 的 NDJSON 增量响应并将每段 `message.content` 立刻转发。
    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        Box::pin(async_stream::try_stream! {
            let response = self
                .client
                .post(&self.endpoint)
                .json(&self.request_body(&request, true))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let response = checked_stream_response(response).await?;
            let mut bytes = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|error| CompletionError::Request(error.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(line) = take_json_line(&mut buffer) {
                    let payload: serde_json::Value = serde_json::from_str(&line)
                        .map_err(|error| CompletionError::InvalidResponse(error.to_string()))?;
                    if let Some(message) = payload["error"].as_str() {
                        Err(CompletionError::InvalidResponse(compact_text(message, 600)))?;
                    }
                    if let Some(text) = payload["message"]["content"].as_str().filter(|text| !text.is_empty()) {
                        yield CompletionDelta { text: text.into(), provider: self.provider_name().into() };
                    }
                }
            }
            if !buffer.trim().is_empty() {
                let payload: serde_json::Value = serde_json::from_str(buffer.trim())
                    .map_err(|error| CompletionError::InvalidResponse(error.to_string()))?;
                if let Some(text) = payload["message"]["content"].as_str().filter(|text| !text.is_empty()) {
                    yield CompletionDelta { text: text.into(), provider: self.provider_name().into() };
                }
            }
        })
    }

    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn sends_data_remote(&self) -> bool {
        false
    }
}

/// 调用 OpenAI Chat Completions API 的远程 Provider。
pub struct OpenAiCompletion {
    inner: OpenAiCompatibleCompletion,
}

impl OpenAiCompletion {
    /// 使用自带 Key、模型和可选完整 Chat Completions 地址创建 Provider。
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        endpoint: Option<String>,
    ) -> Result<Self, CompletionError> {
        Ok(Self {
            inner: OpenAiCompatibleCompletion::new(
                "openai",
                api_key.into(),
                model.into(),
                endpoint.unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".into()),
                false,
            )?,
        })
    }
}

impl Completion for OpenAiCompletion {
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        self.inner.complete(request)
    }

    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        self.inner.stream(request)
    }

    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn sends_data_remote(&self) -> bool {
        self.inner.sends_data_remote()
    }
}

/// 调用 OpenAI Chat Completions 兼容自定义端点的 Provider，也可连接本机 LM Studio 等服务。
pub struct CustomCompletion {
    inner: OpenAiCompatibleCompletion,
}

impl CustomCompletion {
    /// 使用完整端点或 API 根地址创建自定义 Provider；回环地址允许无 Key。
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, CompletionError> {
        let endpoint = normalized_chat_completions_endpoint(endpoint.into());
        Ok(Self {
            inner: OpenAiCompatibleCompletion::new(
                "custom",
                api_key.into(),
                model.into(),
                endpoint,
                true,
            )?,
        })
    }
}

impl Completion for CustomCompletion {
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        self.inner.complete(request)
    }

    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        self.inner.stream(request)
    }

    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn sends_data_remote(&self) -> bool {
        self.inner.sends_data_remote()
    }
}

/// 调用 Anthropic Messages API 的 Claude Completion Provider。
pub struct AnthropicCompletion {
    client: reqwest::Client,
    api_key: String,
    model: String,
    endpoint: String,
}

impl AnthropicCompletion {
    /// 使用自带 Key、模型和可选 Messages API 地址创建 Provider。
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        endpoint: Option<String>,
    ) -> Result<Self, CompletionError> {
        let api_key = required_value(api_key.into(), "Claude API Key")?;
        let model = required_value(model.into(), "Claude 模型")?;
        let endpoint = endpoint.unwrap_or_else(|| "https://api.anthropic.com/v1/messages".into());
        validate_remote_endpoint(&endpoint, false)?;
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            endpoint,
        })
    }

    /// 构造 Claude Messages API 请求体，避免流式与非流式分支出现提示词差异。
    fn request_body(&self, request: &CompletionRequest, stream: bool) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "system": request.system,
            "messages": [{"role": "user", "content": render_user_content(request)}],
            "max_tokens": request.max_tokens.clamp(1, 8_192),
            "temperature": request.temperature.clamp(0.0, 2.0),
            "stream": stream,
        })
    }
}

impl Completion for AnthropicCompletion {
    /// 调用 Claude 的非流式接口并提取第一段文本内容。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(&self.endpoint)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&self.request_body(&request, false))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let payload = checked_json(response).await?;
            let text = payload["content"]
                .as_array()
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        (part["type"] == "text")
                            .then(|| part["text"].as_str())
                            .flatten()
                    })
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CompletionError::InvalidResponse("Claude content 未包含文本".into())
                })?;
            Ok(CompletionResponse {
                text: text.into(),
                provider: self.provider_name().into(),
            })
        })
    }

    /// 解析 Claude SSE 的 `content_block_delta.delta.text` 并逐段输出。
    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        Box::pin(async_stream::try_stream! {
            let response = self
                .client
                .post(&self.endpoint)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&self.request_body(&request, true))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let response = checked_stream_response(response).await?;
            let mut bytes = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|error| CompletionError::Request(error.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(data) = take_sse_data(&mut buffer) {
                    let payload: serde_json::Value = serde_json::from_str(&data)
                        .map_err(|error| CompletionError::InvalidResponse(error.to_string()))?;
                    if let Some(message) = payload["error"]["message"].as_str() {
                        Err(CompletionError::InvalidResponse(compact_text(message, 600)))?;
                    }
                    if let Some(text) = payload["delta"]["text"].as_str().filter(|text| !text.is_empty()) {
                        yield CompletionDelta { text: text.into(), provider: self.provider_name().into() };
                    }
                }
            }
        })
    }

    fn provider_name(&self) -> &str {
        "claude"
    }

    fn sends_data_remote(&self) -> bool {
        true
    }
}

/// 保存 OpenAI Chat Completions 兼容请求的共用实现。
struct OpenAiCompatibleCompletion {
    client: reqwest::Client,
    provider: &'static str,
    api_key: String,
    model: String,
    endpoint: String,
    sends_data_remote: bool,
}

impl OpenAiCompatibleCompletion {
    /// 校验敏感配置但不打印或序列化 API Key；本地兼容端点允许无 Key。
    fn new(
        provider: &'static str,
        api_key: String,
        model: String,
        endpoint: String,
        allow_loopback: bool,
    ) -> Result<Self, CompletionError> {
        validate_remote_endpoint(&endpoint, allow_loopback)?;
        let is_local = is_loopback_endpoint(&endpoint);
        let api_key = if is_local {
            api_key.trim().to_owned()
        } else {
            required_value(api_key, "API Key")?
        };
        Ok(Self {
            client: reqwest::Client::new(),
            provider,
            api_key,
            model: required_value(model, "模型")?,
            endpoint,
            sends_data_remote: !is_local,
        })
    }

    /// 构造 OpenAI-compatible 请求体，统一支持标准与流式 Chat Completions。
    fn request_body(&self, request: &CompletionRequest, stream: bool) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": render_user_content(request)}
            ],
            "max_tokens": request.max_tokens.clamp(1, 8_192),
            "temperature": request.temperature.clamp(0.0, 2.0),
            "stream": stream,
        })
    }

    /// 在配置了 Key 时附加 Bearer 认证；本地兼容服务无需伪造空凭据。
    fn authenticated_request(&self) -> reqwest::RequestBuilder {
        let request = self.client.post(&self.endpoint);
        if self.api_key.is_empty() {
            request
        } else {
            request.bearer_auth(&self.api_key)
        }
    }
}

impl Completion for OpenAiCompatibleCompletion {
    /// 调用非流式 Chat Completions 并读取首个 choice 的文本。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let response = self
                .authenticated_request()
                .json(&self.request_body(&request, false))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let payload = checked_json(response).await?;
            let text = payload["choices"][0]["message"]["content"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    CompletionError::InvalidResponse("Chat Completions choices 未包含文本".into())
                })?;
            Ok(CompletionResponse {
                text: text.into(),
                provider: self.provider.into(),
            })
        })
    }

    /// 解析 OpenAI-compatible SSE 的 `choices[0].delta.content` 并实时转发。
    fn stream<'a>(&'a self, request: CompletionRequest) -> CompletionStream<'a> {
        Box::pin(async_stream::try_stream! {
            let response = self
                .authenticated_request()
                .json(&self.request_body(&request, true))
                .send()
                .await
                .map_err(|error| CompletionError::Request(error.to_string()))?;
            let response = checked_stream_response(response).await?;
            let mut bytes = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|error| CompletionError::Request(error.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(data) = take_sse_data(&mut buffer) {
                    if data == "[DONE]" {
                        return;
                    }
                    let payload: serde_json::Value = serde_json::from_str(&data)
                        .map_err(|error| CompletionError::InvalidResponse(error.to_string()))?;
                    if let Some(message) = payload["error"]["message"].as_str() {
                        Err(CompletionError::InvalidResponse(compact_text(message, 600)))?;
                    }
                    if let Some(text) = payload["choices"][0]["delta"]["content"].as_str().filter(|text| !text.is_empty()) {
                        yield CompletionDelta { text: text.into(), provider: self.provider.into() };
                    }
                }
            }
        })
    }

    fn provider_name(&self) -> &str {
        self.provider
    }

    fn sends_data_remote(&self) -> bool {
        self.sends_data_remote
    }
}

/// 从检索片段生成不引入外部事实的本地回答。
fn local_answer(request: &CompletionRequest) -> String {
    if request.context.is_empty() {
        return "没有在当前范围内找到足够的相关记忆。".into();
    }
    let evidence = request
        .context
        .iter()
        .map(|context| {
            let text = compact_text(&context.text, 420);
            format!("- {text} {}", context.label)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("根据当前检索到的记忆：\n{evidence}")
}

/// 从用户选中的单条来源记忆生成一张可编辑的本地抽取式卡片。
fn local_card(request: &CompletionRequest) -> Result<String, CompletionError> {
    let context = request
        .context
        .first()
        .ok_or_else(|| CompletionError::InvalidResponse("生成卡片需要至少一条来源上下文".into()))?;
    let title = context.title.trim();
    let front = if title.is_empty() {
        "这段记忆的核心要点是什么？".to_owned()
    } else {
        format!("「{title}」的核心要点是什么？")
    };
    let back = compact_text(&context.text, 560);
    if back.is_empty() {
        return Err(CompletionError::InvalidResponse(
            "来源记忆没有可用于卡片的文本".into(),
        ));
    }
    serde_json::to_string(&[serde_json::json!({
        "card_front": front,
        "card_back": back,
    })])
    .map_err(|error| CompletionError::InvalidResponse(error.to_string()))
}

/// 只渲染已经由调用方最小化的上下文，不读取额外数据。
fn render_user_content(request: &CompletionRequest) -> String {
    let context = request
        .context
        .iter()
        .map(|item| format!("{} {}\n{}", item.label, item.title.trim(), item.text.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{}\n\n可用上下文：\n{}", request.prompt.trim(), context)
}

/// 检查远程状态并把成功正文解析为 JSON。
async fn checked_json(response: reqwest::Response) -> Result<serde_json::Value, CompletionError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| CompletionError::Request(error.to_string()))?;
    if !status.is_success() {
        return Err(CompletionError::Remote {
            status: status.as_u16(),
            message: compact_text(&body, 600),
        });
    }
    serde_json::from_str(&body).map_err(|error| CompletionError::InvalidResponse(error.to_string()))
}

/// 在开始读取增量正文前验证 HTTP 状态，错误正文不会泄露请求凭据。
async fn checked_stream_response(
    response: reqwest::Response,
) -> Result<reqwest::Response, CompletionError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response
        .text()
        .await
        .map_err(|error| CompletionError::Request(error.to_string()))?;
    Err(CompletionError::Remote {
        status: status.as_u16(),
        message: compact_text(&body, 600),
    })
}

/// 从累积缓冲区取出一帧 SSE 的全部 data 字段，保留不完整数据直到下一次读取。
fn take_sse_data(buffer: &mut String) -> Option<String> {
    let separator = buffer
        .find("\r\n\r\n")
        .map(|index| (index, 4))
        .or_else(|| buffer.find("\n\n").map(|index| (index, 2)))?;
    let frame = buffer
        .drain(..separator.0 + separator.1)
        .collect::<String>();
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    (!data.is_empty()).then_some(data)
}

/// 从 Ollama NDJSON 缓冲区取出一行完整 JSON；结尾残片保留给下一次网络读取。
fn take_json_line(buffer: &mut String) -> Option<String> {
    let index = buffer.find('\n')?;
    let line = buffer.drain(..=index).collect::<String>();
    let line = line.trim();
    (!line.is_empty()).then_some(line.into())
}

/// 校验字符串配置并去除首尾空白。
fn required_value(value: String, label: &str) -> Result<String, CompletionError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(CompletionError::InvalidConfiguration(format!(
            "{label} 不能为空"
        )));
    }
    Ok(value)
}

/// 判断端点是否严格指向回环地址，防止本地 Provider 将记忆文本意外外发。
fn is_loopback_endpoint(endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint.trim()) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
        )
}

/// 云 Provider 只接受 HTTPS；自定义端点额外允许回环 HTTP，便于连接本地模型服务。
fn validate_remote_endpoint(endpoint: &str, allow_loopback: bool) -> Result<(), CompletionError> {
    let endpoint = endpoint.trim();
    if endpoint.starts_with("https://") || (allow_loopback && is_loopback_endpoint(endpoint)) {
        return Ok(());
    }
    Err(CompletionError::InvalidConfiguration(
        "远程端点必须使用 HTTPS；自定义本地模型可使用回环 HTTP".into(),
    ))
}

/// Ollama 仅允许回环端点，保证“本地 LLM”不会将上下文发送到网络。
fn validate_local_endpoint(endpoint: &str) -> Result<(), CompletionError> {
    if is_loopback_endpoint(endpoint) {
        Ok(())
    } else {
        Err(CompletionError::InvalidConfiguration(
            "Ollama 仅允许 127.0.0.1、localhost 或 [::1] 回环端点".into(),
        ))
    }
}

/// 把自定义 API 根地址规范化为 OpenAI-compatible Chat Completions 路径。
fn normalized_chat_completions_endpoint(endpoint: String) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.ends_with("/chat/completions") {
        endpoint.into()
    } else {
        format!("{endpoint}/chat/completions")
    }
}

/// 把 Ollama 根地址规范化为 `/api/chat` 路径。
fn normalized_ollama_endpoint(endpoint: String) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.ends_with("/api/chat") {
        endpoint.into()
    } else {
        format!("{endpoint}/api/chat")
    }
}

/// 压平空白并按 Unicode 字符数截断文本，避免错误正文或片段无限膨胀。
fn compact_text(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        let mut shortened = normalized.chars().take(max_chars).collect::<String>();
        shortened.push('…');
        shortened
    }
}
