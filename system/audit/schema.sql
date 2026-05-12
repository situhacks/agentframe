CREATE TABLE IF NOT EXISTS system_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    actor TEXT NOT NULL,
    mode TEXT,
    change_type TEXT NOT NULL,
    target_kind TEXT,
    target_path TEXT,
    campaign_slug TEXT,
    reason TEXT,
    summary TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'live'
);

CREATE INDEX IF NOT EXISTS idx_system_changes_created_at
    ON system_changes (created_at);

CREATE INDEX IF NOT EXISTS idx_system_changes_change_type
    ON system_changes (change_type);

CREATE INDEX IF NOT EXISTS idx_system_changes_target_path
    ON system_changes (target_path);
