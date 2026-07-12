//! 本文件验证媒体迁移、跨 Memory 去重和最后引用删除时的文件清理。

use nexus_core::{
    ContentFormat, HashEmbedder, IngestInput, Ingestor, MasterKey, MediaKind, MediaMetadata,
    MediaService, MediaVault, MemoryKind, MemorySource, MemoryStore,
};

/// 创建一条测试 Memory。
fn create_memory(store: &MemoryStore, name: &str) -> nexus_core::Memory {
    Ingestor::new(store, &HashEmbedder::default())
        .ingest(IngestInput {
            source: MemorySource::Echo,
            kind: MemoryKind::Screen,
            title: Some(name.into()),
            content: format!("{name} OCR content"),
            content_format: ContentFormat::Markdown,
            tags: Vec::new(),
            captured_at: None,
            device_id: "test-device".into(),
            meta: serde_json::json!({}),
        })
        .expect("测试记忆应创建成功")
}

/// 验证相同媒体只保存一份，并在最后一个 Memory 删除时清理文件。
#[test]
fn deduplicates_media_across_memories_and_cleans_last_reference() {
    let root =
        std::env::temp_dir().join(format!("nexus-media-repository-{}", uuid::Uuid::now_v7()));
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let vault = MediaVault::open(&root, MasterKey::from_bytes([7; 32])).expect("媒体仓库应打开");
    let service = MediaService::new(&store, &vault);
    let first_memory = create_memory(&store, "first");
    let second_memory = create_memory(&store, "second");
    let first = service
        .attach(
            &first_memory.id,
            MediaKind::Image,
            b"same screenshot",
            "image/png",
            MediaMetadata::default(),
        )
        .expect("媒体关联应成功");
    let second = service
        .attach(
            &second_memory.id,
            MediaKind::Image,
            b"same screenshot",
            "image/png",
            MediaMetadata::default(),
        )
        .expect("重复媒体关联应成功");
    assert_eq!(first.id, second.id);
    assert!(first.path.as_str().len() > 1);

    service
        .delete_memory(&first_memory.id)
        .expect("首个记忆删除应成功");
    assert!(std::path::Path::new(&first.path).exists());
    service
        .delete_memory(&second_memory.id)
        .expect("第二个记忆删除应成功");
    assert!(!std::path::Path::new(&first.path).exists());
    std::fs::remove_dir_all(root).expect("测试目录应清理成功");
}
