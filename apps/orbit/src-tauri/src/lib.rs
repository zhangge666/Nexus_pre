//! 本文件装配 Orbit Tauri 运行时、本地服务持有者仲裁以及统一 HTTP 协议访问。

mod credentials;

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{Local, Timelike};
use futures_util::StreamExt;
use nexus_core::{Collection, HashEmbedder, Link, LinkRelation, MemoryStore};
use nexus_protocol::dto::{ListMemoriesResponse, MemoryResponse};
use nexus_protocol::{
    CapabilityGrant, LocalServiceClaim, ProtocolState, Scope, serve_with_shutdown,
    shared_nexus_data_dir,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

/// 持有 Orbit 使用的本地协议客户端以及当前持有服务的可选关闭信号。
struct OrbitState {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    role: ServiceRole,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    settings_path: PathBuf,
    settings: Arc<Mutex<serde_json::Value>>,
}

/// 表示当前 Orbit 是本地服务持有者还是连接既有服务的客户端。
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ServiceRole {
    Holder,
    Client,
}

/// 表示前端状态栏展示本地服务健康度所需的最小诊断信息。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceStatus {
    role: ServiceRole,
    endpoint: String,
    available: bool,
    message: Option<String>,
}

/// 返回 Orbit M4 全量默认设置；Completion 默认离线且不发送任何数据。
fn default_settings() -> serde_json::Value {
    serde_json::json!({
        "search": {"defaultMode": "hybrid", "enableRerank": true, "defaultScope": "all"},
        "rag": {
            "provider": "local",
            "apiKey": "",
            "hasApiKey": false,
            "model": "",
            "customEndpoint": "",
            "streamEnabled": true,
            "confirmBeforeSend": true
        },
        "cards": {"generationMode": "ai", "provider": "local", "maxCardsPerNote": 10, "defaultDeck": "默认"},
        "review": {"algorithm": "fsrs", "dailyNewLimit": 20, "dailyReviewLimit": 100, "reminderTime": "08:00", "reminderEnabled": true, "lastDesktopReminderDate": ""},
        "links": {"autoLink": true, "dedupeThreshold": 0.85, "graphDensity": 0.6},
        "sync": {"mode": "local", "relayEndpoint": "", "conflictStrategy": "auto"},
        "appearance": {"theme": "dark", "language": "zh-CN"}
    })
}

/// 从磁盘读取非敏感设置并与当前默认值做深度合并。
fn load_settings(path: &Path) -> serde_json::Value {
    let mut settings = default_settings();
    if let Ok(content) = fs::read_to_string(path)
        && let Ok(saved) = serde_json::from_str::<serde_json::Value>(&content)
    {
        merge_json(&mut settings, &saved);
    }
    // API Key 永远不从磁盘恢复；应用重启后云 Provider 需重新输入自带 Key。
    set_json_string(&mut settings, "/rag/apiKey", "");
    set_json_bool(&mut settings, "/rag/hasApiKey", false);
    settings
}

/// 递归合并 JSON 对象，使前端可以只提交一个设置分组。
fn merge_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                if let Some(existing) = target.get_mut(key) {
                    merge_json(existing, value);
                } else {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

/// 设置已存在的 JSON 字符串字段。
fn set_json_string(value: &mut serde_json::Value, pointer: &str, next: &str) {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = serde_json::Value::String(next.into());
    }
}

/// 设置已存在的 JSON 布尔字段。
fn set_json_bool(value: &mut serde_json::Value, pointer: &str, next: bool) {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = serde_json::Value::Bool(next);
    }
}

/// 返回不会暴露 API Key 的设置副本，并用 `hasApiKey` 告知当前进程是否已配置。
fn public_settings(settings: &serde_json::Value) -> serde_json::Value {
    let mut public = settings.clone();
    let provider = json_string(settings, "/rag/provider");
    let has_api_key = settings
        .pointer("/rag/apiKey")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.is_empty())
        || credentials::load_api_key(&provider).is_some();
    set_json_string(&mut public, "/rag/apiKey", "");
    set_json_bool(&mut public, "/rag/hasApiKey", has_api_key);
    public
}

