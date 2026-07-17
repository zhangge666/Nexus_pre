//! 本文件定义 Memory Protocol v1 的 JSON 请求与响应契约。

use nexus_core::{
    Block, ContentFormat, GradeResult, LinkCreator, LinkRelation, Memory, MemoryKind, Rating,
    ReviewPhase, ReviewStats,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 表示第一方本地应用请求最小 capability token 的登记信息。
#[derive(Debug, Deserialize)]
pub struct RegisterConnectionRequest {
    /// 稳定应用标识；M3 仅接受 Muse 桌面来源。
    pub app_id: String,
    /// 面向用户展示的应用名称。
    pub name: String,
    /// 令牌允许写入的统一来源。
    pub source: String,
    /// 应用申请的最小能力域。
    pub scopes: Vec<String>,
}

/// 表示登记成功后签发给本地应用的 capability token。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterConnectionResponse {
    /// 连接管理使用的令牌标识，不等同于敏感令牌正文。
    pub token_id: Uuid,
    /// 后续协议请求携带的 Bearer token。
    pub token: String,
    /// 实际授予的能力域。
    pub scopes: Vec<String>,
    /// 实际限制的可写来源。
    pub source: String,
}

/// 表示 Orbit 连接管理页面使用的已授权应用摘要。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedAppResponse {
    /// 稳定应用标识。
    pub id: String,
    /// 面向用户的应用名称。
    pub name: String,
    /// 令牌限定的来源。
    pub source: String,
    /// 已授予的能力域。
    pub scopes: Vec<String>,
    /// 最近成功鉴权的 Unix 毫秒时间。
    pub last_active_at: i64,
    /// 当前来源已经写入的记忆数量。
    pub memories_count: usize,
    /// 可撤销令牌的非敏感标识。
    pub token_id: Uuid,
}

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
    /// 信息实际发生时间；省略时表示与写入时间无独立记录。
    pub captured_at: Option<i64>,
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
    /// 新的信息实际发生时间。
    pub captured_at: Option<i64>,
    /// 替换应用扩展字段。
    pub meta: Option<serde_json::Value>,
}

/// 表示协议返回的语义块。
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Deserialize, Serialize)]
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
    /// Unix 毫秒信息实际发生时间。
    pub captured_at: Option<i64>,
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
            captured_at: memory.captured_at,
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
#[derive(Debug, Deserialize, Serialize)]
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

/// 表示 RAG 问答可选的集合或来源收窄范围。
#[derive(Debug, Default, Deserialize)]
pub struct AskScopeRequest {
    /// 集合 UUID 或精确集合名称。
    pub collection: Option<String>,
    /// 单一允许来源。
    pub source: Option<String>,
}

/// 表示 `POST /v1/ask` 的问题和可选本地检索范围。
#[derive(Debug, Deserialize)]
pub struct AskRequest {
    /// 用户对记忆库提出的问题。
    pub question: String,
    /// 可选集合或来源范围。
    #[serde(default)]
    pub scope: Option<AskScopeRequest>,
}

/// 表示 RAG 回答中的一条可回跳块级引用。
#[derive(Debug, Deserialize, Serialize)]
pub struct CitationResponse {
    /// 来源记忆标识。
    pub memory_id: Uuid,
    /// 实际命中的语义块标识。
    pub block_id: Uuid,
    /// 发送给 Completion 的截断片段。
    pub snippet: String,
    /// 来源记忆标题。
    pub source_title: Option<String>,
    /// 来源记忆类别。
    pub source_kind: MemoryKind,
    /// 来源记忆创建时间。
    pub created_at: i64,
}

/// 表示带引用和数据流向元数据的 RAG 回答。
#[derive(Debug, Deserialize, Serialize)]
pub struct AskResponse {
    /// Completion 生成或本地抽取的回答。
    pub answer: String,
    /// 回答所依据的本地块级引用。
    pub citations: Vec<CitationResponse>,
    /// 实际执行请求的 Provider 标识。
    pub provider: String,
    /// 本次发给 Completion 的片段数量。
    pub sent_context_count: usize,
    /// 本次 Provider 是否会把片段发送到远程端点。
    pub sends_data_remote: bool,
}

/// 表示管理员切换 Completion Provider 所需的进程内配置。
#[derive(Debug, Deserialize)]
pub struct CompletionConfigRequest {
    /// local、claude、openai 或 custom。
    pub provider: String,
    /// 云 Provider 自带 Key；仅驻留进程内存。
    pub api_key: Option<String>,
    /// 模型标识。
    pub model: Option<String>,
    /// Claude、OpenAI 或自定义兼容端点的可选覆盖地址。
    pub endpoint: Option<String>,
}

/// 表示当前激活 Completion Provider 的非敏感状态。
#[derive(Debug, Deserialize, Serialize)]
pub struct CompletionStatusResponse {
    /// 当前实际 Provider 标识。
    pub provider: String,
    /// 当前 Provider 是否会发送最小上下文到远程端点。
    pub sends_data_remote: bool,
}

