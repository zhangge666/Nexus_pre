//! 本文件定义嵌入、OCR、转写与生成能力的 Provider 分类。

use std::{path::PathBuf, sync::Mutex};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

mod completion;

pub use completion::{
    AnthropicCompletion, Completion, CompletionContext, CompletionDelta, CompletionError,
    CompletionFuture, CompletionRequest, CompletionResponse, CompletionStream, CompletionTask,
    CustomCompletion, LocalExtractiveCompletion, OllamaCompletion, OpenAiCompletion,
};

/// 表示嵌入模型加载、推理或并发访问错误。
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    /// ONNX 模型加载或推理失败。
    #[error("本地嵌入模型失败: {0}")]
    Model(String),
    /// 模型互斥锁被异常线程污染。
    #[error("本地嵌入模型暂不可用")]
    Unavailable,
    /// 模型没有返回预期的单条向量。
    #[error("本地嵌入模型未返回向量")]
    MissingOutput,
}

/// 抽象本地 ONNX、回退算法或远程嵌入 Provider。
pub trait Embedder: Send + Sync {
    /// 返回向量空间维度。
    fn dimension(&self) -> usize;

    /// 返回用于检测向量空间兼容性的稳定模型标识。
    fn model_id(&self) -> &str;

    /// 将单段文本转换为单位语义向量。
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

/// 使用 ONNX Runtime 执行 BGE-small-en-v1.5 本地文本嵌入。
pub struct LocalOnnxEmbedder {
    model: Mutex<TextEmbedding>,
}

impl LocalOnnxEmbedder {
    /// 从指定缓存目录加载模型；首次调用会按 fastembed 官方流程下载模型资产。
    pub fn open(cache_dir: impl Into<PathBuf>) -> Result<Self, EmbeddingError> {
        let options = TextInitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_cache_dir(cache_dir.into())
            .with_show_download_progress(false);
        let model = TextEmbedding::try_new(options)
            .map_err(|error| EmbeddingError::Model(error.to_string()))?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

impl Embedder for LocalOnnxEmbedder {
    /// 返回 BGE-small-en-v1.5 的实际 384 维输出。
    fn dimension(&self) -> usize {
        384
    }

    /// 返回持久化嵌入配置使用的模型标识。
    fn model_id(&self) -> &str {
        "bge-small-en-v1.5-onnx"
    }

    /// 以 passage 前缀生成归一化语义向量。
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let input = format!("passage: {text}");
        self.model
            .lock()
            .map_err(|_| EmbeddingError::Unavailable)?
            .embed([input], None)
            .map_err(|error| EmbeddingError::Model(error.to_string()))?
            .into_iter()
            .next()
            .ok_or(EmbeddingError::MissingOutput)
    }
}

/// 表示文档中约定的 AI 能力类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// 将文本转换为语义向量。
    Embedding,
    /// 识别图片中的文本。
    Ocr,
    /// 将音频转换为文本。
    Transcription,
    /// 执行总结、卡片或问答生成。
    Completion,
}

/// 返回软件族计划支持的全部 AI 能力。
#[must_use]
pub const fn capabilities() -> &'static [Capability] {
    &[
        Capability::Embedding,
        Capability::Ocr,
        Capability::Transcription,
        Capability::Completion,
    ]
}