/// 持久化设置时剔除敏感 Key 和仅在进程内成立的 Key 状态。
fn persist_settings(path: &Path, settings: &serde_json::Value) -> Result<(), String> {
    let mut safe = settings.clone();
    set_json_string(&mut safe, "/rag/apiKey", "");
    set_json_bool(&mut safe, "/rag/hasApiKey", false);
    let content = serde_json::to_vec_pretty(&safe).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

/// 准备产品族共享目录，并在首次升级时复制 Orbit 旧目录中的数据库与媒体。
fn prepare_shared_data_dir(app_data_dir: &Path) -> io::Result<PathBuf> {
    let shared_dir = shared_nexus_data_dir(app_data_dir);
    fs::create_dir_all(&shared_dir)?;
    let shared_database = shared_dir.join("nexus.db");
    if !shared_database.exists() && app_data_dir.join("nexus.db").exists() {
        for file_name in ["nexus.db", "nexus.db-wal", "nexus.db-shm"] {
            let source = app_data_dir.join(file_name);
            if source.exists() {
                fs::copy(source, shared_dir.join(file_name))?;
            }
        }
        copy_directory_if_missing(&app_data_dir.join("media"), &shared_dir.join("media"))?;
    }
    Ok(shared_dir)
}

/// 递归复制尚未存在的媒体目录，避免共享目录迁移破坏历史附件引用。
fn copy_directory_if_missing(source: &Path, target: &Path) -> io::Result<()> {
    if !source.exists() || target.exists() {
        return Ok(());
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory_if_missing(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

impl OrbitState {
    /// 通过本地 Memory Protocol 创建手动记忆。
    async fn create_memory(&self, content: String) -> Result<MemorySummary, String> {
        let created: CreatedMemory = self
            .send_json(
                self.client.post(format!("{}/v1/memories", self.endpoint)),
                serde_json::json!({
                    "source": "orbit",
                    "kind": "note",
                    "title": content.lines().next(),
                    "content": content,
                    "content_format": "markdown",
                    "tags": [],
                    "device_id": "orbit-desktop",
                    "meta": {"entrypoint": "tauri-ipc"}
                }),
            )
            .await?;
        // 创建接口只返回标识与时间；随后读取完整对象，确保前端不会拿到残缺的 mock 形状。
        self.get_memory(created.id).await
    }

    /// 通过本地 Memory Protocol 执行默认混合检索。
    async fn search_memory(&self, query: String, mode: String) -> Result<Vec<MemoryHit>, String> {
        let response: SearchResponse = self
            .send_json(
                self.client.post(format!("{}/v1/search", self.endpoint)),
                serde_json::json!({"text": query, "mode": mode, "limit": 20}),
            )
            .await?;
        Ok(response.hits)
    }

    /// 读取用于 Orbit 时间线和筛选界面的记忆分页数据。
    async fn list_memories(&self, source: Option<String>) -> Result<Vec<MemorySummary>, String> {
        let source = source.filter(|value| {
            let value = value.trim();
            !value.is_empty() && value != "all"
        });
        let path = source.map_or_else(
            || "/v1/memories?limit=100".to_owned(),
            |value| format!("/v1/memories?limit=100&source={value}"),
        );
        let response: ListMemoriesResponse = self
            .send_json(
                self.client.get(format!("{}{}", self.endpoint, path)),
                serde_json::json!({}),
            )
            .await?;
        Ok(response
            .items
            .into_iter()
            .map(MemorySummary::from)
            .collect())
    }

    /// 聚合全部记忆及其关联，用于知识图谱展示；无关联的独立记忆同样必须作为节点返回。
    async fn get_graph_data(&self) -> Result<GraphData, String> {
        let memories = self.list_memories(None).await?;
        let memory_ids = memories
            .iter()
            .map(|memory| memory.id.clone())
            .collect::<std::collections::HashSet<_>>();
        let mut seen_edges = std::collections::HashSet::new();
        let mut edges = Vec::new();
        for memory in &memories {
            let links: Vec<Link> = self
                .send_json(
                    self.client.get(format!(
                        "{}/v1/links?memory_id={}",
                        self.endpoint, memory.id
                    )),
                    serde_json::json!({}),
                )
                .await?;
            for link in links {
                let from = link.from_id.to_string();
                let to = link.to_id.to_string();
                let relation = graph_relation(link.relation);
                if !memory_ids.contains(&from)
                    || !memory_ids.contains(&to)
                    || !seen_edges.insert((from.clone(), to.clone(), relation))
                {
                    continue;
                }
                edges.push(GraphEdge {
                    from,
                    to,
                    relation: relation.into(),
                });
            }
        }
        Ok(GraphData {
            nodes: memories
                .into_iter()
                .map(|memory| GraphNode {
                    id: memory.id,
                    title: memory.title.unwrap_or_else(|| memory.kind.clone()),
                    kind: memory.kind,
                    source: memory.source,
                })
                .collect(),
            edges,
        })
    }

    /// 当前收件箱没有独立持久化模型，先返回稳定的空集合，确保前端显示空状态而不是无限加载。
    async fn list_inbox_items(&self) -> Result<Vec<serde_json::Value>, String> {
        Ok(Vec::new())
    }

    /// 收件箱为空时将已读操作视为幂等成功，为后续持久化收件箱实现保持 IPC 契约。
    async fn mark_inbox_read(&self, _id: String) -> Result<(), String> {
        Ok(())
    }

    /// 读取指定集合中的记忆，使集合导航与 Memory Protocol 的集合成员关系保持一致。
    async fn list_collection_memories(
        &self,
        collection_id: String,
    ) -> Result<Vec<MemorySummary>, String> {
        let ids: Vec<String> = self
            .send_json(
                self.client.get(format!(
                    "{}/v1/collections/{collection_id}/memories",
                    self.endpoint
                )),
                serde_json::json!({}),
            )
            .await?;
        let mut memories = Vec::with_capacity(ids.len());
        for id in ids {
            memories.push(self.get_memory(id).await?);
        }
        Ok(memories)
    }

    /// 读取详情面板展示的完整记忆。
    async fn get_memory(&self, id: String) -> Result<MemorySummary, String> {
        let response: MemoryResponse = self
            .send_json(
                self.client
                    .get(format!("{}/v1/memories/{id}", self.endpoint)),
                serde_json::json!({}),
            )
            .await?;
        Ok(MemorySummary::from(response))
    }

    /// 更新 Orbit 编辑器提交的标题和正文，并返回最新记忆摘要。
    async fn update_memory(
        &self,
        id: String,
        title: Option<String>,
        content: String,
    ) -> Result<MemorySummary, String> {
        let response: MemoryResponse = self
            .send_json(
                self.client
                    .patch(format!("{}/v1/memories/{id}", self.endpoint)),
                serde_json::json!({"title": title, "content": content}),
            )
            .await?;
        Ok(MemorySummary::from(response))
    }

    /// 读取集合树需要的全部集合。
    async fn list_collections(&self) -> Result<Vec<Collection>, String> {
        self.send_json(
            self.client.get(format!("{}/v1/collections", self.endpoint)),
            serde_json::json!({}),
        )
        .await
    }

    /// 创建集合并返回可立即插入侧边栏的数据。
    async fn create_collection(&self, name: String) -> Result<Collection, String> {
        self.send_json(
            self.client
                .post(format!("{}/v1/collections", self.endpoint)),
            serde_json::json!({"name": name}),
        )
        .await
    }

    /// 幂等地将记忆归入集合。
    async fn add_memory_to_collection(
        &self,
        collection_id: String,
        memory_id: String,
    ) -> Result<(), String> {
        self.send_json::<serde_json::Value>(
            self.client.put(format!(
                "{}/v1/collections/{collection_id}/memories/{memory_id}",
                self.endpoint
            )),
            serde_json::json!({}),
        )
        .await
        .map(|_| ())
    }

    /// 返回 Memory Protocol 中真实登记的本地应用连接。
    async fn list_connected_apps(&self) -> Result<Vec<ConnectedApp>, String> {
        self.send_json(
            self.client.get(format!("{}/v1/connections", self.endpoint)),
            serde_json::json!({}),
        )
        .await
    }

    /// 撤销指定应用 capability token，使后续写入立即返回未授权。
    async fn revoke_app(&self, token_id: String) -> Result<(), String> {
        self.send_json::<serde_json::Value>(
            self.client
                .delete(format!("{}/v1/connections/{token_id}", self.endpoint)),
            serde_json::json!({}),
        )
        .await
        .map(|_| ())
    }

    /// 返回当前已经到期的真实 FSRS 复习队列。
    async fn get_review_queue(&self) -> Result<Vec<ReviewCard>, String> {
        self.send_json(
            self.client.get(format!("{}/v1/reviews/due", self.endpoint)),
            serde_json::json!({}),
        )
        .await
    }

    /// 返回全部知识卡片及其派生来源。
    async fn list_review_cards(&self) -> Result<Vec<ReviewCard>, String> {
        self.send_json(
            self.client.get(format!("{}/v1/reviews", self.endpoint)),
            serde_json::json!({}),
        )
        .await
    }

    /// 返回真实复习统计。
    async fn get_review_stats(&self) -> Result<ReviewStats, String> {
        self.send_json(
            self.client
                .get(format!("{}/v1/reviews/stats", self.endpoint)),
            serde_json::json!({}),
        )
        .await
    }

    /// 提交一张卡片的 Again/Hard/Good/Easy 评分。
    async fn grade_card(&self, memory_id: String, rating: String) -> Result<GradeResult, String> {
        self.send_json(
            self.client
                .post(format!("{}/v1/reviews/{memory_id}/grade", self.endpoint)),
            serde_json::json!({"rating": rating}),
        )
        .await
    }

    /// 创建一张手动知识卡片。
    async fn create_card(&self, request: CreateCardRequest) -> Result<ReviewCard, String> {
        self.send_json(
            self.client.post(format!("{}/v1/cards", self.endpoint)),
            serde_json::json!({
                "card_front": request.card_front,
                "card_back": request.card_back,
                "source_memory_id": request.source_memory_id,
                "deck": request.deck,
                "tags": request.tags,
            }),
        )
        .await
    }

    /// 从用户选中的来源记忆生成卡片。
    async fn generate_cards(
        &self,
        request: GenerateCardsRequest,
    ) -> Result<Vec<ReviewCard>, String> {
        self.send_json(
            self.client
                .post(format!("{}/v1/cards/generate", self.endpoint)),
            serde_json::json!({
                "source_memory_id": request.source_memory_id,
                "instruction": request.instruction,
                "deck": request.deck,
                "tags": request.tags,
                "max_cards": request.max_cards,
            }),
        )
        .await
    }

    /// 执行真实 `/v1/ask`，保留引用和 Provider 数据流向元数据。
    async fn ask_memory(
        &self,
        question: String,
        scope: Option<AskScope>,
    ) -> Result<AskResponse, String> {
        self.send_json(
            self.client.post(format!("{}/v1/ask", self.endpoint)),
            serde_json::json!({"question": question, "scope": scope}),
        )
        .await
    }

    /// 连接协议 SSE 问答端点，并把元数据、文本增量和结束事件桥接到指定 Tauri 窗口。
    async fn ask_memory_stream(
        &self,
        question: String,
        scope: Option<AskScope>,
        request_id: String,
        app: tauri::AppHandle,
    ) -> Result<(), String> {
        let response = self
            .client
            .post(format!("{}/v1/ask/stream", self.endpoint))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"question": question, "scope": scope}))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(format!("本地记忆服务返回 {status}: {message}"));
        }

        let mut bytes = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|error| error.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(data) = take_sse_data(&mut buffer) {
                let frame: AskStreamFrame =
                    serde_json::from_str(&data).map_err(|error| error.to_string())?;
                app.emit(
                    "ask-stream",
                    AskStreamEvent {
                        request_id: request_id.clone(),
                        frame,
                    },
                )
                .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    /// 从系统凭据库恢复上次的 Provider 配置，使应用重启后无需重新输入云端 Key。
    async fn restore_completion(&self) -> Result<(), String> {
        let settings = self
            .settings
            .lock()
            .map_err(|_| "Orbit 设置状态不可用".to_owned())?
            .clone();
        self.save_settings(settings).await
    }

    fn get_settings(&self) -> Result<serde_json::Value, String> {
        self.settings
            .lock()
            .map(|settings| public_settings(&settings))
            .map_err(|_| "Orbit 设置状态不可用".into())
    }

    /// 深度合并设置、切换服务端 Provider，并只持久化非敏感字段。
    async fn save_settings(&self, patch: serde_json::Value) -> Result<(), String> {
        let current = self
            .settings
            .lock()
            .map_err(|_| "Orbit 设置状态不可用".to_owned())?
            .clone();
        let current_provider = json_string(&current, "/rag/provider");
        let current_key = {
            let in_memory = json_string(&current, "/rag/apiKey");
            if in_memory.is_empty() {
                credentials::load_api_key(&current_provider).unwrap_or_default()
            } else {
                in_memory
            }
        };
        let submitted_key = patch
            .pointer("/rag/apiKey")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let mut next = current;
        merge_json(&mut next, &patch);
        let provider = json_string(&next, "/rag/provider");
        let endpoint = matches!(provider.as_str(), "custom" | "ollama")
            .then(|| json_string(&next, "/rag/customEndpoint"))
            .filter(|value| !value.is_empty());
        let api_key = if !provider_uses_api_key(&provider) {
            String::new()
        } else if !submitted_key.is_empty() {
            submitted_key
        } else if provider == current_provider {
            current_key
        } else {
            credentials::load_api_key(&provider).unwrap_or_default()
        };
        if provider_requires_api_key(&provider, endpoint.as_deref()) && api_key.is_empty() {
            return Err(format!("请为 {provider} 填写 API Key"));
        }
        set_json_string(&mut next, "/rag/apiKey", &api_key);

        let status: CompletionStatus = self
            .send_json(
                self.client.post(format!("{}/v1/completion", self.endpoint)),
                serde_json::json!({
                    "provider": provider,
                    "api_key": (!api_key.is_empty()).then(|| api_key.clone()),
                    "model": json_string(&next, "/rag/model"),
                    "endpoint": endpoint,
                }),
            )
            .await?;
        if status.provider != provider
            || status.sends_data_remote
                != provider_sends_data_remote(&provider, endpoint.as_deref())
        {
            return Err("本地服务未激活所选 Completion Provider".into());
        }
        if provider_uses_api_key(&provider) && !api_key.is_empty() {
            credentials::save_api_key(&provider, &api_key)?;
        }
        // Completion 已完成进程内切换，后续设置更新会按 Provider 从凭据库重新取得 Key。
        set_json_string(&mut next, "/rag/apiKey", "");
        persist_settings(&self.settings_path, &next)?;
        *self
            .settings
            .lock()
            .map_err(|_| "Orbit 设置状态不可用".to_owned())? = next;
        Ok(())
    }

    /// 探测已发现的回环服务，供前端在失败时显示可操作的本地诊断信息。
    async fn service_status(&self) -> ServiceStatus {
        let response = self
            .client
            .get(format!("{}/v1/capabilities", self.endpoint))
            .bearer_auth(&self.token)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => ServiceStatus {
                role: self.role,
                endpoint: self.endpoint.clone(),
                available: true,
                message: None,
            },
            Ok(response) => ServiceStatus {
                role: self.role,
                endpoint: self.endpoint.clone(),
                available: false,
                message: Some(format!("本地服务返回 {}", response.status())),
            },
            Err(error) => ServiceStatus {
                role: self.role,
                endpoint: self.endpoint.clone(),
                available: false,
                message: Some(format!("无法连接本地服务：{error}")),
            },
        }
    }

    /// 发送带本地 capability token 的 JSON 请求并解析成功响应。
    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        body: serde_json::Value,
    ) -> Result<T, String> {
        let response = request
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(format!("本地记忆服务返回 {status}: {message}"));
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return serde_json::from_value(serde_json::Value::Null)
                .map_err(|error| error.to_string());
        }
        response.json().await.map_err(|error| error.to_string())
    }
}

