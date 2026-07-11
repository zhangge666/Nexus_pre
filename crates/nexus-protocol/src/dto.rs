//! 本文件定义 Memory Protocol v1 的 JSON 请求与响应契约。

use nexus_core::{ContentFormat, MemoryKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 表示 `POST /v1/memories` 的请求正文。
#[derive(Debug, Deserialize)]
pub struct CreateMemoryRequest {
    /// `echo`、`muse`、`quill`、`orbit` 或 `external:<app_id>`。
    pub source: String,
    /// 统一记忆类别。
    pub kind: MemoryKind,
    /// 可选标题。
    pub title: Option<String>,
    /// 记忆正文。
    pub content: String,
    /// Markdown、纯文本或 JSON。
    pub content_format: ContentFormat,
    /// 用户标签。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 来源设备；本地客户端未传时使用协议默认值。
    pub device_id: Option<String>,
    /// 应用自有扩展字段。
    #[serde(default = "empty_meta")]
    pub meta: serde_json::Value,
}

/// 表示记忆创建成功后的最小响应。
#[derive(Debug, Serialize)]
pub struct CreateMemoryResponse {
    /// 新记忆 UUID v7。
    pub id: Uuid,
    /// Unix 毫秒创建时间。
    pub created_at: i64,
}

/// 表示 `POST /v1/search` 的请求正文。
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// 用户查询文本。
    pub text: String,
    /// semantic、keyword 或 hybrid，默认 hybrid。
    #[serde(default = "default_search_mode")]
    pub mode: String,
    /// 最大返回条数，默认 10。
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// 表示一个可序列化的检索命中。
#[derive(Debug, Serialize)]
pub struct SearchHitResponse {
    /// 命中的记忆标识。
    pub memory_id: Uuid,
    /// 命中的块标识。
    pub block_id: Uuid,
    /// 当前检索模式下的分数。
    pub score: f32,
    /// 命中块原文。
    pub snippet: String,
}

/// 表示检索响应的结果集合。
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// 按相关性降序排列的命中。
    pub hits: Vec<SearchHitResponse>,
}

/// 表示服务端当前支持的协议能力。
#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    /// 协议主版本。
    pub version: &'static str,
    /// 已实现的 HTTP 能力名称。
    pub capabilities: &'static [&'static str],
    /// 协议定义的全部授权域。
    pub scopes: Vec<&'static str>,
}

/// 返回空 JSON 对象作为扩展字段默认值。
fn empty_meta() -> serde_json::Value {
    serde_json::json!({})
}

/// 返回文档规定的默认混合检索模式。
fn default_search_mode() -> String {
    "hybrid".into()
}

/// 返回协议规定的默认检索条数。
const fn default_limit() -> usize {
    10
}
