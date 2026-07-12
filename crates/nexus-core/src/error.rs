//! 本文件统一描述核心存储、序列化和输入校验产生的错误。

/// Nexus 核心操作可能返回的错误。
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// SQLite 存储或查询失败。
    #[error("SQLite 操作失败: {0}")]
    Database(#[from] rusqlite::Error),
    /// JSON 扩展字段或向量序列化失败。
    #[error("JSON 处理失败: {0}")]
    Json(#[from] serde_json::Error),
    /// 调用方提供了不符合统一数据模型的输入。
    #[error("输入无效: {0}")]
    InvalidInput(String),
    /// 系统时间早于 Unix epoch，无法写入时间戳。
    #[error("系统时间无效")]
    InvalidSystemTime,
    /// SQLite 连接互斥锁被异常线程污染。
    #[error("存储连接不可用")]
    StoreUnavailable,
    /// 请求的记忆标识不存在。
    #[error("记忆不存在: {0}")]
    NotFound(uuid::Uuid),
    /// sqlite-vec 原生扩展注册失败。
    #[error("sqlite-vec 扩展注册失败，SQLite 错误码: {0}")]
    VectorExtension(i32),
    /// 本地或远程嵌入 Provider 执行失败。
    #[error(transparent)]
    Embedding(#[from] nexus_ai::EmbeddingError),
    /// 当前数据库向量空间与正在使用的嵌入模型不兼容。
    #[error(
        "嵌入空间不兼容: 数据库={stored_model}/{stored_dimensions}维, 请求={requested_model}/{requested_dimensions}维"
    )]
    EmbeddingSpaceMismatch {
        /// 数据库当前模型标识。
        stored_model: String,
        /// 数据库当前向量维度。
        stored_dimensions: usize,
        /// 请求模型标识。
        requested_model: String,
        /// 请求向量维度。
        requested_dimensions: usize,
    },
}

/// Nexus 核心模块统一使用的结果类型。
pub type Result<T> = std::result::Result<T, CoreError>;