/// 读取 JSON 指针指向的字符串设置，缺失时返回空串。
fn json_string(value: &serde_json::Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// 将核心关联枚举映射为前端图谱使用的稳定 snake_case 字符串。
fn graph_relation(relation: LinkRelation) -> &'static str {
    match relation {
        LinkRelation::References => "references",
        LinkRelation::DerivedFrom => "derived_from",
        LinkRelation::Related => "related",
        LinkRelation::Duplicate => "duplicate",
    }
}

/// 判断 Provider 是否需要由系统凭据库维护 API Key，纯本地模式永远不读取或写入密钥。
fn provider_uses_api_key(provider: &str) -> bool {
    matches!(provider, "claude" | "openai" | "custom")
}

/// 判断自定义端点是否严格指向回环地址；本机 OpenAI-compatible 服务允许无 Key。
fn is_loopback_endpoint(endpoint: Option<&str>) -> bool {
    endpoint
        .and_then(|value| reqwest::Url::parse(value.trim()).ok())
        .is_some_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && matches!(
                    url.host_str(),
                    Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
                )
        })
}

/// 判断切换 Provider 前是否必须输入 Key，避免对 Ollama 与本机兼容端点产生无效阻塞。
fn provider_requires_api_key(provider: &str, endpoint: Option<&str>) -> bool {
    matches!(provider, "claude" | "openai")
        || (provider == "custom" && !is_loopback_endpoint(endpoint))
}

