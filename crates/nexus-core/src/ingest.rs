//! 本文件实现原始输入的校验、Markdown 切块、嵌入和事务落库管线。

use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::{Block, CoreError, Embedder, IngestInput, Memory, MemoryStore, Result};

/// 编排统一记忆写入流程。
pub struct Ingestor<'a, E: Embedder> {
    store: &'a MemoryStore,
    embedder: &'a E,
}

impl<'a, E: Embedder> Ingestor<'a, E> {
    /// 使用指定存储和嵌入器创建写入管线。
    #[must_use]
    pub const fn new(store: &'a MemoryStore, embedder: &'a E) -> Self {
        Self { store, embedder }
    }

    /// 校验输入并完成切块、向量化和原子落库。
    pub fn ingest(&self, input: IngestInput) -> Result<Memory> {
        if input.content.trim().is_empty() {
            return Err(CoreError::InvalidInput("记忆正文不能为空".into()));
        }
        if input.device_id.trim().is_empty() {
            return Err(CoreError::InvalidInput("来源设备不能为空".into()));
        }

        let id = Uuid::now_v7();
        let timestamp = current_timestamp_millis()?;
        let blocks = split_into_blocks(id, &input.content);
        let embeddings = blocks
            .iter()
            .map(|block| self.embedder.embed(&block.text))
            .collect::<Vec<_>>();
        let memory = Memory {
            id,
            source: input.source,
            kind: input.kind,
            title: input.title,
            content: input.content,
            content_format: input.content_format,
            blocks,
            tags: input.tags,
            pinned: false,
            archived: false,
            created_at: timestamp,
            updated_at: timestamp,
            device_id: input.device_id,
            meta: input.meta,
        };
        self.store.create(&memory, &embeddings)?;
        Ok(memory)
    }
}

/// 将 Markdown 按空行切分，并识别标题块；纯文本也会得到至少一个段落块。
fn split_into_blocks(memory_id: Uuid, content: &str) -> Vec<Block> {
    content
        .split("\n\n")
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .enumerate()
        .map(|(seq, text)| Block {
            id: Uuid::now_v7(),
            memory_id,
            seq,
            block_type: if text.starts_with('#') {
                "heading"
            } else {
                "paragraph"
            }
            .into(),
            text: text.to_owned(),
        })
        .collect()
}

/// 返回数据库统一使用的 Unix 毫秒时间戳。
fn current_timestamp_millis() -> Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoreError::InvalidSystemTime)?;
    i64::try_from(duration.as_millis()).map_err(|_| CoreError::InvalidSystemTime)
}
