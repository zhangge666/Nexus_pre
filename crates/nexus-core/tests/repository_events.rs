//! 本文件验证记忆 CRUD、分页过滤、级联索引删除和事件广播顺序。

use std::time::Duration;

use nexus_core::{
    ContentFormat, CoreEvent, HashEmbedder, IngestInput, Ingestor, ListQuery, MemoryFilters,
    MemoryKind, MemoryPatch, MemorySource, MemoryStore, SearchMode, SearchQuery,
};

/// 创建测试使用的完整记忆并返回其模型。
fn create_fixture(
    store: &MemoryStore,
    embedder: &HashEmbedder,
    content: &str,
) -> nexus_core::Memory {
    Ingestor::new(store, embedder)
        .ingest(IngestInput {
            source: MemorySource::Quill,
            kind: MemoryKind::Note,
            title: Some("测试笔记".into()),
            content: content.into(),
            content_format: ContentFormat::Markdown,
            tags: vec!["reading".into()],
            captured_at: None,
            device_id: "test-device".into(),
            meta: serde_json::json!({"version": 1}),
        })
        .expect("测试记忆应写入成功")
}

/// 验证读取、字段更新、重新索引和级联删除形成完整闭环。
#[test]
fn supports_crud_and_cascading_search_cleanup() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let embedder = HashEmbedder::default();
    let created = create_fixture(&store, &embedder, "Original searchable phrase");
    assert_eq!(
        store.get(&created.id).expect("读取应成功").unwrap().title,
        created.title
    );

    let updated = store
        .update(
            &created.id,
            MemoryPatch {
                content: Some("Replacement searchable phrase".into()),
                tags: Some(vec!["updated".into()]),
                pinned: Some(true),
                ..Default::default()
            },
            &embedder,
        )
        .expect("更新应成功");
    assert!(updated.pinned);
    assert_eq!(updated.tags, vec!["updated"]);
    let old_hits = store
        .search(
            &SearchQuery {
                text: "Original".into(),
                mode: SearchMode::Keyword,
                filters: Default::default(),
                limit: 10,
            },
            &embedder,
        )
        .expect("旧内容检索应成功");
    assert!(old_hits.is_empty());

    store.delete(&created.id).expect("删除应成功");
    assert!(store.get(&created.id).expect("读取应成功").is_none());
    let deleted_hits = store
        .search(
            &SearchQuery {
                text: "Replacement".into(),
                mode: SearchMode::Hybrid,
                filters: Default::default(),
                limit: 10,
            },
            &embedder,
        )
        .expect("删除后检索应成功");
    assert!(deleted_hits.is_empty());
}

/// 验证列表和检索均应用来源、类别与标签过滤。
#[test]
fn filters_list_and_search_results() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let embedder = HashEmbedder::default();
    let created = create_fixture(&store, &embedder, "Shared filtering keyword");
    let filters = MemoryFilters {
        sources: vec!["quill".into()],
        kinds: vec![MemoryKind::Note],
        tags: vec!["reading".into()],
        ..Default::default()
    };
    let page = store
        .list(&ListQuery {
            filters: filters.clone(),
            limit: 20,
            offset: 0,
        })
        .expect("列表过滤应成功");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, created.id);

    let hits = store
        .search(
            &SearchQuery {
                text: "filtering".into(),
                mode: SearchMode::Hybrid,
                filters,
                limit: 10,
            },
            &embedder,
        )
        .expect("检索过滤应成功");
    assert_eq!(hits[0].memory_id, created.id);
}

/// 验证事件只在事务完成后按创建、更新、删除顺序广播。
#[test]
fn publishes_committed_events_in_order() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let embedder = HashEmbedder::default();
    let subscription = store.subscribe().expect("事件订阅应成功");
    let memory = create_fixture(&store, &embedder, "Event sequence");
    store
        .update(
            &memory.id,
            MemoryPatch {
                archived: Some(true),
                ..Default::default()
            },
            &embedder,
        )
        .expect("更新应成功");
    store.delete(&memory.id).expect("删除应成功");

    assert_eq!(
        subscription.recv_timeout(Duration::from_secs(1)),
        Some(CoreEvent::MemoryCreated {
            id: memory.id,
            source: "quill".into(),
        })
    );
    assert_eq!(
        subscription.recv_timeout(Duration::from_secs(1)),
        Some(CoreEvent::MemoryUpdated {
            id: memory.id,
            source: "quill".into(),
        })
    );
    assert_eq!(
        subscription.recv_timeout(Duration::from_secs(1)),
        Some(CoreEvent::MemoryDeleted {
            id: memory.id,
            source: "quill".into(),
        })
    );
}
