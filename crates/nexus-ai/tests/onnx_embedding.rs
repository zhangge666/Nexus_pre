//! 本文件验证 BGE-small ONNX 模型能够加载并生成实际 384 维向量。

use fastembed::{EmbeddingModel, TextEmbedding};
use nexus_ai::{Embedder, LocalOnnxEmbedder};

/// 下载一次模型后验证真实 ONNX 推理输出。
#[test]
#[ignore = "首次运行需要下载 BGE-small 模型"]
fn generates_bge_small_embedding() {
    let cache = std::env::temp_dir().join("nexus-fastembed-cache");
    let embedder = LocalOnnxEmbedder::open(cache).expect("BGE-small ONNX 模型应加载成功");
    let vector = embedder
        .embed("Nexus local memory retrieval")
        .expect("ONNX 推理应成功");
    assert_eq!(vector.len(), 384);
    assert!(vector.iter().any(|value| value.abs() > f32::EPSILON));
    assert!(
        TextEmbedding::list_supported_models()
            .iter()
            .any(|model| model.model == EmbeddingModel::BGESmallENV15)
    );
}
