//! 本文件验证 sqlite-vec 原生扩展、384 维索引和语义检索路径实际可用。

use nexus_core::{
    ContentFormat, HashEmbedder, IngestInput, Ingestor, MemoryKind, MemorySource, MemoryStore,
    SearchMode, SearchQuery,
};

/// 验证写入向量进入 vec0 虚拟表并可通过 MATCH 查询命中。
#[test]
fn indexes_and_queries_vectors_with_sqlite_vec() {
    let store = MemoryStore::open_in_memory().expect("sqlite-vec 工作库应打开");
    let embedder = HashEmbedder::default();
    let memory = Ingestor::new(&store, &embedder)
        .ingest(IngestInput {
            source: MemorySource::Orbit,
            kind: MemoryKind::Note,
            title: Some("向量测试".into()),
            content: "semantic vector index verification".into(),
            content_format: ContentFormat::Plain,
            tags: Vec::new(),
            captured_at: None,
            device_id: "test-device".into(),
            meta: serde_json::json!({}),
        })
        .expect("记忆应写入 vec0 索引");
    let hits = store
        .search(
            &SearchQuery {
                text: "vector index".into(),
                mode: SearchMode::Semantic,
                filters: Default::default(),
                limit: 5,
            },
            &embedder,
        )
        .expect("sqlite-vec MATCH 查询应成功");
    assert_eq!(hits[0].memory_id, memory.id);
}
