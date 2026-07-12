//! 本文件实现 SQLite 连接、版本迁移以及 Memory 与检索索引的事务写入。

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::{CoreError, CoreEvent, EventSubscription, Memory, Result, events::EventBus};

const MIGRATION_V1: &str = include_str!("../migrations/0001_initial.sql");
const MIGRATION_V2: &str = include_str!("../migrations/0002_media.sql");
const MIGRATION_V3: &str = include_str!("../migrations/0003_vector_index.sql");
const MIGRATION_V4: &str = include_str!("../migrations/0004_organization.sql");

/// 持有单写者 SQLite 连接并在创建时自动执行迁移。
pub struct MemoryStore {
    connection: Mutex<Connection>,
    pub(crate) events: EventBus,
}

impl MemoryStore {
    /// 打开文件数据库、启用 WAL 和外键，并执行缺失迁移。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        register_vector_extension()?;
        Self::from_connection(Connection::open(path)?)
    }

    /// 创建用于测试或临时会话的内存数据库。
    pub fn open_in_memory() -> Result<Self> {
        register_vector_extension()?;
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// 将已打开的连接配置为 Nexus 工作库。
    fn from_connection(connection: Connection) -> Result<Self> {
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let store = Self {
            connection: Mutex::new(connection),
            events: EventBus::default(),
        };
        store.migrate()?;
        Ok(store)
    }

    /// 在同一事务中写入记忆、语义块、向量、全文索引与标签。
    pub fn create(&self, memory: &Memory, embeddings: &[Vec<f32>]) -> Result<()> {
        if memory.blocks.len() != embeddings.len() {
            return Err(CoreError::InvalidInput("语义块与嵌入向量数量不一致".into()));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO memories (id, source, kind, title, content, content_format, pinned, archived, created_at, updated_at, captured_at, device_id, meta) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![memory.id.to_string(), memory.source.as_storage_value(), enum_json(&memory.kind)?, memory.title, memory.content, enum_json(&memory.content_format)?, memory.pinned, memory.archived, memory.created_at, memory.updated_at, memory.captured_at, memory.device_id, memory.meta.to_string()],
        )?;

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
        for tag in &memory.tags {
            transaction.execute(
                "INSERT INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
                params![memory.id.to_string(), tag],
            )?;
        }
        transaction.commit()?;
        self.events.publish(CoreEvent::MemoryCreated {
            id: memory.id,
            source: memory.source.as_storage_value(),
        })?;
        Ok(())
    }

    /// 订阅事务提交后的记忆创建、更新和删除事件。
    pub fn subscribe(&self) -> Result<EventSubscription> {
        self.events.subscribe()
    }

    /// 获取内部连接锁，统一处理互斥锁污染错误。
    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| CoreError::StoreUnavailable)
    }

    /// 按顺序执行尚未应用的数据库迁移。
    fn migrate(&self) -> Result<()> {
        let connection = self.connection()?;
        // 首版迁移负责建立版本表；后续迁移只在版本尚未登记时执行。
        connection.execute_batch(MIGRATION_V1)?;
        let version = connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if version < 2 {
            connection.execute_batch(MIGRATION_V2)?;
        }
        if version < 3 {
            connection.execute_batch(MIGRATION_V3)?;
        }
        if version < 4 {
            connection.execute_batch(MIGRATION_V4)?;
        }
        Ok(())
    }
}

/// 注册 sqlite-vec 自动扩展并映射原生错误码。
fn register_vector_extension() -> Result<()> {
    nexus_sqlite_vec::register().map_err(CoreError::VectorExtension)
}

/// 使用 serde 的 snake_case 规则把模型枚举转换为稳定数据库值。
pub(crate) fn enum_json<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_owned())
}

/// 将数据库文本标识转换为 UUID，并把无效历史数据映射为输入错误。
pub(crate) fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| CoreError::InvalidInput(format!("数据库 UUID 无效: {error}")))
}
