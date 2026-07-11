//! 本文件将加密媒体仓库与 SQLite 媒体引用、去重和级联清理连接起来。

use std::path::PathBuf;

use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use crate::{
    CoreError, MediaKind, MediaMetadata, MediaRecord, MediaVault, MemoryStore, Result,
    store::{enum_json, parse_uuid},
};

/// 编排媒体加密文件与 Memory 数据库引用的一致性操作。
pub struct MediaService<'a> {
    store: &'a MemoryStore,
    vault: &'a MediaVault,
}

impl<'a> MediaService<'a> {
    /// 使用统一工作库和加密媒体仓库创建服务。
    #[must_use]
    pub const fn new(store: &'a MemoryStore, vault: &'a MediaVault) -> Self {
        Self { store, vault }
    }

    /// 加密媒体、按内容哈希去重，并关联到指定 Memory。
    pub fn attach(
        &self,
        memory_id: &Uuid,
        kind: MediaKind,
        plaintext: &[u8],
        mime: impl Into<String>,
        metadata: MediaMetadata,
    ) -> Result<MediaRecord> {
        if self.store.get(memory_id)?.is_none() {
            return Err(CoreError::NotFound(*memory_id));
        }
        let mime = mime.into();
        let encrypted = self
            .vault
            .put(plaintext, mime.clone())
            .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
        let mut connection = self.store.connection()?;
        let transaction = connection.transaction()?;
        let existing_id = transaction
            .query_row(
                "SELECT id FROM media WHERE hash=?1",
                params![encrypted.hash],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let media_id = existing_id
            .as_deref()
            .map(parse_uuid)
            .transpose()?
            .unwrap_or_else(Uuid::now_v7);
        transaction.execute(
            "INSERT OR IGNORE INTO media (id, kind, path, mime, width, height, duration_ms, ocr_text, transcript, hash, size) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![media_id.to_string(), enum_json(&kind)?, encrypted.path.to_string_lossy(), mime, metadata.width, metadata.height, metadata.duration_ms, metadata.ocr_text, metadata.transcript, encrypted.hash, encrypted.size],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO memory_media (memory_id, media_id) VALUES (?1, ?2)",
            params![memory_id.to_string(), media_id.to_string()],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get(&media_id)?
            .ok_or(CoreError::InvalidInput("媒体元数据写入失败".into()))
    }

    /// 返回指定 Memory 关联的全部媒体。
    pub fn list(&self, memory_id: &Uuid) -> Result<Vec<MediaRecord>> {
        let connection = self.store.connection()?;
        let mut statement = connection.prepare(
            "SELECT m.id FROM media m JOIN memory_media mm ON mm.media_id=m.id WHERE mm.memory_id=?1 ORDER BY m.id",
        )?;
        let ids = statement
            .query_map(params![memory_id.to_string()], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        ids.into_iter()
            .map(|id| {
                parse_uuid(&id).and_then(|id| {
                    load_media(&connection, &id)?
                        .ok_or(CoreError::InvalidInput("媒体引用失效".into()))
                })
            })
            .collect()
    }

    /// 删除 Memory，并在媒体没有其他引用时删除加密文件和元数据。
    pub fn delete_memory(&self, memory_id: &Uuid) -> Result<()> {
        let media = self.list(memory_id)?;
        self.store.delete(memory_id)?;
        for record in media {
            let remaining = {
                let connection = self.store.connection()?;
                connection.query_row(
                    "SELECT COUNT(*) FROM memory_media WHERE media_id=?1",
                    params![record.id.to_string()],
                    |row| row.get::<_, u64>(0),
                )?
            };
            if remaining == 0 {
                let encrypted_ref = crate::EncryptedMediaRef {
                    hash: record.hash.clone(),
                    path: PathBuf::from(&record.path),
                    mime: record.mime.clone(),
                    size: record.size,
                };
                self.vault
                    .delete(&encrypted_ref)
                    .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
                self.store.connection()?.execute(
                    "DELETE FROM media WHERE id=?1",
                    params![record.id.to_string()],
                )?;
            }
        }
        Ok(())
    }

    /// 按媒体标识读取完整媒体元数据。
    fn get(&self, id: &Uuid) -> Result<Option<MediaRecord>> {
        let connection = self.store.connection()?;
        load_media(&connection, id)
    }
}

/// 从 SQLite 主记录恢复媒体模型。
fn load_media(connection: &rusqlite::Connection, id: &Uuid) -> Result<Option<MediaRecord>> {
    connection
        .query_row(
            "SELECT kind, path, mime, width, height, duration_ms, ocr_text, transcript, hash, size FROM media WHERE id=?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                    row.get::<_, Option<u32>>(3)?, row.get::<_, Option<u32>>(4)?, row.get::<_, Option<u64>>(5)?,
                    row.get::<_, Option<String>>(6)?, row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?, row.get::<_, u64>(9)?,
                ))
            },
        )
        .optional()?
        .map(|(kind, path, mime, width, height, duration_ms, ocr_text, transcript, hash, size)| {
            Ok(MediaRecord {
                id: *id,
                kind: serde_json::from_str(&format!("\"{kind}\""))?,
                path, mime, width, height, duration_ms, ocr_text, transcript, hash, size,
            })
        })
        .transpose()
}
