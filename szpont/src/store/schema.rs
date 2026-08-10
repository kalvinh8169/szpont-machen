pub const SCHEMA_VERSION: i64 = 4;

pub const MIGRATE_V2_TO_V3: &str = "
ALTER TABLE sessions_meta ADD COLUMN context_tokens INTEGER;
ALTER TABLE sessions_meta ADD COLUMN context_window INTEGER;
";

pub const MIGRATE_V3_TO_V4: &str = "
ALTER TABLE sessions_meta ADD COLUMN last_seen_at INTEGER;
UPDATE sessions_meta SET last_seen_at = CAST(strftime('%s', 'now') AS INTEGER) * 1000;
";

pub const DROP_ALL: &str = "
DROP TABLE IF EXISTS meta;
DROP TABLE IF EXISTS sessions_meta;
DROP TABLE IF EXISTS usage_cache;
DROP TABLE IF EXISTS usage_buckets;
DROP TABLE IF EXISTS limits_snapshots;
DROP TABLE IF EXISTS mcp_events;
";

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT
);

CREATE TABLE IF NOT EXISTS sessions_meta (
  tool TEXT NOT NULL,
  session_id TEXT NOT NULL,
  cwd TEXT,
  title TEXT,
  title_source TEXT,
  preview TEXT,
  model TEXT,
  context_tokens INTEGER,
  context_window INTEGER,
  first_seen_at INTEGER NOT NULL,
  last_seen_at INTEGER,
  completed_at INTEGER,
  completed_by TEXT,
  source TEXT NOT NULL DEFAULT 'scan',
  PRIMARY KEY (tool, session_id)
);

CREATE TABLE IF NOT EXISTS usage_cache (
  tool TEXT NOT NULL,
  session_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  byte_offset INTEGER NOT NULL,
  mtime_ms INTEGER NOT NULL,
  size INTEGER NOT NULL,
  input_uncached INTEGER NOT NULL DEFAULT 0,
  input_cache_read INTEGER NOT NULL DEFAULT 0,
  input_cache_write INTEGER NOT NULL DEFAULT 0,
  output INTEGER NOT NULL DEFAULT 0,
  reasoning INTEGER NOT NULL DEFAULT 0,
  dedup_state TEXT,
  PRIMARY KEY (tool, session_id, file_path)
);

CREATE INDEX IF NOT EXISTS usage_cache_session
  ON usage_cache (tool, session_id);

CREATE TABLE IF NOT EXISTS usage_buckets (
  tool TEXT NOT NULL,
  hour_ts INTEGER NOT NULL,
  input_uncached INTEGER NOT NULL DEFAULT 0,
  input_cache_read INTEGER NOT NULL DEFAULT 0,
  input_cache_write INTEGER NOT NULL DEFAULT 0,
  output INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (tool, hour_ts)
);

CREATE TABLE IF NOT EXISTS limits_snapshots (
  tool TEXT PRIMARY KEY,
  captured_at INTEGER NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mcp_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at INTEGER NOT NULL,
  tool TEXT NOT NULL,
  session_id TEXT,
  kind TEXT NOT NULL,
  payload_json TEXT
);
";