/// 映射 Provider 的实际数据流向，供保存设置后校验本地服务是否按预期生效。
fn provider_sends_data_remote(provider: &str, endpoint: Option<&str>) -> bool {
    matches!(provider, "claude" | "openai")
        || (provider == "custom" && !is_loopback_endpoint(endpoint))
}

/// 判断当前本地日期是否已到设置的提醒时刻，并原子认领当天唯一一次桌面通知。
fn claim_desktop_reminder(
    settings: &mut serde_json::Value,
    current_date: &str,
    hour: u32,
    minute: u32,
) -> bool {
    if !settings
        .pointer("/review/reminderEnabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    let reminder_time = json_string(settings, "/review/reminderTime");
    let Some((target_hour, target_minute)) = reminder_time
        .split_once(':')
        .and_then(|(hours, minutes)| {
            Some((hours.parse::<u32>().ok()?, minutes.parse::<u32>().ok()?))
        })
        .filter(|(hours, minutes)| *hours < 24 && *minutes < 60)
    else {
        return false;
    };
    if (hour, minute) < (target_hour, target_minute)
        || json_string(settings, "/review/lastDesktopReminderDate") == current_date
    {
        return false;
    }
    set_json_string(settings, "/review/lastDesktopReminderDate", current_date);
    true
}

/// 返回到期扫描使用的 Unix 毫秒时间。
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

/// 从累积缓冲区取出一帧 SSE 的 data 内容，网络分片未完整时保留到下一次读取。
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

impl Drop for OrbitState {
    /// 在状态释放时通知当前进程持有的本地服务优雅停止。
    fn drop(&mut self) {
        if let Ok(shutdown) = self.shutdown.get_mut()
            && let Some(sender) = shutdown.take()
        {
            let _ = sender.send(());
        }
    }
}

/// 表示前端写入成功后需要展示的记忆标识与时间。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatedMemory {
    id: String,
    #[serde(alias = "created_at")]
    created_at: i64,
}

/// 表示前端检索列表使用的块级命中结果。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryHit {
    #[serde(alias = "memory_id")]
    memory_id: String,
    #[serde(alias = "block_id")]
    block_id: String,
    score: f32,
    snippet: String,
}

/// 表示 Memory Protocol 检索响应包裹结构。
#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<MemoryHit>,
}

/// 表示前端手动创建卡片的输入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCardRequest {
    card_front: String,
    card_back: String,
    source_memory_id: Option<String>,
    deck: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// 表示前端从来源记忆生成卡片的输入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateCardsRequest {
    source_memory_id: String,
    instruction: Option<String>,
    deck: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    max_cards: usize,
}

/// 表示 Orbit 卡片和复习页面使用的完整卡片摘要。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCard {
    #[serde(alias = "memory_id")]
    memory_id: String,
    #[serde(alias = "card_front")]
    card_front: String,
    #[serde(alias = "card_back")]
    card_back: String,
    state: String,
    stability: f64,
    difficulty: f64,
    #[serde(alias = "due_at")]
    due_at: i64,
    #[serde(alias = "last_reviewed_at")]
    last_reviewed_at: Option<i64>,
    reps: u32,
    lapses: u32,
    #[serde(alias = "source_memory_id")]
    source_memory_id: Option<String>,
    #[serde(alias = "source_title")]
    source_title: Option<String>,
    deck: Option<String>,
    tags: Vec<String>,
}

