-- 本文件创建统一记忆、语义块、向量、全文索引和标签的首版 SQLite 结构。

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL DEFAULT (unixepoch('subsec') * 1000)
);

CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT,
    content TEXT NOT NULL,
    content_format TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    captured_at INTEGER,
    device_id TEXT NOT NULL,
    meta TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS blocks (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    type TEXT NOT NULL,
    text TEXT NOT NULL,
    UNIQUE(memory_id, seq)
);

CREATE TABLE IF NOT EXISTS block_vectors (
    block_id TEXT PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    embedding TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS blocks_fts USING fts5(
    memory_id UNINDEXED,
    block_id UNINDEXED,
    text,
    tokenize = 'unicode61'
);

CREATE TABLE IF NOT EXISTS memory_tags (
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY(memory_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_blocks_memory ON blocks(memory_id, seq);
CREATE INDEX IF NOT EXISTS idx_vectors_memory ON block_vectors(memory_id);
CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at DESC);

INSERT OR IGNORE INTO schema_migrations(version) VALUES (1);

