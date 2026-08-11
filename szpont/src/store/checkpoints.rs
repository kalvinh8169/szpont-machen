use std::path::Path;

use rusqlite::{OptionalExtension, params};

use crate::core::{ToolId, Usage};

const MAX_BUCKET_TOKENS: u64 = 1_000_000_000_000;

pub struct FileCheckpoint {
    pub byte_offset: u64,
    pub mtime_ms: i64,
    pub size: u64,
    pub usage: Usage,
    pub dedup_state: Option<String>,
}

pub struct CheckpointTxn<'a> {
    conn: &'a mut rusqlite::Connection,
    active: bool,
}

impl<'a> CheckpointTxn<'a> {
    pub fn begin(conn: &'a mut rusqlite::Connection) -> anyhow::Result<CheckpointTxn<'a>> {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(CheckpointTxn { conn, active: true })
    }

    pub fn checkpoint(&mut self) -> anyhow::Result<()> {
        self.conn.execute_batch("COMMIT")?;
        self.active = false;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        self.active = true;
        Ok(())
    }

    pub fn commit(mut self) -> anyhow::Result<()> {
        self.conn.execute_batch("COMMIT")?;
        self.active = false;
        Ok(())
    }

    pub fn upsert_facts(
        &self,
        tool: ToolId,
        session_id: &str,
        cwd: Option<&str>,
        title: Option<(&str, crate::core::TitleKind)>,
        preview: Option<&str>,
        model: Option<&str>,
        context_tokens: Option<i64>,
        context_window: Option<i64>,
    ) -> anyhow::Result<()> {
        super::upsert_session_facts(
            self.conn,
            tool,
            session_id,
            cwd,
            title,
            preview,
            model,
            context_tokens,
            context_window,
            "scan",
        )
    }

    pub fn touch_seen(&self, tool: ToolId, session_id: &str, now: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE sessions_meta SET last_seen_at = ?3 WHERE tool = ?1 AND session_id = ?2",
            params![tool.as_str(), session_id, now],
        )?;
        Ok(())
    }

    pub fn get(
        &self,
        tool: ToolId,
        session_id: &str,
        file_path: &Path,
    ) -> anyhow::Result<Option<FileCheckpoint>> {
        let row = self
            .conn
            .query_row(
                "SELECT byte_offset, mtime_ms, size, input_uncached, input_cache_read,
                        input_cache_write, output, reasoning, dedup_state
                 FROM usage_cache
                 WHERE tool = ?1 AND session_id = ?2 AND file_path = ?3",
                params![tool.as_str(), session_id, file_path.to_string_lossy()],
                |row| {
                    Ok(FileCheckpoint {
                        byte_offset: row.get::<_, i64>(0)? as u64,
                        mtime_ms: row.get(1)?,
                        size: row.get::<_, i64>(2)? as u64,
                        usage: Usage {
                            input_uncached: row.get::<_, i64>(3)? as u64,
                            input_cache_read: row.get::<_, i64>(4)? as u64,
                            input_cache_write: row.get::<_, i64>(5)? as u64,
                            output: row.get::<_, i64>(6)? as u64,
                            reasoning: row.get::<_, i64>(7)? as u64,
                        },
                        dedup_state: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    pub fn put(
        &self,
        tool: ToolId,
        session_id: &str,
        file_path: &Path,
        ckpt: &FileCheckpoint,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO usage_cache
               (tool, session_id, file_path, byte_offset, mtime_ms, size,
                input_uncached, input_cache_read, input_cache_write, output, reasoning, dedup_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (tool, session_id, file_path)
             DO UPDATE SET
               byte_offset = excluded.byte_offset,
               mtime_ms = excluded.mtime_ms,
               size = excluded.size,
               input_uncached = excluded.input_uncached,
               input_cache_read = excluded.input_cache_read,
               input_cache_write = excluded.input_cache_write,
               output = excluded.output,
               reasoning = excluded.reasoning,
               dedup_state = excluded.dedup_state",
            params![
                tool.as_str(),
                session_id,
                file_path.to_string_lossy(),
                ckpt.byte_offset as i64,
                ckpt.mtime_ms,
                ckpt.size as i64,
                ckpt.usage.input_uncached as i64,
                ckpt.usage.input_cache_read as i64,
                ckpt.usage.input_cache_write as i64,
                ckpt.usage.output as i64,
                ckpt.usage.reasoning as i64,
                ckpt.dedup_state,
            ],
        )?;
        Ok(())
    }

    pub fn add_bucket(&self, tool: ToolId, hour_ts: i64, usage: &Usage) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO usage_buckets
               (tool, hour_ts, input_uncached, input_cache_read, input_cache_write, output)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (tool, hour_ts)
             DO UPDATE SET
               input_uncached = usage_buckets.input_uncached + excluded.input_uncached,
               input_cache_read = usage_buckets.input_cache_read + excluded.input_cache_read,
               input_cache_write = usage_buckets.input_cache_write + excluded.input_cache_write,
               output = usage_buckets.output + excluded.output",
            params![
                tool.as_str(),
                hour_ts,
                usage.input_uncached.min(MAX_BUCKET_TOKENS) as i64,
                usage.input_cache_read.min(MAX_BUCKET_TOKENS) as i64,
                usage.input_cache_write.min(MAX_BUCKET_TOKENS) as i64,
                usage.output.min(MAX_BUCKET_TOKENS) as i64,
            ],
        )?;
        Ok(())
    }

    pub fn prune_buckets(&self, before_hour_ts: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM usage_buckets WHERE hour_ts < ?1",
            params![before_hour_ts],
        )?;
        Ok(())
    }
}

impl Drop for CheckpointTxn<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::store::Store;

    #[test]
    fn concurrent_read_then_write_txns_serialize() {
        let dir = PathBuf::from(".tmp/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("ckpt-concurrent.db");
        let _ = std::fs::remove_file(&db);
        let _ = Store::open(Some(&db)).unwrap();
        let writers: Vec<_> = (0..2)
            .map(|writer| {
                let db = db.clone();
                std::thread::spawn(move || {
                    let mut store = Store::open(Some(&db)).unwrap();
                    let file = PathBuf::from(format!("file-{writer}"));
                    for round in 0..25u64 {
                        let txn = CheckpointTxn::begin(store.conn_mut()).unwrap();
                        let _ = txn.get(ToolId::Claude, "shared", &file).unwrap();
                        txn.put(
                            ToolId::Claude,
                            "shared",
                            &file,
                            &FileCheckpoint {
                                byte_offset: round,
                                mtime_ms: 1,
                                size: round,
                                usage: Usage::default(),
                                dedup_state: None,
                            },
                        )
                        .unwrap();
                        txn.commit().unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
    }
}