/// 表示复习首页使用的真实聚合统计。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewStats {
    #[serde(alias = "due_today")]
    due_today: usize,
    #[serde(alias = "new_today")]
    new_today: usize,
    #[serde(alias = "reviewed_today")]
    reviewed_today: usize,
    streak: usize,
    mature: usize,
    young: usize,
    #[serde(alias = "total_cards")]
    total_cards: usize,
}

/// 表示评分后的真实 FSRS 调度结果。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GradeResult {
    #[serde(alias = "next_due_at")]
    next_due_at: i64,
    #[serde(alias = "new_stability")]
    new_stability: f64,
    #[serde(alias = "new_difficulty")]
    new_difficulty: f64,
    #[serde(alias = "new_state")]
    new_state: String,
}

/// 表示问答可选集合或来源范围。
#[derive(Debug, Deserialize, Serialize)]
struct AskScope {
    collection: Option<String>,
    source: Option<String>,
}

/// 表示 RAG 回答的一条块级引用。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Citation {
    #[serde(alias = "memory_id")]
    memory_id: String,
    #[serde(alias = "block_id")]
    block_id: String,
    snippet: String,
    #[serde(alias = "source_title")]
    source_title: Option<String>,
    #[serde(alias = "source_kind")]
    source_kind: String,
    #[serde(alias = "created_at")]
    created_at: i64,
}

/// 表示带引用与 Provider 数据流向信息的问答响应。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AskResponse {
    answer: String,
    citations: Vec<Citation>,
    provider: String,
    #[serde(alias = "sent_context_count")]
    sent_context_count: usize,
    #[serde(alias = "sends_data_remote")]
    sends_data_remote: bool,
}

/// 表示协议 SSE 返回的问答帧，字段保持与 `/v1/ask` 响应一致。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AskStreamFrame {
    /// 首帧携带 Provider、引用和数据流向，不会重复传输到每个文本增量。
    Meta {
        provider: String,
        citations: Vec<Citation>,
        #[serde(rename = "sentContextCount", alias = "sent_context_count")]
        sent_context_count: usize,
        #[serde(rename = "sendsDataRemote", alias = "sends_data_remote")]
        sends_data_remote: bool,
    },
    /// 一段新增的回答文本。
    Delta { text: String },
    /// 正常生成结束。
    Done,
    /// 生成过程出现 Provider 错误。
    Error { message: String },
}

/// 向前端事件总线发送的流式问答帧，使用请求标识避免并发响应串台。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AskStreamEvent {
    request_id: String,
    #[serde(flatten)]
    frame: AskStreamFrame,
}

/// 表示 Completion 配置成功后的非敏感状态。
#[derive(Debug, Deserialize)]
struct CompletionStatus {
    provider: String,
    sends_data_remote: bool,
}

/// 表示 Orbit 列表和详情面板共用的记忆数据。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemorySummary {
    id: String,
    source: String,
    kind: String,
    title: Option<String>,
    content: String,
    content_format: String,
    tags: Vec<String>,
    pinned: bool,
    archived: bool,
    created_at: i64,
    updated_at: i64,
    captured_at: Option<i64>,
    links: Vec<serde_json::Value>,
}

/// 表示知识图谱所需的最小节点字段，内容来自 Memory Protocol 的真实记忆列表。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNode {
    id: String,
    title: String,
    kind: String,
    source: String,
}

/// 表示知识图谱中的一条已持久化关联。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphEdge {
    from: String,
    to: String,
    relation: String,
}

/// 汇总 Orbit 图谱 IPC 响应的节点与关联，字段名与前端 `GraphNode`/`GraphEdge` 保持一致。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphData {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

/// 表示 Orbit 连接管理页面展示的真实本地授权应用。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectedApp {
    id: String,
    name: String,
    source: String,
    scopes: Vec<String>,
    last_active_at: i64,
    memories_count: usize,
    token_id: String,
}

impl From<MemoryResponse> for MemorySummary {
    /// 将协议记忆响应收窄为 Orbit 当前界面需要的字段。
    fn from(memory: MemoryResponse) -> Self {
        Self {
            id: memory.id.to_string(),
            source: memory.source,
            kind: format!("{:?}", memory.kind).to_lowercase(),
            title: memory.title,
            content: memory.content,
            content_format: format!("{:?}", memory.content_format).to_lowercase(),
            tags: memory.tags,
            pinned: memory.pinned,
            archived: memory.archived,
            created_at: memory.created_at,
            updated_at: memory.updated_at,
            captured_at: memory.captured_at,
            // Memory Protocol 当前详情响应不携带 links；显式返回空数组以保持前端契约完整。
            links: Vec::new(),
        }
    }
}

