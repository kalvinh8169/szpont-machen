use std::cell::OnceCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{OpenSessionIndex, ToolAdapter};
use crate::core::{
    Enrichment, LaunchSpec, Liveness, SessionKey, SessionSummary, ToolId, Usage, now_ms,
    system_time_ms,
};
use crate::store::checkpoints::{CheckpointTxn, FileCheckpoint};

pub struct KimiAdapter {
    root: Option<PathBuf>,
    context_windows: OnceCell<HashMap<String, u64>>,
    open_index: OpenSessionIndex,
}

impl KimiAdapter {
    pub fn new() -> Self {
        Self {
            root: ToolId::Kimi.home(),
            context_windows: OnceCell::new(),
            open_index: OpenSessionIndex::new("kimi"),
        }
    }

    #[cfg(test)]
    fn with_root(root: PathBuf) -> Self {
        Self {
            root: Some(root),
            context_windows: OnceCell::new(),
            open_index: OpenSessionIndex::new("kimi"),
        }
    }

    fn context_window_for(&self, model: &str) -> Option<u64> {
        self.context_windows
            .get_or_init(|| {
                self.root
                    .as_deref()
                    .map(read_context_windows)
                    .unwrap_or_default()
            })
            .get(model)
            .copied()
    }
}

impl ToolAdapter for KimiAdapter {
    fn id(&self) -> ToolId {
        ToolId::Kimi
    }

    fn is_installed(&self) -> bool {
        self.root
            .as_ref()
            .is_some_and(|r| r.join("sessions").exists())
    }

    fn store_roots(&self) -> Vec<PathBuf> {
        self.root.iter().cloned().collect()
    }

    fn discover(&self) -> anyhow::Result<Vec<SessionSummary>> {
        let Some(root) = &self.root else {
            return Ok(Vec::new());
        };
        let sessions_root = root.join("sessions");
        let mut sessions = Vec::new();
        for workspace in std::fs::read_dir(&sessions_root)? {
            let Ok(workspace) = workspace else { continue };
            if !workspace.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            for entry in std::fs::read_dir(workspace.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                let session_dir = entry.path();
                let Some(name) = session_dir.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.starts_with("session_") {
                    continue;
                }
                if let Some(session) = read_session(&session_dir) {
                    sessions.push(session);
                }
            }
        }
        self.open_index.reindex(&sessions);
        Ok(sessions)
    }

