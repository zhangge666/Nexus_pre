//! 本文件装配 Orbit Tauri 运行时、本地服务持有者仲裁以及统一 HTTP 协议访问。

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use nexus_core::{Collection, HashEmbedder, MemoryStore};
use nexus_protocol::dto::{ListMemoriesResponse, MemoryResponse};
use nexus_protocol::{
    CapabilityGrant, LocalServiceClaim, ProtocolState, Scope, serve_with_shutdown,
    shared_nexus_data_dir,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{Emitter, Manager, State};

/// 持有 Orbit 使用的本地协议客户端以及当前持有服务的可选关闭信号。
struct OrbitState {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    role: ServiceRole,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
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

/// 初始化持有者或客户端角色，并启动 Orbit Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let data_dir = prepare_shared_data_dir(&app_data_dir)?;
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
                    }
                }
                LocalServiceClaim::Client(discovery) => OrbitState {
                    client: reqwest::Client::new(),
                    endpoint: discovery.endpoint,
                    token: discovery.token,
                    role: ServiceRole::Client,
                    shutdown: Mutex::new(None),
                },
            };
            app.manage(Arc::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_memory,
            search_memory,
            list_memories,
            get_memory,
            list_collection_memories,
            update_memory,
            list_collections,
            create_collection,
            add_memory_to_collection,
            get_service_status,
            list_connected_apps,
            revoke_app
        ])
        .run(tauri::generate_context!())
        .expect("Orbit Tauri 运行时启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

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
        let state = OrbitState {
            client: reqwest::Client::new(),
            endpoint,
            token,
            role: ServiceRole::Holder,
            shutdown: Mutex::new(None),
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
        let listed = state.list_memories(None).await.expect("协议列表读取应成功");
        assert_eq!(listed.len(), 1);
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
