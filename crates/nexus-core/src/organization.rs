//! 本文件实现记忆关联、可嵌套集合及集合成员关系的 SQLite 仓储能力。

use rusqlite::{OptionalExtension, params};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::{
    Collection, CollectionPatch, CoreError, Link, LinkCreator, LinkRelation, MemoryStore, Result,
    ingest::current_timestamp_millis,
    store::{enum_json, parse_uuid},
};

impl MemoryStore {
    /// 创建一条有向记忆关联，拒绝自关联和不存在的记忆。
    pub fn create_link(
        &self,
        from_id: Uuid,
        to_id: Uuid,
        relation: LinkRelation,
        created_by: LinkCreator,
    ) -> Result<Link> {
        if from_id == to_id {
            return Err(CoreError::InvalidInput("记忆不能关联到自身".into()));
        }
        self.require_memory(from_id)?;
        self.require_memory(to_id)?;
        let link = Link {
            from_id,
            to_id,
            relation,
            created_by,
            created_at: current_timestamp_millis()?,
        };
        self.connection()?.execute(
            "INSERT INTO links (from_id, to_id, relation, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from_id.to_string(), to_id.to_string(), enum_json(&relation)?, enum_json(&created_by)?, link.created_at],
        )?;
        Ok(link)
    }

    /// 返回一条记忆作为源或目标参与的全部关联。
    pub fn list_links(&self, memory_id: Uuid) -> Result<Vec<Link>> {
        self.require_memory(memory_id)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT from_id, to_id, relation, created_by, created_at FROM links WHERE from_id=?1 OR to_id=?1 ORDER BY created_at, from_id, to_id",
        )?;
        statement
            .query_map(params![memory_id.to_string()], parse_link_row)?
            .map(|row| row.map_err(Into::into))
            .collect()
    }

    /// 删除一条由源、目标和关系类型唯一确定的关联。
    pub fn delete_link(&self, from_id: Uuid, to_id: Uuid, relation: LinkRelation) -> Result<()> {
        let affected = self.connection()?.execute(
            "DELETE FROM links WHERE from_id=?1 AND to_id=?2 AND relation=?3",
            params![
                from_id.to_string(),
                to_id.to_string(),
                enum_json(&relation)?
            ],
        )?;
        if affected == 0 {
            return Err(CoreError::InvalidInput("指定记忆关联不存在".into()));
        }
        Ok(())
    }

    /// 创建集合；父集合存在时将新集合放入其下级。
    pub fn create_collection(
        &self,
        name: impl Into<String>,
        icon: Option<String>,
        parent_id: Option<Uuid>,
        sort: i64,
    ) -> Result<Collection> {
        let name = validated_collection_name(name.into())?;
        if let Some(parent_id) = parent_id {
            self.require_collection(parent_id)?;
        }
        let timestamp = current_timestamp_millis()?;
        let collection = Collection {
            id: Uuid::now_v7(),
            name,
            icon,
            parent_id,
            sort,
            created_at: timestamp,
            updated_at: timestamp,
        };
        self.connection()?.execute(
            "INSERT INTO collections (id, name, icon, parent_id, sort, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![collection.id.to_string(), collection.name, collection.icon, collection.parent_id.map(|id| id.to_string()), collection.sort, collection.created_at, collection.updated_at],
        )?;
        Ok(collection)
    }

    /// 按标识读取集合。
    pub fn get_collection(&self, id: Uuid) -> Result<Option<Collection>> {
        self.connection()?
            .query_row(
                "SELECT id, name, icon, parent_id, sort, created_at, updated_at FROM collections WHERE id=?1",
                params![id.to_string()],
                parse_collection_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// 按父级、排序值和名称返回全部集合。
    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, icon, parent_id, sort, created_at, updated_at FROM collections ORDER BY parent_id, sort, name, id",
        )?;
        statement
            .query_map([], parse_collection_row)?
            .map(|row| row.map_err(Into::into))
            .collect()
    }

    /// 更新集合字段，并拒绝把集合移动到自身或自身后代之下。
    pub fn update_collection(&self, id: Uuid, patch: CollectionPatch) -> Result<Collection> {
        let mut collection = self.get_collection(id)?.ok_or(CoreError::NotFound(id))?;
        if let Some(name) = patch.name {
            collection.name = validated_collection_name(name)?;
        }
        if let Some(icon) = patch.icon {
            collection.icon = icon;
        }
        if let Some(parent_id) = patch.parent_id {
            if let Some(parent_id) = parent_id {
                self.require_collection(parent_id)?;
                if parent_id == id || self.collection_is_descendant(parent_id, id)? {
                    return Err(CoreError::InvalidInput("集合层级不能形成循环".into()));
                }
            }
            collection.parent_id = parent_id;
        }
        if let Some(sort) = patch.sort {
            collection.sort = sort;
        }
        collection.updated_at = current_timestamp_millis()?;
        self.connection()?.execute(
            "UPDATE collections SET name=?2, icon=?3, parent_id=?4, sort=?5, updated_at=?6 WHERE id=?1",
            params![id.to_string(), collection.name, collection.icon, collection.parent_id.map(|value| value.to_string()), collection.sort, collection.updated_at],
        )?;
        Ok(collection)
    }

    /// 删除集合；子集合自动移动到根级，成员关系级联删除。
    pub fn delete_collection(&self, id: Uuid) -> Result<()> {
        let affected = self.connection()?.execute(
            "DELETE FROM collections WHERE id=?1",
            params![id.to_string()],
        )?;
        if affected == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// 将记忆加入集合；重复加入保持幂等。
    pub fn add_memory_to_collection(&self, collection_id: Uuid, memory_id: Uuid) -> Result<()> {
        self.require_collection(collection_id)?;
        self.require_memory(memory_id)?;
        self.connection()?.execute(
            "INSERT OR IGNORE INTO collection_items (collection_id, memory_id, added_at) VALUES (?1, ?2, ?3)",
            params![collection_id.to_string(), memory_id.to_string(), current_timestamp_millis()?],
        )?;
        Ok(())
    }

    /// 从集合移除记忆。
    pub fn remove_memory_from_collection(
        &self,
        collection_id: Uuid,
        memory_id: Uuid,
    ) -> Result<()> {
        let affected = self.connection()?.execute(
            "DELETE FROM collection_items WHERE collection_id=?1 AND memory_id=?2",
            params![collection_id.to_string(), memory_id.to_string()],
        )?;
        if affected == 0 {
            return Err(CoreError::InvalidInput("记忆不在指定集合中".into()));
        }
        Ok(())
    }

    /// 返回集合内按加入时间排序的记忆标识。
    pub fn list_collection_memory_ids(&self, collection_id: Uuid) -> Result<Vec<Uuid>> {
        self.require_collection(collection_id)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT memory_id FROM collection_items WHERE collection_id=?1 ORDER BY added_at, memory_id",
        )?;
        statement
            .query_map(params![collection_id.to_string()], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| parse_uuid(&row?))
            .collect()
    }

    /// 确认记忆存在，以便把外键错误转换为稳定领域错误。
    fn require_memory(&self, id: Uuid) -> Result<()> {
        if self.get(&id)?.is_none() {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// 确认集合存在。
    fn require_collection(&self, id: Uuid) -> Result<()> {
        if self.get_collection(id)?.is_none() {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }

    /// 判断候选父级是否位于当前集合的后代链中。
    fn collection_is_descendant(&self, candidate: Uuid, ancestor: Uuid) -> Result<bool> {
        self.connection()?
            .query_row(
                "WITH RECURSIVE descendants(id) AS (SELECT id FROM collections WHERE parent_id=?1 UNION ALL SELECT c.id FROM collections c JOIN descendants d ON c.parent_id=d.id) SELECT EXISTS(SELECT 1 FROM descendants WHERE id=?2)",
                params![ancestor.to_string(), candidate.to_string()],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}

/// 校验并清理集合名称。
fn validated_collection_name(name: String) -> Result<String> {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(CoreError::InvalidInput("集合名称不能为空".into()));
    }
    Ok(name)
}

/// 从 SQLite 行恢复关联模型。
fn parse_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Link> {
    let from_id = row.get::<_, String>(0)?;
    let to_id = row.get::<_, String>(1)?;
    let relation = row.get::<_, String>(2)?;
    let created_by = row.get::<_, String>(3)?;
    Ok(Link {
        from_id: Uuid::parse_str(&from_id).map_err(sql_conversion_error)?,
        to_id: Uuid::parse_str(&to_id).map_err(sql_conversion_error)?,
        relation: parse_stored_enum(&relation).map_err(sql_conversion_error)?,
        created_by: parse_stored_enum(&created_by).map_err(sql_conversion_error)?,
        created_at: row.get(4)?,
    })
}

/// 从 SQLite 行恢复集合模型。
fn parse_collection_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Collection> {
    let id = row.get::<_, String>(0)?;
    let parent_id = row.get::<_, Option<String>>(3)?;
    Ok(Collection {
        id: Uuid::parse_str(&id).map_err(sql_conversion_error)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        parent_id: parent_id
            .map(|value| Uuid::parse_str(&value).map_err(sql_conversion_error))
            .transpose()?,
        sort: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// 从数据库 snake_case 字符串恢复枚举。
fn parse_stored_enum<T: DeserializeOwned>(value: &str) -> serde_json::Result<T> {
    serde_json::from_str(&format!("\"{value}\""))
}

/// 把模型解析错误转换为 rusqlite 列转换错误。
fn sql_conversion_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