/// 表示手动创建知识卡片所需字段。
#[derive(Debug, Deserialize)]
pub struct CreateCardRequest {
    /// 卡片正面。
    pub card_front: String,
    /// 卡片背面。
    pub card_back: String,
    /// 可选来源记忆。
    pub source_memory_id: Option<Uuid>,
    /// 可选复习集。
    pub deck: Option<String>,
    /// 卡片标签。
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 表示从一条来源记忆生成卡片的请求。
#[derive(Debug, Deserialize)]
pub struct GenerateCardsRequest {
    /// 卡片派生来源。
    pub source_memory_id: Uuid,
    /// 可选补充生成指令。
    pub instruction: Option<String>,
    /// 可选复习集。
    pub deck: Option<String>,
    /// 生成卡片标签。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 一次最多创建的卡片数，协议上限为 10。
    #[serde(default = "default_generated_card_limit")]
    pub max_cards: usize,
}

/// 表示协议返回的完整复习卡片和溯源摘要。
#[derive(Debug, Deserialize, Serialize)]
pub struct ReviewCardResponse {
    /// 对应卡片 Memory 标识。
    pub memory_id: Uuid,
    /// 卡片正面。
    pub card_front: String,
    /// 卡片背面。
    pub card_back: String,
    /// 当前学习阶段。
    pub state: ReviewPhase,
    /// FSRS 稳定度，单位为天。
    pub stability: f64,
    /// FSRS 难度。
    pub difficulty: f64,
    /// 下次到期 Unix 毫秒时间。
    pub due_at: i64,
    /// 最近评分时间。
    pub last_reviewed_at: Option<i64>,
    /// 累计评分次数。
    pub reps: u32,
    /// 累计遗忘次数。
    pub lapses: u32,
    /// 派生来源记忆标识。
    pub source_memory_id: Option<Uuid>,
    /// 派生来源标题。
    pub source_title: Option<String>,
    /// 可选复习集。
    pub deck: Option<String>,
    /// 卡片标签。
    pub tags: Vec<String>,
}

/// 表示一次复习评分请求。
#[derive(Debug, Deserialize)]
pub struct GradeReviewRequest {
    /// Again、Hard、Good 或 Easy。
    pub rating: Rating,
    /// 可选客户端评分时间；省略时使用服务端当前时间。
    pub reviewed_at: Option<i64>,
}

/// 表示协议返回的评分调度结果。
#[derive(Debug, Serialize)]
pub struct GradeReviewResponse {
    /// 下次到期时间。
    pub next_due_at: i64,
    /// 新稳定度。
    pub new_stability: f64,
    /// 新难度。
    pub new_difficulty: f64,
    /// 新学习阶段。
    pub new_state: ReviewPhase,
}

impl From<GradeResult> for GradeReviewResponse {
    /// 将核心评分结果转换为稳定协议响应。
    fn from(result: GradeResult) -> Self {
        Self {
            next_due_at: result.next_due_at,
            new_stability: result.new_stability,
            new_difficulty: result.new_difficulty,
            new_state: result.new_state,
        }
    }
}

/// 表示协议复习统计响应。
pub type ReviewStatsResponse = ReviewStats;

/// 表示显式扫描到期卡片后的通知数量。
#[derive(Debug, Serialize)]
pub struct NotifyDueResponse {
    /// 本轮首次发布 `review.due` 的卡片数。
    pub notified: usize,
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

/// 表示创建记忆关联的协议请求。
#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    /// 源记忆标识。
    pub from_id: Uuid,
    /// 目标记忆标识。
    pub to_id: Uuid,
    /// 关联类型。
    pub relation: LinkRelation,
    /// 创建主体，手动请求默认为用户。
    #[serde(default = "default_link_creator")]
    pub created_by: LinkCreator,
}

/// 表示按参与记忆筛选关联的查询。
#[derive(Debug, Deserialize)]
pub struct ListLinksRequest {
    /// 作为源或目标参与关联的记忆标识。
    pub memory_id: Uuid,
}

/// 表示创建集合的协议请求。
#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    /// 集合名称。
    pub name: String,
    /// 可选图标标识。
    pub icon: Option<String>,
    /// 可选父集合。
    pub parent_id: Option<Uuid>,
    /// 同级排序值。
    #[serde(default)]
    pub sort: i64,
}

/// 表示更新集合的协议请求。
#[derive(Debug, Default, Deserialize)]
pub struct UpdateCollectionRequest {
    /// 新名称。
    pub name: Option<String>,
    /// 新图标；清除图标使用 `clear_icon`。
    pub icon: Option<String>,
    /// 是否清除图标。
    #[serde(default)]
    pub clear_icon: bool,
    /// 新父集合；移动到根级使用 `move_to_root`。
    pub parent_id: Option<Uuid>,
    /// 是否移动到根级。
    #[serde(default)]
    pub move_to_root: bool,
    /// 新排序值。
    pub sort: Option<i64>,
}

/// 返回手动创建关联时使用的默认主体。
const fn default_link_creator() -> LinkCreator {
    LinkCreator::User
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

/// 返回单次 AI 卡片生成的保守默认上限。
const fn default_generated_card_limit() -> usize {
    3
}
