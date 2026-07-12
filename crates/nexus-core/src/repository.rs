//! 本文件实现 Memory 的读取、更新、删除、分页列表和过滤匹配。

use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{
    Block, CoreError, CoreEvent, Embedder, ListQuery, Memory, MemoryFilters, MemoryPage,
    MemoryPatch, MemorySource, MemoryStore, Result,
    ingest::{current_timestamp_millis, split_into_blocks},
    store::{enum_json, parse_uuid},
};

impl MemoryStore {
    /// 按 UUID 读取完整记忆及其块和标签。
    pub fn get(&self, id: &Uuid) -> Result<Option<Memory>> {
        let connection = self.connection()?;
        load_memory(&connection, id)
    }

    /// 应用字段补丁，并在正文改变时重建块、全文索引和嵌入向量。
    pub fn update<E: Embedder + ?Sized>(
        &self,
        id: &Uuid,
        patch: MemoryPatch,
        embedder: &E,
    ) -> Result<Memory> {
        let mut memory = self.get(id)?.ok_or(CoreError::NotFound(*id))?;
        if patch.is_empty() {
            return Ok(memory);
        }

        let content_changed = patch.content.is_some();
        if let Some(title) = patch.title {
            memory.title = title;
        }
        if let Some(content) = patch.content {
            if content.trim().is_empty() {
                return Err(CoreError::InvalidInput("记忆正文不能为空".into()));
            }
            memory.content = content;
        }
        if let Some(content_format) = patch.content_format {
            memory.content_format = content_format;
        }
        if let Some(tags) = patch.tags {
            memory.tags = tags;
        }
        if let Some(pinned) = patch.pinned {
            memory.pinned = pinned;
        }
        if let Some(archived) = patch.archived {
            memory.archived = archived;
        }
        if let Some(meta) = patch.meta {
            memory.meta = meta;
        }
        memory.updated_at = current_timestamp_millis()?;

        let embeddings = if content_changed {
            memory.blocks = split_into_blocks(memory.id, &memory.content);
            memory
                .blocks
                .iter()
                .map(|block| embedder.embed(&block.text))
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE memories SET title=?2, content=?3, content_format=?4, pinned=?5, archived=?6, updated_at=?7, meta=?8 WHERE id=?1",
            params![memory.id.to_string(), memory.title, memory.content, enum_json(&memory.content_format)?, memory.pinned, memory.archived, memory.updated_at, memory.meta.to_string()],
        )?;

        if content_changed {
            transaction.execute(
                "DELETE FROM blocks_fts WHERE memory_id=?1",
                params![memory.id.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM block_vectors_vec WHERE block_id IN (SELECT id FROM blocks WHERE memory_id=?1)",
                params![memory.id.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM blocks WHERE memory_id=?1",
                params![memory.id.to_string()],
            )?;
            insert_blocks(&transaction, &memory, &embeddings)?;
        }

        transaction.execute(
            "DELETE FROM memory_tags WHERE memory_id=?1",
            params![memory.id.to_string()],
        )?;
        for tag in &memory.tags {
            transaction.execute(
                "INSERT INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
                params![memory.id.to_string(), tag],
            )?;
        }
        transaction.commit()?;
        self.events.publish(CoreEvent::MemoryUpdated { id: *id })?;
        Ok(memory)
    }

    /// 级联删除记忆、块、向量、标签和全文索引。
    pub fn delete(&self, id: &Uuid) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM block_vectors_vec WHERE block_id IN (SELECT id FROM blocks WHERE memory_id=?1)",
            params![id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM blocks_fts WHERE memory_id=?1",
            params![id.to_string()],
        )?;
        let affected =
            transaction.execute("DELETE FROM memories WHERE id=?1", params![id.to_string()])?;
        if affected == 0 {
            return Err(CoreError::NotFound(*id));
        }
        transaction.commit()?;
        self.events.publish(CoreEvent::MemoryDeleted { id: *id })?;
        Ok(())
    }

    /// 按创建时间倒序返回经过过滤的分页记忆。
    pub fn list(&self, query: &ListQuery) -> Result<MemoryPage> {
        let ids = {
            let connection = self.connection()?;
            let mut statement =
                connection.prepare("SELECT id FROM memories ORDER BY created_at DESC, id DESC")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut matching = Vec::new();
        for encoded_id in ids {
            let id = parse_uuid(&encoded_id)?;
            if let Some(memory) = self.get(&id)?
                && memory_matches_filters(&memory, &query.filters)
            {
                matching.push(memory);
            }
        }
        let total = matching.len();
        let limit = query.limit.min(100);
        let items = matching
            .into_iter()
            .skip(query.offset)
            .take(limit)
            .collect::<Vec<_>>();
        let consumed = query.offset.saturating_add(items.len());
        Ok(MemoryPage {
            items,
            total,
            next_offset: (consumed < total).then_some(consumed),
        })
    }

    /// 判断指定记忆是否满足来源、类别、标签和时间过滤条件。
    pub(crate) fn matches_filters(&self, id: &Uuid, filters: &MemoryFilters) -> Result<bool> {
        Ok(self
            .get(id)?
            .is_some_and(|memory| memory_matches_filters(&memory, filters)))
    }
}

/// 从同一连接加载记忆主记录、块和标签。
fn load_memory(connection: &Connection, id: &Uuid) -> Result<Option<Memory>> {
    let stored = connection
        .query_row(
            "SELECT source, kind, title, content, content_format, pinned, archived, created_at, updated_at, device_id, meta FROM memories WHERE id=?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .optional()?;
    let Some((
        source,
        kind,
        title,
        content,
        content_format,
        pinned,
        archived,
        created_at,
        updated_at,
        device_id,
        meta,
    )) = stored
    else {
        return Ok(None);
    };

    let mut block_statement = connection
        .prepare("SELECT id, seq, type, text FROM blocks WHERE memory_id=?1 ORDER BY seq ASC")?;
    let blocks = block_statement
        .query_map(params![id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, usize>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .map(|row| {
            let (block_id, seq, block_type, text) = row?;
            Ok(Block {
                id: parse_uuid(&block_id)?,
                memory_id: *id,
                seq,
                block_type,
                text,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut tag_statement =
        connection.prepare("SELECT tag FROM memory_tags WHERE memory_id=?1 ORDER BY tag ASC")?;
    let tags = tag_statement
        .query_map(params![id.to_string()], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Some(Memory {
        id: *id,
        source: MemorySource::from_storage_value(&source)
            .ok_or_else(|| CoreError::InvalidInput(format!("数据库来源无效: {source}")))?,
        kind: parse_enum(&kind)?,
        title,
        content,
        content_format: parse_enum(&content_format)?,
        blocks,
        tags,
        pinned,
        archived,
        created_at,
        updated_at,
        device_id,
        meta: serde_json::from_str(&meta)?,
    }))
}

/// 在更新事务中重建语义块、向量与全文索引。
fn insert_blocks(
    transaction: &rusqlite::Transaction<'_>,
    memory: &Memory,
    embeddings: &[Vec<f32>],
) -> Result<()> {
    for (block, embedding) in memory.blocks.iter().zip(embeddings) {
        transaction.execute(
            "INSERT INTO blocks (id, memory_id, seq, type, text) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                block.id.to_string(),
                memory.id.to_string(),
                block.seq,
                block.block_type,
                block.text
            ],
        )?;
        transaction.execute(
            "INSERT INTO block_vectors (block_id, memory_id, embedding) VALUES (?1, ?2, ?3)",
            params![
                block.id.to_string(),
                memory.id.to_string(),
                serde_json::to_string(embedding)?
            ],
        )?;
        transaction.execute(
            "INSERT INTO block_vectors_vec (block_id, embedding) VALUES (?1, ?2)",
            params![block.id.to_string(), serde_json::to_string(embedding)?],
        )?;
        transaction.execute(
            "INSERT INTO blocks_fts (memory_id, block_id, text) VALUES (?1, ?2, ?3)",
            params![memory.id.to_string(), block.id.to_string(), block.text],
        )?;
    }
    Ok(())
}

/// 判断一条完整记忆是否满足过滤条件。
fn memory_matches_filters(memory: &Memory, filters: &MemoryFilters) -> bool {
    (filters.sources.is_empty() || filters.sources.contains(&memory.source.as_storage_value()))
        && (filters.kinds.is_empty() || filters.kinds.contains(&memory.kind))
        && (filters.tags.is_empty() || filters.tags.iter().any(|tag| memory.tags.contains(tag)))
        && filters
            .created_from
            .is_none_or(|from| memory.created_at >= from)
        && filters.created_to.is_none_or(|to| memory.created_at <= to)
}

/// 从 snake_case 数据库值恢复 serde 枚举。
fn parse_enum<T: DeserializeOwned>(value: &str) -> Result<T> {
    Ok(serde_json::from_str(&format!("\"{value}\""))?)
}
