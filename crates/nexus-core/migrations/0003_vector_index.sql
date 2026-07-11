-- 本文件创建 BGE-small 384 维 sqlite-vec 索引与嵌入空间版本记录。

CREATE VIRTUAL TABLE IF NOT EXISTS block_vectors_vec USING vec0(
    block_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE TABLE IF NOT EXISTS embedding_config (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    version INTEGER NOT NULL
);

INSERT OR IGNORE INTO embedding_config(singleton, model, dimensions, version)
VALUES (1, 'hash-384-m0', 384, 1);

INSERT OR IGNORE INTO schema_migrations(version) VALUES (3);

