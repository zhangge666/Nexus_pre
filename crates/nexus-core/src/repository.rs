//! 本文件实现 Memory 的读取、更新、删除、分页列表和过滤匹配。

use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
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
        if let Some(captured_at) = patch.captured_at {
            memory.captured_at = captured_at;
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
            "UPDATE memories SET title=?2, content=?3, content_format=?4, pinned=?5, archived=?6, updated_at=?7, meta=?8, captured_at=?9 WHERE id=?1",
            params![memory.id.to_string(), memory.title, memory.content, enum_json(&memory.content_format)?, memory.pinned, memory.archived, memory.updated_at, memory.meta.to_string(), memory.captured_at],
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
        self.events.publish(CoreEvent::MemoryUpdated {
            id: *id,
            source: memory.source.as_storage_value(),
        })?;
        Ok(memory)
    }

    /// 级联删除记忆、块、向量、标签和全文索引。
    pub fn delete(&self, id: &Uuid) -> Result<()> {
        let source = self
            .get(id)?
            .ok_or(CoreError::NotFound(*id))?
            .source
            .as_storage_value();
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
        self.events
            .publish(CoreEvent::MemoryDeleted { id: *id, source })?;
        Ok(())
    }

    /// 按创建时间倒序返回经过过滤的分页记忆。
    pub fn list(&self, query: &ListQuery) -> Result<MemoryPage> {
        let (where_clause, filter_values) = memory_filter_clause(&query.filters)?;
        let limit = query.limit.min(100);
        let (ids, total) = {
            let connection = self.connection()?;
            let total = connection.query_row(
                &format!("SELECT COUNT(*) FROM memories m{where_clause}"),
                params_from_iter(filter_values.iter()),
                |row| row.get::<_, usize>(0),
            )?;
            let mut page_values = filter_values.clone();
            page_values.push(Value::Integer(limit as i64));
            page_values.push(Value::Integer(query.offset as i64));
            let mut statement = connection.prepare(&format!(
                "SELECT m.id FROM memories m{where_clause} ORDER BY m.created_at DESC, m.id DESC LIMIT ? OFFSET ?"
            ))?;
            let ids = statement
                .query_map(params_from_iter(page_values.iter()), |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            (ids, total)
        };
        let mut items = Vec::with_capacity(ids.len());
        for encoded_id in ids {
            let id = parse_uuid(&encoded_id)?;
            if let Some(memory) = self.get(&id)? {
                items.push(memory);
            }
        }
        let consumed = query.offset.saturating_add(items.len());
        Ok(MemoryPage {
            items,
            total,
            next_offset: (consumed < total).then_some(consumed),
        })
    }

    /// 判断指定记忆是否满足来源、类别、标签和时间过滤条件。
    pub(crate) fn matches_filters(&self, id: &Uuid, filters: &MemoryFilters) -> Result<bool> {
        let (where_clause, filter_values) = memory_filter_clause(filters)?;
        let predicate = if where_clause.is_empty() {
            " WHERE m.id = ?".to_owned()
        } else {
            where_clause.replacen(" WHERE ", " WHERE m.id = ? AND ", 1)
        };
        let mut values = vec![Value::Text(id.to_string())];
        values.extend(filter_values);
        let connection = self.connection()?;
        connection
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM memories m{predicate})"),
                params_from_iter(values.iter()),
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}

/// 构造记忆列表使用的参数化 SQL 过滤条件，避免把全表加载到内存。
fn memory_filter_clause(filters: &MemoryFilters) -> Result<(String, Vec<Value>)> {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    if !filters.sources.is_empty() {
        clauses.push(format!(
            "m.source IN ({})",
            placeholders(filters.sources.len())
        ));
        values.extend(filters.sources.iter().cloned().map(Value::Text));
    }
    if !filters.kinds.is_empty() {
        clauses.push(format!("m.kind IN ({})", placeholders(filters.kinds.len())));
        for kind in &filters.kinds {
            values.push(Value::Text(enum_json(kind)?));
        }
    }
    if !filters.tags.is_empty() {
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM memory_tags mt WHERE mt.memory_id=m.id AND mt.tag IN ({}))",
            placeholders(filters.tags.len())
        ));
        values.extend(filters.tags.iter().cloned().map(Value::Text));
    }
    if let Some(from) = filters.created_from {
        clauses.push("m.created_at >= ?".into());
        values.push(Value::Integer(from));
    }
    if let Some(to) = filters.created_to {
        clauses.push("m.created_at <= ?".into());
        values.push(Value::Integer(to));
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    Ok((sql, values))
}

/// 返回指定数量的匿名 SQL 参数占位符。
fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

/// 从同一连接加载记忆主记录、块和标签。
fn load_memory(connection: &Connection, id: &Uuid) -> Result<Option<Memory>> {
    let stored = connection
        .query_row(
            "SELECT source, kind, title, content, content_format, pinned, archived, created_at, updated_at, captured_at, device_id, meta FROM memories WHERE id=?1",
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
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
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
        captured_at,
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
        captured_at,
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

/// 从 snake_case 数据库值恢复 serde 枚举。
fn parse_enum<T: DeserializeOwned>(value: &str) -> Result<T> {
    Ok(serde_json::from_str(&format!("\"{value}\""))?)
}
