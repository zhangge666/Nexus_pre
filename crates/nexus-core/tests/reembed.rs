//! 本文件验证嵌入模型版本检测与全库重嵌入切换。

use nexus_core::{
    ContentFormat, Embedder, EmbeddingError, HashEmbedder, IngestInput, Ingestor, MemoryKind,
    MemorySource, MemoryStore,
};

/// 使用不同标识复用 384 维哈希算法，模拟切换到新模型空间。
struct AlternateEmbedder(HashEmbedder);

impl Embedder for AlternateEmbedder {
    /// 返回 sqlite-vec 索引要求的维度。
    fn dimension(&self) -> usize {
        384
    }

    /// 返回模拟的新模型标识。
    fn model_id(&self) -> &str {
        "alternate-384-test"
    }

    /// 生成测试向量。
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.0.embed(text)
    }
}

/// 验证模型不匹配时拒绝混写，重嵌入后原子切换配置。
#[test]
fn detects_and_rebuilds_embedding_space() {
    let store = MemoryStore::open_in_memory().expect("工作库应打开");
    let fallback = HashEmbedder::default();
    Ingestor::new(&store, &fallback)
        .ingest(IngestInput {
            source: MemorySource::Orbit,
            kind: MemoryKind::Note,
            title: None,
            content: "embedding migration content".into(),
            content_format: ContentFormat::Plain,
            tags: Vec::new(),
            captured_at: None,
            device_id: "test-device".into(),
            meta: serde_json::json!({}),
        })
        .expect("初始写入应成功");
    let alternate = AlternateEmbedder(HashEmbedder::default());
    assert!(store.ensure_embedding_profile(&alternate).is_err());
    assert_eq!(store.reembed_all(&alternate).expect("重嵌入应成功"), 1);
    assert_eq!(
        store.embedding_profile().unwrap().model,
        "alternate-384-test"
    );
}
