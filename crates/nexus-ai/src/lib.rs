//! 本文件定义嵌入、OCR、转写与生成能力的 Provider 分类。

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