    fn refresh_usage(
        &self,
        session: &SessionSummary,
        ckpt: &mut CheckpointTxn,
    ) -> anyhow::Result<Enrichment> {
        let mut enrich = Enrichment::default();
        let mut total = Usage::default();
        let mut buckets: HashMap<i64, Usage> = HashMap::new();
        let bucket_cutoff_ms = now_ms() - crate::core::BUCKET_RETENTION_SECS * 1000;
        for file in &session.usage_files {
            let Ok(meta) = std::fs::metadata(file) else {
                continue;
            };
            let mtime_ms = system_time_ms(meta.modified().ok());
            let size = meta.len();
            let prior = ckpt.get(ToolId::Kimi, &session.key.id, file)?;
            if let Some(p) = &prior
                && p.mtime_ms == mtime_ms
                && p.size == size
            {
                total.add(&p.usage);
                continue;
            }
            let (start_offset, mut usage) = match prior {
                Some(p) if p.byte_offset <= size => (p.byte_offset, p.usage),
                _ => (0, Usage::default()),
            };
            let track_context = file
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "main");
            let consumed = parse_wire_file(
                file,
                start_offset,
                &mut usage,
                &mut enrich,
                &mut buckets,
                bucket_cutoff_ms,
                track_context,
            )?;
            total.add(&usage);
            ckpt.put(
                ToolId::Kimi,
                &session.key.id,
                file,
                &FileCheckpoint {
                    byte_offset: consumed,
                    mtime_ms,
                    size,
                    usage,
                    dedup_state: None,
                },
            )?;
        }
        for (hour_ts, usage) in &buckets {
            ckpt.add_bucket(ToolId::Kimi, *hour_ts, usage)?;
        }
        if enrich.context_window.is_none() {
            let model = enrich.model.as_deref().or(session.model.as_deref());
            enrich.context_window = model.and_then(|m| self.context_window_for(m));
        }
        enrich.usage = Some(total);
        Ok(enrich)
    }

    fn probe_context(&self, session: &SessionSummary) -> Option<(u64, Option<u64>)> {
        let wire = session
            .usage_files
            .iter()
            .find(|f| {
                f.parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "main")
            })
            .or_else(|| session.usage_files.first())?;
        let (tokens, model) = tail_last_usage_record(wire)?;
        let window = model
            .as_deref()
            .or(session.model.as_deref())
            .and_then(|m| self.context_window_for(m));
        Some((tokens, window))
    }

    fn delete_session(&self, session: &SessionSummary) -> anyhow::Result<()> {
        let Some(root) = &self.root else {
            anyhow::bail!("~/.kimi-code not found");
        };
        let sessions_root = root.join("sessions").canonicalize()?;
        let candidate = session
            .usage_files
            .first()
            .and_then(|f| f.parent())
            .and_then(|agent_dir| agent_dir.parent())
            .and_then(|agents_dir| agents_dir.parent())
            .map(Path::to_path_buf);
        let Some(session_dir) = candidate else {
            anyhow::bail!("session directory is unknown");
        };
        let resolved = session_dir.canonicalize()?;
        if !resolved.starts_with(&sessions_root) {
            anyhow::bail!(
                "refusing to delete {}: outside {}",
                resolved.display(),
                sessions_root.display()
            );
        }
        if !resolved
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("session_") && !n.contains('/') && !n.contains(".."))
        {
            anyhow::bail!(
                "refusing to delete {}: not a session directory",
                resolved.display()
            );
        }
        std::fs::remove_dir_all(&resolved)?;
        Ok(())
    }

    fn liveness(&self, session: &SessionSummary) -> Liveness {
        let state_recent = session
            .usage_files
            .first()
            .and_then(|f| f.parent())
            .and_then(|agents_dir| agents_dir.parent())
            .map(|session_dir| session_dir.join("state.json"))
            .or_else(|| {
                session
                    .usage_files
                    .first()
                    .map(|f| f.with_file_name("state.json"))
            })
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < super::RUNNING_WINDOW);
        if state_recent {
            return Liveness::Running;
        }
        self.open_index.liveness(session)
    }

    fn resume_command(&self, session: &SessionSummary) -> LaunchSpec {
        LaunchSpec {
            program: "kimi".to_string(),
            args: vec!["-S".to_string(), session.key.id.clone()],
            cwd: session
                .cwd
                .clone()
                .unwrap_or_else(crate::core::fallback_cwd),
        }
    }

    fn new_session_command(&self, cwd: &Path) -> LaunchSpec {
        LaunchSpec {
            program: "kimi".to_string(),
            args: Vec::new(),
            cwd: cwd.to_path_buf(),
        }
    }
}

#[derive(Deserialize)]
struct KimiState {
    id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<i64>,
    #[serde(default)]
    archived: bool,
    #[serde(rename = "lastPrompt")]
    last_prompt: Option<String>,
    title: Option<String>,
}

fn valid_session_id(id: &str) -> bool {
    crate::core::valid_session_id(id, Some("session_"))
}

fn read_session(session_dir: &Path) -> Option<SessionSummary> {
    let state_path = session_dir.join("state.json");
    let text = crate::core::read_to_string_capped(&state_path, crate::core::MAX_CONFIG_BYTES)?;
    let state: KimiState = serde_json::from_str(&text).ok()?;
    let id = state.id.filter(|id| valid_session_id(id)).or_else(|| {
        session_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|id| valid_session_id(id))
    })?;
    let mut usage_files = Vec::new();
    let agents_dir = session_dir.join("agents");
    for agent in std::fs::read_dir(&agents_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        let wire = agent.path().join("wire.jsonl");
        if wire.exists() {
            usage_files.push(wire);
        }
    }
    let updated_at_ms = state.updated_at.unwrap_or_else(|| {
        std::fs::metadata(&state_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map_or(0, |t| system_time_ms(Some(t)))
    });
    Some(SessionSummary {
        key: SessionKey {
            tool: ToolId::Kimi,
            id,
        },
        cwd: state.cwd.map(PathBuf::from),
        title: state
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .map(|t| crate::core::clean_text_fact(&t, crate::core::MAX_TEXT_FACT_CHARS)),
        preview: state
            .last_prompt
            .map(|p| crate::core::clean_text_fact(&p, crate::core::MAX_PREVIEW_CHARS)),
        model: None,
        origin_url: None,
        created_at_ms: state.created_at,
        updated_at_ms,
        native_archived: state.archived,
        native_tokens_used: None,
        usage_files,
    })
}

