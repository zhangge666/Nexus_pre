//! 本文件装配 Orbit Tauri 运行时，并通过 IPC 暴露 nexus-core 写入与检索能力。

use std::sync::Arc;

use nexus_core::{
    ContentFormat, HashEmbedder, IngestInput, Ingestor, MemoryKind, MemorySource, MemoryStore,
    SearchMode, SearchQuery,
};
use serde::Serialize;
use tauri::{Manager, State};

/// 持有 Orbit 进程内共享的记忆库与嵌入器。
struct OrbitState {
    store: MemoryStore,
    embedder: HashEmbedder,
}

/// 表示前端写入成功后需要展示的记忆标识与时间。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatedMemory {
    id: String,
    created_at: i64,
}

/// 表示前端检索列表使用的块级命中结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryHit {
    memory_id: String,
    block_id: String,
    score: f32,
    snippet: String,
}

/// 通过 Tauri IPC 将 Markdown 内容写入统一记忆库。
#[tauri::command]
fn create_memory(
    content: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<CreatedMemory, String> {
    let memory = Ingestor::new(&state.store, &state.embedder)
        .ingest(IngestInput {
            source: MemorySource::Orbit,
            kind: MemoryKind::Note,
            title: content.lines().next().map(str::to_owned),
            content,
            content_format: ContentFormat::Markdown,
            tags: Vec::new(),
            captured_at: None,
            device_id: "orbit-desktop".into(),
            meta: serde_json::json!({"entrypoint": "tauri-ipc"}),
        })
        .map_err(|error| error.to_string())?;
    Ok(CreatedMemory {
        id: memory.id.to_string(),
        created_at: memory.created_at,
    })
}

/// 通过 Tauri IPC 对本地记忆库执行默认混合检索。
#[tauri::command]
fn search_memory(
    query: String,
    state: State<'_, Arc<OrbitState>>,
) -> Result<Vec<MemoryHit>, String> {
    state
        .store
        .search(
            &SearchQuery {
                text: query,
                mode: SearchMode::Hybrid,
                filters: Default::default(),
                limit: 20,
            },
            &state.embedder,
        )
        .map(|hits| {
            hits.into_iter()
                .map(|hit| MemoryHit {
                    memory_id: hit.memory_id.to_string(),
                    block_id: hit.block_id.to_string(),
                    score: hit.score,
                    snippet: hit.snippet,
                })
                .collect()
        })
        .map_err(|error| error.to_string())
}

/// 初始化工作库并启动 Orbit Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = MemoryStore::open(data_dir.join("nexus.db"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(Arc::new(OrbitState {
                store,
                embedder: HashEmbedder::default(),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![create_memory, search_memory])
        .run(tauri::generate_context!())
        .expect("Orbit Tauri 运行时启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 IPC 背后的核心状态能够完成写入和检索。
    #[test]
    fn ipc_state_supports_write_and_search() {
        let state = OrbitState {
            store: MemoryStore::open_in_memory().expect("应能创建内存工作库"),
            embedder: HashEmbedder::default(),
        };
        let memory = Ingestor::new(&state.store, &state.embedder)
            .ingest(IngestInput {
                source: MemorySource::Orbit,
                kind: MemoryKind::Note,
                title: Some("IPC test".into()),
                content: "Tauri IPC connects React to the local memory core.".into(),
                content_format: ContentFormat::Markdown,
                tags: Vec::new(),
                captured_at: None,
                device_id: "test-device".into(),
                meta: serde_json::json!({}),
            })
            .expect("IPC 写入依赖应成功");
        let hits = state
            .store
            .search(
                &SearchQuery {
                    text: "Tauri IPC".into(),
                    mode: SearchMode::Hybrid,
                    filters: Default::default(),
                    limit: 5,
                },
                &state.embedder,
            )
            .expect("IPC 检索依赖应成功");
        assert_eq!(hits[0].memory_id, memory.id);
    }
}