/// 通过 Tauri IPC 将 Markdown 内容写入统一记忆服务。
#[tauri::command]
async fn create_memory(
    content: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<MemorySummary, String> {
    state.create_memory(content).await
}

/// 通过 Tauri IPC 对本地记忆服务执行默认混合检索。
#[tauri::command]
async fn search_memory(
    query: String,
    mode: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<MemoryHit>, String> {
    state.search_memory(query, mode).await
}

/// 通过 Tauri IPC 返回可按来源筛选的时间线记忆。
#[tauri::command]
async fn list_memories(
    source: Option<String>,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<MemorySummary>, String> {
    state.list_memories(source).await
}

/// 通过 Tauri IPC 读取指定记忆详情。
#[tauri::command]
async fn get_memory(
    id: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<MemorySummary, String> {
    state.get_memory(id).await
}

/// 通过 Tauri IPC 返回指定集合实际包含的记忆列表。
#[tauri::command]
async fn list_collection_memories(
    collection_id: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<MemorySummary>, String> {
    state.list_collection_memories(collection_id).await
}

/// 通过 Tauri IPC 返回所有已持久化记忆和关联，供知识图谱显示独立节点与关联边。
#[tauri::command]
async fn get_graph_data(state: State<'_, Arc<OrbitState>>) -> Result<GraphData, String> {
    state.get_graph_data().await
}

/// 通过 Tauri IPC 返回收件箱项目；当前没有待处理内容时返回空数组。
#[tauri::command]
async fn list_inbox_items(
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<serde_json::Value>, String> {
    state.list_inbox_items().await
}

/// 通过 Tauri IPC 标记收件箱项目已读，当前空收件箱实现保持幂等成功。
#[tauri::command]
async fn mark_inbox_read(id: String, state: State<'_, Arc<OrbitState>>) -> Result<(), String> {
    state.mark_inbox_read(id).await
}

/// 通过 Tauri IPC 保存记忆编辑结果。
#[tauri::command]
async fn update_memory(
    id: String,
    title: Option<String>,
    content: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<MemorySummary, String> {
    state.update_memory(id, title, content).await
}

/// 通过 Tauri IPC 返回集合列表。
#[tauri::command]
async fn list_collections(state: State<'_, Arc<OrbitState>>) -> Result<Vec<Collection>, String> {
    state.list_collections().await
}

/// 通过 Tauri IPC 创建集合。
#[tauri::command]
async fn create_collection(
    name: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Collection, String> {
    state.create_collection(name).await
}

/// 通过 Tauri IPC 将记忆加入集合。
#[tauri::command]
async fn add_memory_to_collection(
    collection_id: String,
    memory_id: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<(), String> {
    state
        .add_memory_to_collection(collection_id, memory_id)
        .await
}

/// 通过 Tauri IPC 返回本地服务当前的角色、端点与连通性诊断。
#[tauri::command]
async fn get_service_status(state: State<'_, Arc<OrbitState>>) -> Result<ServiceStatus, String> {
    Ok(state.service_status().await)
}

/// 通过 Tauri IPC 返回当前服务真实登记的本地应用。
#[tauri::command]
async fn list_connected_apps(
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<ConnectedApp>, String> {
    state.list_connected_apps().await
}

/// 通过 Tauri IPC 撤销一条本地应用授权。
#[tauri::command]
async fn revoke_app(token_id: String, state: State<'_, Arc<OrbitState>>) -> Result<(), String> {
    state.revoke_app(token_id).await
}

/// 通过 Tauri IPC 返回真实到期复习队列。
#[tauri::command]
async fn get_review_queue(state: State<'_, Arc<OrbitState>>) -> Result<Vec<ReviewCard>, String> {
    state.get_review_queue().await
}

/// 通过 Tauri IPC 返回全部知识卡片。
#[tauri::command]
async fn list_review_cards(state: State<'_, Arc<OrbitState>>) -> Result<Vec<ReviewCard>, String> {
    state.list_review_cards().await
}

/// 通过 Tauri IPC 返回复习统计。
#[tauri::command]
async fn get_review_stats(state: State<'_, Arc<OrbitState>>) -> Result<ReviewStats, String> {
    state.get_review_stats().await
}

/// 通过 Tauri IPC 提交 FSRS 四档评分。
#[tauri::command]
async fn grade_card(
    memory_id: String,
    rating: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<GradeResult, String> {
    state.grade_card(memory_id, rating).await
}

/// 通过 Tauri IPC 创建手动卡片。
#[tauri::command]
async fn create_card(
    request: CreateCardRequest,
    state: State<'_, Arc<OrbitState>>,
) -> Result<ReviewCard, String> {
    state.create_card(request).await
}

/// 通过 Tauri IPC 从选定来源生成卡片。
#[tauri::command]
async fn generate_cards(
    request: GenerateCardsRequest,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<ReviewCard>, String> {
    state.generate_cards(request).await
}

/// 通过 Tauri IPC 执行带引用的本地 RAG 问答。
#[tauri::command]
async fn ask_memory(
    question: String,
    scope: Option<AskScope>,
    state: State<'_, Arc<OrbitState>>,
) -> Result<AskResponse, String> {
    state.ask_memory(question, scope).await
}

/// 通过 Tauri IPC 启动服务端真实 SSE 问答，并把事件转发给当前 WebView。
#[tauri::command]
async fn ask_memory_stream(
    question: String,
    scope: Option<AskScope>,
    request_id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<OrbitState>>,
) -> Result<(), String> {
    state
        .ask_memory_stream(question, scope, request_id, app)
        .await
}

/// 通过 Tauri IPC 返回脱敏设置；API Key 仅通过 `hasApiKey` 表示存在。
#[tauri::command]
fn get_settings(state: State<'_, Arc<OrbitState>>) -> Result<serde_json::Value, String> {
    state.get_settings()
}

/// 通过 Tauri IPC 保存设置并切换进程内 Completion Provider。
#[tauri::command]
async fn save_settings(
    settings: serde_json::Value,
    state: State<'_, Arc<OrbitState>>,
) -> Result<(), String> {
    state.save_settings(settings).await
}

/// 在 macOS 中清除 NSWindow 的默认矩形底板，使页面根容器的圆角成为真实窗口轮廓。
#[cfg(target_os = "macos")]
fn configure_macos_window_surface(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use objc2_app_kit::{NSColor, NSWindow};

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "未找到 Orbit 主窗口"))?;
    // Tauri 只负责开启透明能力；此处必须同步清除 AppKit 窗口底色，否则圆角外仍会显示矩形背景。
    // SAFETY: 指针由当前 Tauri 主窗口返回，setup 在主线程且窗口生命周期内完成配置。
    let native_window: &NSWindow = unsafe { &*window.ns_window()?.cast() };
    native_window.setOpaque(false);
    let clear_color = NSColor::clearColor();
    native_window.setBackgroundColor(Some(&clear_color));
    Ok(())
}

/// 初始化持有者或客户端角色，并启动 Orbit Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            configure_macos_window_surface(app)?;
            let app_data_dir = app.path().app_data_dir()?;
            let data_dir = prepare_shared_data_dir(&app_data_dir)?;
            let settings_path = data_dir.join("orbit-settings.json");
            let settings = Arc::new(Mutex::new(load_settings(&settings_path)));
            let claim = tauri::async_runtime::block_on(LocalServiceClaim::acquire(&data_dir))?;
            let state = match claim {
                LocalServiceClaim::Holder {
                    lease,
                    listener,
                    discovery,
                } => {
                    let store = Arc::new(
                        MemoryStore::open(data_dir.join("nexus.db"))
                            .map_err(|error| std::io::Error::other(error.to_string()))?,
                    );
                    let embedder = Arc::new(HashEmbedder::default());
                    let event_subscription = store
                        .subscribe()
                        .map_err(|error| std::io::Error::other(error.to_string()))?;
                    let event_app = app.handle().clone();
                    // 核心事件只在事务提交后广播，前端刷新不会读取半成品。
                    tauri::async_runtime::spawn_blocking(move || {
                        while let Some(event) = event_subscription.recv() {
                            if event_app.emit("memory-changed", event).is_err() {
                                break;
                            }
                        }
                    });
                    let reminder_store = Arc::clone(&store);
                    let reminder_app = app.handle().clone();
                    let reminder_settings = Arc::clone(&settings);
                    let reminder_settings_path = settings_path.clone();
                    // 持有者每分钟扫描一次到期状态；核心去重字段确保同一调度周期只发一次事件。
                    tauri::async_runtime::spawn(async move {
                        let mut interval =
                            tokio::time::interval(std::time::Duration::from_secs(60));
                        loop {
                            interval.tick().await;
                            let store = Arc::clone(&reminder_store);
                            let due_count = tauri::async_runtime::spawn_blocking(move || {
                                store.notify_due_reviews(unix_millis(), 200)?;
                                Ok::<usize, nexus_core::CoreError>(
                                    store.reviews_due(unix_millis(), 200)?.len(),
                                )
                            })
                            .await
                            .ok()
                            .and_then(Result::ok)
                            .unwrap_or(0);
                            if due_count == 0 {
                                continue;
                            }
                            let now = Local::now();
                            let should_notify =
                                reminder_settings.lock().ok().is_some_and(|mut settings| {
                                    if !claim_desktop_reminder(
                                        &mut settings,
                                        &now.format("%F").to_string(),
                                        now.hour(),
                                        now.minute(),
                                    ) {
                                        return false;
                                    }
                                    let _ = persist_settings(&reminder_settings_path, &settings);
                                    true
                                });
                            if should_notify {
                                let _ = reminder_app
                                    .notification()
                                    .builder()
                                    .title("Orbit · 复习提醒")
                                    .body(format!("今天有 {due_count} 张卡片等待复习"))
                                    .show();
                            }
                        }
                    });
                    let grant = CapabilityGrant::new(discovery.token.clone(), [Scope::Admin], None);
                    let protocol_state = ProtocolState::from_shared(store, embedder, grant);
                    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
                    tauri::async_runtime::spawn(async move {
                        // 服务任务持有租约，服务异常退出时立即释放锁供其他实例接管。
                        let _lease = lease;
                        let result = serve_with_shutdown(listener, protocol_state, async {
                            let _ = shutdown_receiver.await;
                        })
                        .await;
                        if let Err(error) = result {
                            eprintln!("本地记忆服务退出: {error}");
                        }
                    });
                    OrbitState {
                        client: reqwest::Client::new(),
                        endpoint: discovery.endpoint,
                        token: discovery.token,
                        role: ServiceRole::Holder,
                        shutdown: Mutex::new(Some(shutdown_sender)),
                        settings_path,
                        settings: Arc::clone(&settings),
                    }
                }
                LocalServiceClaim::Client(discovery) => OrbitState {
                    client: reqwest::Client::new(),
                    endpoint: discovery.endpoint,
                    token: discovery.token,
                    role: ServiceRole::Client,
                    shutdown: Mutex::new(None),
                    settings_path,
                    settings: Arc::clone(&settings),
                },
            };
            let state = Arc::new(state);
            let restored_state = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = restored_state.restore_completion().await {
                    eprintln!("恢复 Completion Provider 失败: {error}");
                }
            });
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_memory,
            search_memory,
            list_memories,
            get_memory,
            list_collection_memories,
            get_graph_data,
            list_inbox_items,
            mark_inbox_read,
            update_memory,
            list_collections,
            create_collection,
            add_memory_to_collection,
            get_service_status,
            list_connected_apps,
            revoke_app,
            get_review_queue,
            list_review_cards,
            get_review_stats,
            grade_card,
            create_card,
            generate_cards,
            ask_memory,
            ask_memory_stream,
            get_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("Orbit Tauri 运行时启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// 验证桌面提醒只会在启用、到点且当天尚未认领时触发一次。
    #[test]
    fn claims_desktop_reminder_once_per_day_after_configured_time() {
        let mut settings = default_settings();
        assert!(!claim_desktop_reminder(&mut settings, "2026-07-17", 7, 59));
        assert!(claim_desktop_reminder(&mut settings, "2026-07-17", 8, 0));
        assert!(!claim_desktop_reminder(&mut settings, "2026-07-17", 8, 1));
        assert!(claim_desktop_reminder(&mut settings, "2026-07-18", 8, 0));
        set_json_bool(&mut settings, "/review/reminderEnabled", false);
        assert!(!claim_desktop_reminder(&mut settings, "2026-07-19", 8, 0));
    }

    /// 验证本地端点判断仅接受完整 URL 的回环主机，防止相似域名绕过云端发送确认。
    #[test]
    fn distinguishes_loopback_from_similar_remote_domains() {
        assert!(is_loopback_endpoint(Some("http://127.0.0.1:11434/v1")));
        assert!(is_loopback_endpoint(Some("http://[::1]:11434/v1")));
        assert!(!is_loopback_endpoint(Some(
            "http://127.0.0.1.example.com/v1"
        )));
        assert!(!is_loopback_endpoint(Some(
            "https://localhost.example.com/v1"
        )));
    }

    /// 验证 Tauri SSE 桥接器可跨网络分片保留残片，并只在帧完整时输出 data。
    #[test]
    fn parses_complete_sse_data_without_losing_partial_frames() {
        let mut buffer = "event: delta\ndata: {\"type\":\"delta\"".to_owned();
        assert!(take_sse_data(&mut buffer).is_none());
        buffer.push_str(",\"text\":\"你好\"}\n\n");
        assert_eq!(
            take_sse_data(&mut buffer).as_deref(),
            Some("{\"type\":\"delta\",\"text\":\"你好\"}")
        );
        assert!(buffer.is_empty());
    }

    /// 验证密钥只保留在进程内与系统凭据库，设置 IPC 和 JSON 文件均不会返回或写入明文。
    #[test]
    fn excludes_api_key_from_ipc_and_persisted_settings() {
        let mut settings = default_settings();
        set_json_string(&mut settings, "/rag/apiKey", "process-only-secret");

        let public = public_settings(&settings);
        assert_eq!(public["rag"]["apiKey"], "");
        assert_eq!(public["rag"]["hasApiKey"], true);

        let directory = tempfile::tempdir().expect("应创建临时设置目录");
        let path = directory.path().join("orbit-settings.json");
        persist_settings(&path, &settings).expect("非敏感设置应持久化");
        let persisted = fs::read_to_string(path).expect("应读取已持久化设置");
        assert!(!persisted.contains("process-only-secret"));
    }

    /// 验证升级到共享目录时保留旧 Orbit 数据库和媒体文件。
    #[test]
    fn migrates_legacy_orbit_data_to_shared_directory() {
        let root = tempfile::tempdir().expect("应创建临时目录");
        let legacy = root.path().join("com.nexus.orbit");
        fs::create_dir_all(legacy.join("media/2026")).expect("应创建旧媒体目录");
        fs::write(legacy.join("nexus.db"), b"legacy-db").expect("应写入旧数据库");
        fs::write(legacy.join("media/2026/audio.enc"), b"legacy-media").expect("应写入旧媒体");

        let shared = prepare_shared_data_dir(&legacy).expect("共享目录迁移应成功");
        assert_eq!(fs::read(shared.join("nexus.db")).unwrap(), b"legacy-db");
        assert_eq!(
            fs::read(shared.join("media/2026/audio.enc")).unwrap(),
            b"legacy-media"
        );
    }

    /// 验证 Orbit 能通过本地协议完成写入、编辑、检索、集合归档与服务诊断闭环。
    #[tokio::test]
    async fn protocol_state_supports_write_and_search() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("测试服务应绑定成功");
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let token = "orbit-test-token".to_owned();
        let protocol_state = ProtocolState::new(
            MemoryStore::open_in_memory().expect("应能创建内存工作库"),
            CapabilityGrant::new(token.clone(), [Scope::Admin], None),
        );
        let server = tokio::spawn(async move {
            nexus_protocol::serve(listener, protocol_state)
                .await
                .expect("测试服务应正常运行");
        });
        let settings_dir = tempfile::tempdir().expect("应创建临时设置目录");
        let state = OrbitState {
            client: reqwest::Client::new(),
            endpoint,
            token,
            role: ServiceRole::Holder,
            shutdown: Mutex::new(None),
            settings_path: settings_dir.path().join("orbit-settings.json"),
            settings: Arc::new(Mutex::new(default_settings())),
        };
        let created = state
            .create_memory("Tauri IPC connects through Memory Protocol.".into())
            .await
            .expect("协议写入应成功");
        let hits = state
            .search_memory("Memory Protocol".into(), "hybrid".into())
            .await
            .expect("协议检索应成功");
        assert_eq!(hits[0].memory_id, created.id);
        let answer = state
            .ask_memory("Memory Protocol 如何连接？".into(), None)
            .await
            .expect("本地 RAG 问答应成功");
        assert_eq!(answer.provider, "local");
        assert!(!answer.citations.is_empty());
        assert!(!answer.sends_data_remote);
        state
            .save_settings(serde_json::json!({
                "rag": {
                    "provider": "ollama",
                    "model": "qwen3:8b",
                    "customEndpoint": "http://127.0.0.1:11434"
                }
            }))
            .await
            .expect("Ollama 本地 Provider 应无需 API Key 即可配置");
        let settings = state.get_settings().expect("设置应可读取");
        assert_eq!(settings["rag"]["provider"], "ollama");
        assert_eq!(settings["rag"]["hasApiKey"], false);
        let card = state
            .create_card(CreateCardRequest {
                card_front: "Memory Protocol 的用途是什么？".into(),
                card_back: "连接 Orbit 与来源应用。".into(),
                source_memory_id: Some(created.id.clone()),
                deck: Some("协议".into()),
                tags: vec!["m4".into()],
            })
            .await
            .expect("手动卡片应创建成功");
        assert_eq!(card.source_memory_id.as_deref(), Some(created.id.as_str()));
        assert_eq!(state.get_review_queue().await.unwrap().len(), 1);
        let graded = state
            .grade_card(card.memory_id, "good".into())
            .await
            .expect("真实评分应成功");
        assert_eq!(graded.new_state, "review");
        assert_eq!(state.get_review_stats().await.unwrap().total_cards, 1);
        let listed = state.list_memories(None).await.expect("协议列表读取应成功");
        assert_eq!(listed.len(), 2);
        let all_listed = state
            .list_memories(Some("all".into()))
            .await
            .expect("全部筛选应映射为全量记忆");
        assert_eq!(all_listed.len(), 2);
        let graph = state
            .get_graph_data()
            .await
            .expect("图谱应返回已创建的独立记忆节点");
        assert!(graph.nodes.iter().any(|node| node.id == created.id));
        assert!(
            state
                .list_inbox_items()
                .await
                .expect("空收件箱应返回可用结果")
                .is_empty()
        );
        let updated = state
            .update_memory(
                created.id.clone(),
                Some("已编辑的标题".into()),
                "更新后的内容".into(),
            )
            .await
            .expect("协议编辑应成功");
        assert_eq!(updated.title.as_deref(), Some("已编辑的标题"));
        let collection = state
            .create_collection("测试集合".into())
            .await
            .expect("协议集合创建应成功");
        state
            .add_memory_to_collection(collection.id.to_string(), created.id)
            .await
            .expect("协议归入集合应成功");
        let collection_memories = state
            .list_collection_memories(collection.id.to_string())
            .await
            .expect("协议集合读取应成功");
        assert_eq!(collection_memories.len(), 1);
        assert!(state.service_status().await.available);
        let registration = state
            .client
            .post(format!("{}/v1/connections", state.endpoint))
            .bearer_auth(&state.token)
            .json(&serde_json::json!({
                "app_id": "com.nexus.muse",
                "name": "Muse",
                "source": "muse",
                "scopes": ["memory:write"]
            }))
            .send()
            .await
            .expect("Muse 测试连接应登记成功");
        assert!(registration.status().is_success());
        let connections = state
            .list_connected_apps()
            .await
            .expect("Orbit 应读取真实连接列表");
        assert_eq!(connections[0].source, "muse");
        state
            .revoke_app(connections[0].token_id.clone())
            .await
            .expect("Orbit 应撤销 Muse 连接");
        assert!(state.list_connected_apps().await.unwrap().is_empty());

        server.abort();
    }
}
