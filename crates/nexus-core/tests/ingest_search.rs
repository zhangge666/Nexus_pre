//! 本文件验证 Memory 从写入、切块、嵌入、SQLite 落库到混合检索命中的完整闭环。

use nexus_core::{
    ContentFormat, HashEmbedder, IngestInput, Ingestor, MemoryKind, MemorySource, MemoryStore,
    SearchMode, SearchQuery,
};

/// 验证一条会议记忆写入后可通过关键词与向量融合检索返回。
#[test]
fn ingested_memory_is_found_by_hybrid_search() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let embedder = HashEmbedder::default();
    let ingestor = Ingestor::new(&store, &embedder);
    let memory = ingestor
        .ingest(IngestInput {
            source: MemorySource::Muse,
            kind: MemoryKind::Idea,
            title: Some("发布会议".into()),
            content:
                "## Release plan\n\nThe desktop release owner is Alice and launch is next Friday."
                    .into(),
            content_format: ContentFormat::Markdown,
            tags: vec!["meeting".into()],
            captured_at: None,
            device_id: "test-device".into(),
            meta: serde_json::json!({"fixture": true}),
        })
        .expect("写入管线应完成事务落库");

    let hits = store
        .search(
            &SearchQuery {
                text: "Alice".into(),
                mode: SearchMode::Hybrid,
                filters: Default::default(),
                limit: 10,
            },
            &embedder,
        )
        .expect("混合检索应成功");

    assert_eq!(hits.first().map(|hit| hit.memory_id), Some(memory.id));
    assert!(hits[0].snippet.contains("Alice"));
}

/// 验证空正文会在进入数据库之前被拒绝。
#[test]
fn rejects_empty_memory_content() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let embedder = HashEmbedder::default();
    let result = Ingestor::new(&store, &embedder).ingest(IngestInput {
        source: MemorySource::Orbit,
        kind: MemoryKind::Note,
        title: None,
        content: "  ".into(),
        content_format: ContentFormat::Plain,
        tags: Vec::new(),
        captured_at: None,
        device_id: "test-device".into(),
        meta: serde_json::json!({}),
    });

    assert!(result.is_err());
}
