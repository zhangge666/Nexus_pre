//! 本文件定义开发文档中的统一 Memory、Block、写入输入和检索结果模型。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 表示记忆内容的来源应用或外部客户端。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySource {
    /// Echo 屏幕记忆。
    Echo,
    /// Muse 灵感捕捉。
    Muse,
    /// Quill 笔记系统。
    Quill,
    /// Orbit 记忆中枢。
    Orbit,
    /// 经过授权的外部应用。
    External(String),
}

impl MemorySource {
    /// 将来源转换为协议与数据库共用的稳定字符串。
    #[must_use]
    pub fn as_storage_value(&self) -> String {
        match self {
            Self::Echo => "echo".into(),
            Self::Muse => "muse".into(),
            Self::Quill => "quill".into(),
            Self::Orbit => "orbit".into(),
            Self::External(app_id) => format!("external:{app_id}"),
        }
    }

    /// 从数据库或协议稳定字符串恢复来源类型。
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "echo" => Some(Self::Echo),
            "muse" => Some(Self::Muse),
            "quill" => Some(Self::Quill),
            "orbit" => Some(Self::Orbit),
            external if external.starts_with("external:") && external.len() > 9 => {
                Some(Self::External(external[9..].to_owned()))
            }
            _ => None,
        }
    }
}

/// 表示统一记忆模型支持的内容类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// 屏幕捕获内容。
    Screen,
    /// 长短笔记。
    Note,
    /// 灵感速记。
    Idea,
    /// 语音记录。
    Voice,
    /// 知识卡片。
    Card,
    /// 外部剪藏。
    Clip,
    /// 文件记忆。
    File,
}

/// 表示正文在数据库与协议中的编码格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFormat {
    /// Markdown 文本。
    Markdown,
    /// 无标记纯文本。
    Plain,
    /// JSON 结构化内容。
    Json,
}

/// 表示供写入管线消费的原始记忆输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestInput {
    /// 内容来源。
    pub source: MemorySource,
    /// 内容类别。
    pub kind: MemoryKind,
    /// 可选标题。
    pub title: Option<String>,
    /// 完整正文。
    pub content: String,
    /// 正文编码格式。
    pub content_format: ContentFormat,
    /// 用户标签。
    pub tags: Vec<String>,
    /// 来源设备标识。
    pub device_id: String,
    /// 应用自有扩展字段。
    pub meta: serde_json::Value,
}

/// 表示正文切分后用于检索和引用的语义块。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// 块的全局标识。
    pub id: Uuid,
    /// 所属记忆标识。
    pub memory_id: Uuid,
    /// 块在正文中的顺序。
    pub seq: usize,
    /// 块类型，例如 heading 或 paragraph。
    pub block_type: String,
    /// 用于检索和引用的文本。
    pub text: String,
}

/// 表示已经完成写入并可供协议返回的记忆。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// UUID v7 全局标识。
    pub id: Uuid,
    /// 内容来源。
    pub source: MemorySource,
    /// 内容类别。
    pub kind: MemoryKind,
    /// 可选标题。
    pub title: Option<String>,
    /// Markdown、纯文本或 JSON 正文。
    pub content: String,
    /// 正文编码格式。
    pub content_format: ContentFormat,
    /// 已生成的语义块。
    pub blocks: Vec<Block>,
    /// 用户标签。
    pub tags: Vec<String>,
    /// 是否置顶。
    pub pinned: bool,
    /// 是否归档。
    pub archived: bool,
    /// Unix 毫秒创建时间。
    pub created_at: i64,
    /// Unix 毫秒更新时间。
    pub updated_at: i64,
    /// 来源设备标识。
    pub device_id: String,
    /// 应用扩展字段。
    pub meta: serde_json::Value,
}

/// 表示对现有记忆执行的字段级更新。
#[derive(Debug, Clone, Default)]
pub struct MemoryPatch {
    /// `None` 表示不修改，`Some(None)` 表示清除标题。
    pub title: Option<Option<String>>,
    /// 替换正文并触发重新切块与嵌入。
    pub content: Option<String>,
    /// 替换正文编码格式。
    pub content_format: Option<ContentFormat>,
    /// 替换全部标签。
    pub tags: Option<Vec<String>>,
    /// 修改置顶状态。
    pub pinned: Option<bool>,
    /// 修改归档状态。
    pub archived: Option<bool>,
    /// 替换应用扩展字段。
    pub meta: Option<serde_json::Value>,
}

impl MemoryPatch {
    /// 判断补丁是否没有任何字段需要修改。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.content.is_none()
            && self.content_format.is_none()
            && self.tags.is_none()
            && self.pinned.is_none()
            && self.archived.is_none()
            && self.meta.is_none()
    }
}

/// 表示记忆列表和检索共用的过滤条件。
#[derive(Debug, Clone, Default)]
pub struct MemoryFilters {
    /// 允许的来源稳定字符串；空集合表示不限来源。
    pub sources: Vec<String>,
    /// 允许的记忆类别；空集合表示不限类别。
    pub kinds: Vec<MemoryKind>,
    /// 至少匹配其中一个标签；空集合表示不限标签。
    pub tags: Vec<String>,
    /// Unix 毫秒创建时间下界。
    pub created_from: Option<i64>,
    /// Unix 毫秒创建时间上界。
    pub created_to: Option<i64>,
}

/// 表示按时间倒序读取记忆列表的请求。
#[derive(Debug, Clone)]
pub struct ListQuery {
    /// 来源、类别、标签和时间条件。
    pub filters: MemoryFilters,
    /// 单页最大条数。
    pub limit: usize,
    /// 从匹配结果中跳过的条数。
    pub offset: usize,
}

/// 表示带总数和下一页偏移量的记忆列表。
#[derive(Debug, Clone)]
pub struct MemoryPage {
    /// 当前页记忆。
    pub items: Vec<Memory>,
    /// 过滤后总条数。
    pub total: usize,
    /// 存在下一页时返回新的偏移量。
    pub next_offset: Option<usize>,
}

/// 表示调用方选择的检索策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// 仅使用向量相似度。
    Semantic,
    /// 仅使用 FTS5 全文检索。
    Keyword,
    /// 使用 RRF 融合向量与全文结果。
    Hybrid,
}

/// 表示一次记忆检索请求。
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// 用户查询文本。
    pub text: String,
    /// 检索模式。
    pub mode: SearchMode,
    /// 来源、类别、标签和时间过滤条件。
    pub filters: MemoryFilters,
    /// 最大返回条数。
    pub limit: usize,
}

/// 表示块级检索命中并回溯到记忆后的结果。
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// 命中的记忆标识。
    pub memory_id: Uuid,
    /// 命中的语义块标识。
    pub block_id: Uuid,
    /// RRF 或单路归一化分数。
    pub score: f32,
    /// 命中块原文。
    pub snippet: String,
}
