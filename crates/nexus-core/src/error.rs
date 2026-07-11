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
}

/// Nexus 核心模块统一使用的结果类型。
pub type Result<T> = std::result::Result<T, CoreError>;
