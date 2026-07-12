//! 本文件验证记忆关联、集合层级、集合成员和级联清理行为。

use nexus_core::{
    CollectionPatch, ContentFormat, HashEmbedder, IngestInput, Ingestor, LinkCreator, LinkRelation,
    MemoryKind, MemorySource, MemoryStore,
};

/// 创建组织关系测试使用的记忆。
fn create_memory(store: &MemoryStore, embedder: &HashEmbedder, content: &str) -> uuid::Uuid {
    Ingestor::new(store, embedder)
        .ingest(IngestInput {
            source: MemorySource::Orbit,
            kind: MemoryKind::Note,
            title: None,
            content: content.into(),
            content_format: ContentFormat::Plain,
            tags: Vec::new(),
            captured_at: None,
            device_id: "organization-test".into(),
            meta: serde_json::json!({}),
        })
        .expect("测试记忆应创建成功")
        .id
}

/// 验证关联可以创建、查询、删除并随记忆级联清理。
#[test]
fn manages_memory_links() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let from_id = create_memory(&store, &embedder, "source memory");
    let to_id = create_memory(&store, &embedder, "target memory");
    let link = store
        .create_link(from_id, to_id, LinkRelation::References, LinkCreator::User)
        .expect("关联应创建成功");
    assert_eq!(store.list_links(to_id).unwrap(), vec![link]);
    store
        .delete_link(from_id, to_id, LinkRelation::References)
        .expect("关联应删除成功");
    assert!(store.list_links(from_id).unwrap().is_empty());

    store
        .create_link(from_id, to_id, LinkRelation::Related, LinkCreator::System)
        .expect("关联应再次创建成功");
    store.delete(&to_id).expect("目标记忆应删除成功");
    assert!(store.list_links(from_id).unwrap().is_empty());
}

/// 验证集合 CRUD、嵌套循环保护和多集合成员关系。
#[test]
fn manages_nested_collections_and_members() {
    let store = MemoryStore::open_in_memory().expect("内存库应创建成功");
    let embedder = HashEmbedder::default();
    let memory_id = create_memory(&store, &embedder, "collection memory");
    let root = store
        .create_collection("工作", Some("briefcase".into()), None, 0)
        .expect("根集合应创建成功");
    let child = store
        .create_collection("项目", None, Some(root.id), 10)
        .expect("子集合应创建成功");
    store
        .add_memory_to_collection(root.id, memory_id)
        .expect("记忆应加入根集合");
    store
        .add_memory_to_collection(child.id, memory_id)
        .expect("记忆应加入子集合");
    assert_eq!(
        store.list_collection_memory_ids(child.id).unwrap(),
        vec![memory_id]
    );

    assert!(
        store
            .update_collection(
                root.id,
                CollectionPatch {
                    parent_id: Some(Some(child.id)),
                    ..Default::default()
                }
            )
            .is_err()
    );
    let updated = store
        .update_collection(
            child.id,
            CollectionPatch {
                name: Some("Nexus 项目".into()),
                sort: Some(20),
                ..Default::default()
            },
        )
        .expect("集合应更新成功");
    assert_eq!(updated.name, "Nexus 项目");

    store.delete_collection(root.id).expect("根集合应删除成功");
    assert_eq!(
        store.get_collection(child.id).unwrap().unwrap().parent_id,
        None
    );
    assert!(store.list_collection_memory_ids(child.id).is_ok());
}
