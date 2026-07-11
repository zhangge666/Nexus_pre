-- 本文件创建加密媒体元数据与 Memory 多对多关联表。

CREATE TABLE IF NOT EXISTS media (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    mime TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    duration_ms INTEGER,
    ocr_text TEXT,
    transcript TEXT,
    hash TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_media (
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    PRIMARY KEY(memory_id, media_id)
);

CREATE INDEX IF NOT EXISTS idx_memory_media_media ON memory_media(media_id);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (2);

