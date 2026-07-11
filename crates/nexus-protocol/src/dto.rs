//! 本文件定义 Memory Protocol v1 的 JSON 请求与响应契约。

use nexus_core::{Block, ContentFormat, Memory, MemoryKind};
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

/// 表示 `PATCH /v1/memories/{id}` 的字段级更新请求。
#[derive(Debug, Default, Deserialize)]
pub struct UpdateMemoryRequest {
    /// 新标题；省略时保持不变。
    pub title: Option<String>,
    /// 新正文；修改后服务端会重建检索索引。
    pub content: Option<String>,
    /// 新正文格式。
    pub content_format: Option<ContentFormat>,
    /// 替换全部标签。
    pub tags: Option<Vec<String>>,
    /// 新置顶状态。
    pub pinned: Option<bool>,
    /// 新归档状态。
    pub archived: Option<bool>,
    /// 替换应用扩展字段。
    pub meta: Option<serde_json::Value>,
}

/// 表示协议返回的语义块。
#[derive(Debug, Serialize)]
pub struct BlockResponse {
    /// 块标识。
    pub id: Uuid,
    /// 块顺序。
    pub seq: usize,
    /// heading、paragraph 等块类型。
    #[serde(rename = "type")]
    pub block_type: String,
    /// 块原文。
    pub text: String,
}

impl From<Block> for BlockResponse {
    /// 将核心块模型转换为协议模型。
    fn from(block: Block) -> Self {
        Self {
            id: block.id,
            seq: block.seq,
            block_type: block.block_type,
            text: block.text,
        }
    }
}

/// 表示读取、更新和列表接口返回的完整记忆。
#[derive(Debug, Serialize)]
pub struct MemoryResponse {
    /// 记忆标识。
    pub id: Uuid,
    /// 来源稳定字符串。
    pub source: String,
    /// 记忆类别。
    pub kind: MemoryKind,
    /// 可选标题。
    pub title: Option<String>,
    /// 正文。
    pub content: String,
    /// 正文格式。
    pub content_format: ContentFormat,
    /// 语义块。
    pub blocks: Vec<BlockResponse>,
    /// 标签。
    pub tags: Vec<String>,
    /// 是否置顶。
    pub pinned: bool,
    /// 是否归档。
    pub archived: bool,
    /// Unix 毫秒创建时间。
    pub created_at: i64,
    /// Unix 毫秒更新时间。
    pub updated_at: i64,
    /// 来源设备。
    pub device_id: String,
    /// 应用扩展字段。
    pub meta: serde_json::Value,
}

impl From<Memory> for MemoryResponse {
    /// 将核心记忆模型转换为协议稳定 JSON 结构。
    fn from(memory: Memory) -> Self {
        Self {
            id: memory.id,
            source: memory.source.as_storage_value(),
            kind: memory.kind,
            title: memory.title,
            content: memory.content,
            content_format: memory.content_format,
            blocks: memory.blocks.into_iter().map(BlockResponse::from).collect(),
            tags: memory.tags,
            pinned: memory.pinned,
            archived: memory.archived,
            created_at: memory.created_at,
            updated_at: memory.updated_at,
            device_id: memory.device_id,
            meta: memory.meta,
        }
    }
}

/// 表示 `GET /v1/memories` 的查询字符串。
#[derive(Debug, Default, Deserialize)]
pub struct ListMemoriesRequest {
    /// 逗号分隔的来源。
    pub source: Option<String>,
    /// 逗号分隔的记忆类别。
    pub kind: Option<String>,
    /// 逗号分隔的标签。
    pub tags: Option<String>,
    /// Unix 毫秒创建时间下界。
    pub created_from: Option<i64>,
    /// Unix 毫秒创建时间上界。
    pub created_to: Option<i64>,
    /// 单页条数。
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 分页偏移量。
    #[serde(default)]
    pub offset: usize,
}

/// 表示记忆分页列表响应。
#[derive(Debug, Serialize)]
pub struct ListMemoriesResponse {
    /// 当前页记忆。
    pub items: Vec<MemoryResponse>,
    /// 匹配总数。
    pub total: usize,
    /// 下一页偏移量；没有下一页时为 null。
    pub next_offset: Option<usize>,
}

/// 表示 `POST /v1/search` 的请求正文。
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// 用户查询文本。
    pub text: String,
    /// semantic、keyword 或 hybrid，默认 hybrid。
    #[serde(default = "default_search_mode")]
    pub mode: String,
    /// 可选来源、类别、标签和时间过滤。
    #[serde(default)]
    pub filters: SearchFiltersRequest,
    /// 最大返回条数，默认 10。
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// 表示检索请求中的过滤条件。
#[derive(Debug, Default, Deserialize)]
pub struct SearchFiltersRequest {
    /// 允许的来源。
    #[serde(default)]
    pub source: Vec<String>,
    /// 允许的记忆类别。
    #[serde(default)]
    pub kind: Vec<MemoryKind>,
    /// 至少匹配一个的标签。
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix 毫秒创建时间下界。
    pub created_from: Option<i64>,
    /// Unix 毫秒创建时间上界。
    pub created_to: Option<i64>,
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