#[derive(Deserialize)]
struct KimiWireLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    model: Option<String>,
    usage: Option<KimiWireUsage>,
    #[serde(rename = "usageScope")]
    usage_scope: Option<String>,
    time: Option<i64>,
}

#[derive(Deserialize)]
struct KimiWireUsage {
    #[serde(rename = "inputOther", default)]
    input_other: u64,
    #[serde(default)]
    output: u64,
    #[serde(rename = "inputCacheRead", default)]
    input_cache_read: u64,
    #[serde(rename = "inputCacheCreation", default)]
    input_cache_creation: u64,
}

fn parse_wire_file(
    path: &Path,
    start_offset: u64,
    usage: &mut Usage,
    enrich: &mut Enrichment,
    buckets: &mut HashMap<i64, Usage>,
    bucket_cutoff_ms: i64,
    track_context: bool,
) -> anyhow::Result<u64> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::with_capacity(128 * 1024, file);
    let mut consumed = start_offset;
    let bucket_ceiling_ms = now_ms() + crate::core::BUCKET_FUTURE_SLACK_MS;
    loop {
        let capped = crate::core::read_line_capped(&mut reader, crate::core::MAX_LINE_BYTES)?;
        if !capped.complete {
            break;
        }
        consumed += capped.consumed;
        if capped.truncated {
            continue;
        }
        let line = String::from_utf8_lossy(&capped.bytes);
        if !line.contains("\"usage.record\"") {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<KimiWireLine>(&line) else {
            continue;
        };
        if parsed.kind.as_deref() != Some("usage.record") {
            continue;
        }
        if parsed.usage_scope.as_deref() != Some("turn") {
            continue;
        }
        let Some(wire_usage) = parsed.usage else {
            continue;
        };
        if let Some(model) = parsed.model {
            enrich.model = Some(crate::core::clean_text_fact(
                &model,
                crate::core::MAX_MODEL_CHARS,
            ));
        }
        if track_context {
            enrich.context_tokens = Some(
                wire_usage
                    .input_other
                    .saturating_add(wire_usage.input_cache_read)
                    .saturating_add(wire_usage.input_cache_creation),
            );
        }
        let delta = Usage {
            input_uncached: wire_usage.input_other,
            input_cache_read: wire_usage.input_cache_read,
            input_cache_write: wire_usage.input_cache_creation,
            output: wire_usage.output,
            reasoning: 0,
        };
        usage.add(&delta);
        if let Some(ts_ms) = parsed.time
            && ts_ms >= bucket_cutoff_ms
            && ts_ms <= bucket_ceiling_ms
        {
            let hour_ts = ts_ms / 1000 / 3600 * 3600;
            buckets.entry(hour_ts).or_default().add(&delta);
        }
    }
    Ok(consumed)
}

fn tail_last_usage_record(path: &Path) -> Option<(u64, Option<String>)> {
    crate::core::tail_find_last(path, "\"usage.record\"", |parsed: KimiWireLine| {
        if parsed.kind.as_deref() != Some("usage.record") {
            return None;
        }
        let usage = parsed.usage?;
        Some((
            usage
                .input_other
                .saturating_add(usage.input_cache_read)
                .saturating_add(usage.input_cache_creation),
            parsed.model,
        ))
    })
}

