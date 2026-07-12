-- 本文件创建记忆关联、可嵌套集合及集合成员关系的 M1 数据结构。

CREATE TABLE IF NOT EXISTS links (
    from_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    to_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    relation TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(from_id, to_id, relation),
    CHECK(from_id <> to_id)
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    parent_id TEXT REFERENCES collections(id) ON DELETE SET NULL,
    sort INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK(parent_id IS NULL OR parent_id <> id)
);

CREATE TABLE IF NOT EXISTS collection_items (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY(collection_id, memory_id)
);

CREATE INDEX IF NOT EXISTS idx_links_to ON links(to_id, relation);
CREATE INDEX IF NOT EXISTS idx_collections_parent ON collections(parent_id, sort, name);
CREATE INDEX IF NOT EXISTS idx_collection_items_memory ON collection_items(memory_id);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (4);
