//! 本文件装配 Orbit Tauri 运行时、本地服务持有者仲裁以及统一 HTTP 协议访问。

use std::sync::{Arc, Mutex};

use nexus_core::{HashEmbedder, MemoryStore};
use nexus_protocol::dto::{ListMemoriesResponse, MemoryResponse};
use nexus_protocol::{
    CapabilityGrant, LocalServiceClaim, ProtocolState, Scope, serve_with_shutdown,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{Manager, State};

/// 持有 Orbit 使用的本地协议客户端以及当前持有服务的可选关闭信号。
struct OrbitState {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl OrbitState {
    /// 通过本地 Memory Protocol 创建手动记忆。
    async fn create_memory(&self, content: String) -> Result<CreatedMemory, String> {
        self.send_json(
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
        .await
    }

    /// 通过本地 Memory Protocol 执行默认混合检索。
    async fn search_memory(&self, query: String) -> Result<Vec<MemoryHit>, String> {
        let response: SearchResponse = self
            .send_json(
                self.client.post(format!("{}/v1/search", self.endpoint)),
                serde_json::json!({"text": query, "mode": "hybrid", "limit": 20}),
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
    tags: Vec<String>,
    pinned: bool,
    archived: bool,
    created_at: i64,
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
            tags: memory.tags,
            pinned: memory.pinned,
            archived: memory.archived,
            created_at: memory.created_at,
        }
    }
}

/// 通过 Tauri IPC 将 Markdown 内容写入统一记忆服务。
#[tauri::command]
async fn create_memory(
    content: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<CreatedMemory, String> {
    state.create_memory(content).await
}

/// 通过 Tauri IPC 对本地记忆服务执行默认混合检索。
#[tauri::command]
async fn search_memory(
    query: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<MemoryHit>, String> {
    state.search_memory(query).await
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

/// 初始化持有者或客户端角色，并启动 Orbit Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
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
                        shutdown: Mutex::new(Some(shutdown_sender)),
                    }
                }
                LocalServiceClaim::Client(discovery) => OrbitState {
                    client: reqwest::Client::new(),
                    endpoint: discovery.endpoint,
                    token: discovery.token,
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
            get_memory
        ])
        .run(tauri::generate_context!())
        .expect("Orbit Tauri 运行时启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// 验证 Orbit 无论角色如何都能通过本地协议完成写入和检索。
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
            shutdown: Mutex::new(None),
        };
        let created = state
            .create_memory("Tauri IPC connects through Memory Protocol.".into())
            .await
            .expect("协议写入应成功");
        let hits = state
            .search_memory("Memory Protocol".into())
            .await
            .expect("协议检索应成功");
        assert_eq!(hits[0].memory_id, created.id);
        server.abort();
    }
}
