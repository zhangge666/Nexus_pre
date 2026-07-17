-- 本文件创建 M4 知识卡片复习状态、评分历史和到期事件去重字段。

CREATE TABLE IF NOT EXISTS review_states (
    memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    card_front TEXT NOT NULL,
    card_back TEXT NOT NULL,
    stability REAL NOT NULL DEFAULT 0,
    difficulty REAL NOT NULL DEFAULT 5,
    due_at INTEGER NOT NULL,
    last_reviewed_at INTEGER,
    reps INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',
    deck TEXT,
    created_at INTEGER NOT NULL,
    last_due_notified_at INTEGER
);

CREATE TABLE IF NOT EXISTS review_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    rating TEXT NOT NULL,
    reviewed_at INTEGER NOT NULL,
    stability REAL NOT NULL,
    difficulty REAL NOT NULL,
    due_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_review_states_due ON review_states(due_at, state);
CREATE INDEX IF NOT EXISTS idx_review_states_deck ON review_states(deck, due_at);
CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed ON review_logs(reviewed_at DESC);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (5);