fn read_context_windows(root: &Path) -> HashMap<String, u64> {
    let Some(text) = crate::core::read_to_string_capped(
        &root.join("config.toml"),
        crate::core::MAX_CONFIG_BYTES,
    ) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    let mut current_model: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(section) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current_model = section
                .strip_prefix("models.")
                .map(|name| name.trim_matches('"').to_string());
            continue;
        }
        if let (Some(model), Some(value)) = (
            current_model.as_ref(),
            trimmed
                .strip_prefix("max_context_size")
                .and_then(|rest| rest.trim_start().strip_prefix('='))
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|v| *v > 0),
        ) {
            map.insert(model.clone(), value);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(".tmp/fixtures").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn session_with_wire(wire: PathBuf, id: &str) -> SessionSummary {
        SessionSummary {
            key: SessionKey {
                tool: ToolId::Kimi,
                id: id.to_string(),
            },
            cwd: None,
            title: None,
            preview: None,
            model: None,
            origin_url: None,
            created_at_ms: None,
            updated_at_ms: 0,
            native_archived: false,
            native_tokens_used: None,
            usage_files: vec![wire],
        }
    }

    #[test]
    fn delete_session_removes_the_session_directory() {
        let root = fresh_dir("kimi-delete-ok");
        let session_dir = root.join("sessions").join("ws").join("session_x");
        let agent_dir = session_dir.join("agents").join("main");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let wire = agent_dir.join("wire.jsonl");
        std::fs::write(&wire, "{}\n").unwrap();
        let adapter = KimiAdapter::with_root(root);
        let session = session_with_wire(wire, "session_x");
        adapter.delete_session(&session).unwrap();
        assert!(!session_dir.exists());
    }

    #[test]
    fn delete_session_refuses_a_non_session_directory() {
        let root = fresh_dir("kimi-delete-name");
        let session_dir = root.join("sessions").join("ws").join("notasession");
        let agent_dir = session_dir.join("agents").join("main");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let wire = agent_dir.join("wire.jsonl");
        std::fs::write(&wire, "{}\n").unwrap();
        let adapter = KimiAdapter::with_root(root);
        let session = session_with_wire(wire, "session_x");
        let err = adapter.delete_session(&session).unwrap_err();
        assert!(err.to_string().contains("not a session directory"));
        assert!(session_dir.exists());
    }

    #[test]
    fn delete_session_refuses_paths_outside_the_sessions_root() {
        let root = fresh_dir("kimi-delete-outside");
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        let outside = fresh_dir("kimi-delete-outside-target");
        let session_dir = outside.join("session_evil");
        let agent_dir = session_dir.join("agents").join("main");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let wire = agent_dir.join("wire.jsonl");
        std::fs::write(&wire, "{}\n").unwrap();
        let adapter = KimiAdapter::with_root(root);
        let session = session_with_wire(wire, "session_evil");
        let err = adapter.delete_session(&session).unwrap_err();
        assert!(err.to_string().contains("outside"));
        assert!(session_dir.exists());
    }

    #[test]
    fn wire_usage_records_are_summed() {
        let dir = PathBuf::from(".tmp/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wire.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"metadata\",\"protocol_version\":\"1.5\"}\n\
             {\"type\":\"usage.record\",\"model\":\"kimi-code/k3\",\"usage\":{\"inputOther\":791,\"output\":238,\"inputCacheRead\":39168,\"inputCacheCreation\":0},\"usageScope\":\"turn\",\"time\":1786377982736}\n\
             {\"type\":\"usage.record\",\"model\":\"kimi-code/k3\",\"usage\":{\"inputOther\":9,\"output\":2,\"inputCacheRead\":100,\"inputCacheCreation\":50},\"usageScope\":\"turn\",\"time\":1786377999999}\n",
        )
        .unwrap();
        let mut usage = Usage::default();
        let mut enrich = Enrichment::default();
        let mut buckets = HashMap::new();
        parse_wire_file(&path, 0, &mut usage, &mut enrich, &mut buckets, 0, true).unwrap();
        assert_eq!(usage.input_uncached, 800);
        assert_eq!(usage.output, 240);
        assert_eq!(usage.input_cache_read, 39268);
        assert_eq!(usage.input_cache_write, 50);
        assert_eq!(enrich.model.as_deref(), Some("kimi-code/k3"));
        assert_eq!(enrich.context_tokens, Some(9 + 100 + 50));
    }
}
