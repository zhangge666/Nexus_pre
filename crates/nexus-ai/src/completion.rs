//! 本文件实现统一 Completion 抽象、本地抽取式回退以及 Claude、OpenAI、自定义端点 Provider。

use std::{future::Future, pin::Pin};

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

/// 表示 Completion Provider 返回的文本和实际 Provider 标识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Provider 生成或本地抽取的文本。
    pub text: String,
    /// 实际完成请求的 Provider 标识。
    pub provider: String,
}

/// 表示 Completion 配置、网络调用或响应解析错误。
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// Provider 配置缺失必要字段或端点协议不安全。
    #[error("Completion Provider 配置无效: {0}")]
    InvalidConfiguration(String),
    /// 请求未能到达远程 Provider。
    #[error("Completion Provider 请求失败: {0}")]
    Request(String),
    /// 远程 Provider 返回非成功状态；正文经过长度限制且不会包含请求密钥。
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

/// 表示对象安全 Completion trait 返回的异步结果。
pub type CompletionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CompletionResponse, CompletionError>> + Send + 'a>>;

/// 抽象本地与远程问答、总结和卡片生成 Provider。
pub trait Completion: Send + Sync {
    /// 执行一次完整文本生成。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a>;

    /// 返回 UI 和审计信息使用的稳定 Provider 标识。
    fn provider_name(&self) -> &str;

    /// 表示该实现是否会把请求中的最小上下文发送到远程服务。
    fn sends_data_remote(&self) -> bool;
}

/// 提供无需模型和网络的本地抽取式问答与单卡生成回退。
#[derive(Debug, Default)]
pub struct LocalExtractiveCompletion;

impl Completion for LocalExtractiveCompletion {
    /// 仅重组调用方已经筛选的片段，不访问网络或本地整库。
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

    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn sends_data_remote(&self) -> bool {
        true
    }
}

/// 调用 OpenAI Chat Completions 兼容自定义端点的远程 Provider。
pub struct CustomCompletion {
    inner: OpenAiCompatibleCompletion,
}

impl CustomCompletion {
    /// 使用完整端点或 API 根地址创建自定义 Provider。
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

    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn sends_data_remote(&self) -> bool {
        true
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
}

impl Completion for AnthropicCompletion {
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(&self.endpoint)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&serde_json::json!({
                    "model": self.model,
                    "system": request.system,
                    "messages": [{"role": "user", "content": render_user_content(&request)}],
                    "max_tokens": request.max_tokens.clamp(1, 8_192),
                    "temperature": request.temperature.clamp(0.0, 2.0),
                }))
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
}

impl OpenAiCompatibleCompletion {
    /// 校验敏感配置但不打印或序列化 API Key。
    fn new(
        provider: &'static str,
        api_key: String,
        model: String,
        endpoint: String,
        allow_loopback: bool,
    ) -> Result<Self, CompletionError> {
        let api_key = required_value(api_key, "API Key")?;
        let model = required_value(model, "模型")?;
        validate_remote_endpoint(&endpoint, allow_loopback)?;
        Ok(Self {
            client: reqwest::Client::new(),
            provider,
            api_key,
            model,
            endpoint,
        })
    }
}

impl Completion for OpenAiCompatibleCompletion {
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(&serde_json::json!({
                    "model": self.model,
                    "messages": [
                        {"role": "system", "content": request.system},
                        {"role": "user", "content": render_user_content(&request)}
                    ],
                    "max_tokens": request.max_tokens.clamp(1, 8_192),
                    "temperature": request.temperature.clamp(0.0, 2.0),
                }))
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

    fn provider_name(&self) -> &str {
        self.provider
    }

    fn sends_data_remote(&self) -> bool {
        true
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

/// 从用户选中的单条来源生成一张可编辑的本地抽取式卡片。
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

/// 检查远程状态并把成功正文解析成 JSON。
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

/// 云 Provider 只接受 HTTPS；自定义端点另外允许本机 HTTP，便于连接本地模型服务。
fn validate_remote_endpoint(endpoint: &str, allow_loopback: bool) -> Result<(), CompletionError> {
    let endpoint = endpoint.trim();
    let is_https = endpoint.starts_with("https://");
    let is_loopback = allow_loopback
        && (endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:"));
    if !is_https && !is_loopback {
        return Err(CompletionError::InvalidConfiguration(
            "远程端点必须使用 HTTPS；自定义本地模型可使用回环 HTTP".into(),
        ));
    }
    Ok(())
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
