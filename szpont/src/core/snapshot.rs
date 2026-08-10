use std::path::{Path, PathBuf};

use crate::adapters;
use crate::core::{BUCKET_RETENTION_SECS, Liveness, SessionKey, SessionSummary, Usage, now_ms};
use crate::store::checkpoints::CheckpointTxn;
use crate::store::{Store, StoredMeta};

pub struct SessionRow {
    pub session: SessionSummary,
    pub liveness: Liveness,
    pub completed: bool,
    pub completed_at: Option<i64>,
    pub usage: Option<Usage>,
    pub context_tokens: Option<u64>,
    pub context_window: Option<u64>,
}

impl SessionRow {
    pub fn tokens_total(&self) -> Option<u64> {
        self.usage
            .map(|u| u.total())
            .or(self.session.native_tokens_used)
    }
}

const PRUNE_GRACE_MS: i64 = 15 * 60 * 1000;

pub fn collect_sessions(
    store: &mut Store,
    progress: &mut dyn FnMut(&str),
) -> anyhow::Result<Vec<SessionRow>> {
    collect_sessions_with(&adapters::installed(), store, progress)
}

pub fn collect_sessions_with(
    adapters: &[Box<dyn adapters::ToolAdapter>],
    store: &mut Store,
    progress: &mut dyn FnMut(&str),
) -> anyhow::Result<Vec<SessionRow>> {
    let meta = store.session_meta_map()?;
    let window_overrides = store.context_window_overrides()?;
    let mut ckpt = CheckpointTxn::begin(store.conn_mut())?;
    let mut rows = Vec::new();
    let mut discovered_counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for adapter in adapters {
        progress(&format!("scan: {} — discovering…", adapter.id().as_str()));
        let discovered = match adapter.discover() {
            Ok(list) => list,
            Err(err) => {
                crate::logging::warn(&format!(
                    "{} discovery failed: {err:#}",
                    adapter.id().as_str()
                ));
                continue;
            }
        };
        discovered_counts.insert(adapter.id().as_str(), discovered.len());
        let total = discovered.len();
        progress(&format!(
            "scan: {} — {total} sessions, reading usage…",
            adapter.id().as_str()
        ));
        for (index, mut session) in discovered.into_iter().enumerate() {
            if index > 0 && index % 25 == 0 {
                progress(&format!(
                    "scan: {} — usage {index}/{total}…",
                    adapter.id().as_str()
                ));
            }
            let stored = meta.get(&session.key);
            if let Some(m) = stored {
                if session.cwd.is_none() {
                    session.cwd = m.cwd.clone().map(PathBuf::from);
                }
                if session.title.is_none() {
                    session.title.clone_from(&m.title);
                }
                if session.preview.is_none() {
                    session.preview.clone_from(&m.preview);
                }
                if session.model.is_none() {
                    session.model.clone_from(&m.model);
                }
            }
            let mut context_tokens = stored
                .and_then(|m| m.context_tokens)
                .map(|v| v.max(0) as u64);
            let mut context_window = stored
                .and_then(|m| m.context_window)
                .map(|v| v.max(0) as u64);
            let usage = match adapter.refresh_usage(&session, &mut ckpt) {
                Ok(enrich) => {
                    let has_facts = enrich.cwd.is_some()
                        || enrich.title.is_some()
                        || enrich.preview.is_some()
                        || enrich.model.is_some()
                        || enrich.context_tokens.is_some()
                        || enrich.context_window.is_some();
                    if let Some(cwd) = enrich.cwd {
                        session.cwd = Some(cwd);
                    }
                    if let Some((title, _)) = &enrich.title {
                        session.title = Some(title.clone());
                    }
                    if let Some(preview) = &enrich.preview {
                        session.preview = Some(preview.clone());
                    }
                    if let Some(model) = &enrich.model {
                        session.model = Some(model.clone());
                    }
                    if let Some(custom) = stored.and_then(StoredMeta::custom_title) {
                        session.title = Some(custom);
                    }
                    if let Some(tokens) = enrich.context_tokens {
                        context_tokens = Some(tokens);
                    }
                    if let Some(window) = enrich.context_window {
                        context_window = Some(window);
                    }
                    if has_facts {
                        ckpt.upsert_facts(
                            session.key.tool,
                            &session.key.id,
                            session.cwd.as_deref().and_then(Path::to_str),
                            enrich.title.as_ref().map(|(t, k)| (t.as_str(), *k)),
                            enrich.preview.as_deref(),
                            enrich.model.as_deref(),
                            enrich.context_tokens.map(|v| v as i64),
                            enrich.context_window.map(|v| v as i64),
                        )?;
                    }
                    enrich.usage
                }
                Err(err) => {
                    crate::logging::warn(&format!(
                        "{} usage refresh failed for {}: {err:#}",
                        adapter.id().as_str(),
                        session.key.id
                    ));
                    None
                }
            };
            if context_tokens.is_none() {
                let probed = adapter.probe_context(&session);
                let (tokens, window) = probed.unwrap_or((0, None));
                context_tokens = Some(tokens);
                if context_window.is_none() {
                    context_window = window;
                }
                ckpt.upsert_facts(
                    session.key.tool,
                    &session.key.id,
                    None,
                    None,
                    None,
                    None,
                    Some(tokens as i64),
                    context_window.map(|w| w as i64),
                )?;
            }
            if context_window.is_none()
                && let Some(model) = &session.model
            {
                context_window = window_overrides.get(model).copied();
            }
            ckpt.touch_seen(session.key.tool, &session.key.id, now_ms())?;
            let liveness = adapter.liveness(&session);
            let completed_at = stored.and_then(|m| m.completed_at);
            let completed = session.native_archived || completed_at.is_some();
            rows.push(SessionRow {
                session,
                liveness,
                completed,
                completed_at,
                usage,
                context_tokens,
                context_window,
            });
        }
    }
    ckpt.prune_buckets(now_ms() / 1000 - BUCKET_RETENTION_SECS)?;
    ckpt.commit()?;
    let discovered_keys: std::collections::HashSet<SessionKey> =
        rows.iter().map(|r| r.session.key.clone()).collect();
    let (kept, gone): (Vec<SessionRow>, Vec<SessionRow>) = rows
        .into_iter()
        .partition(|r| r.session.cwd.as_deref().is_none_or(Path::exists));
    let mut rows = kept;
    let kept_keys: std::collections::HashSet<&SessionKey> =
        rows.iter().map(|r| &r.session.key).collect();
    for row in &gone {
        if kept_keys.contains(&row.session.key) {
            continue;
        }
        if !cwd_definitively_missing(row.session.cwd.as_deref()) {
            continue;
        }
        let _ = store.delete_session_data(row.session.key.tool, &row.session.key.id);
    }
    let now = now_ms();
    for (key, stored) in &meta {
        if discovered_counts
            .get(key.tool.as_str())
            .copied()
            .unwrap_or(0)
            > 0
            && !discovered_keys.contains(key)
            && stored
                .last_seen_at
                .is_some_and(|seen| now - seen > PRUNE_GRACE_MS)
        {
            let _ = store.delete_session_data(key.tool, &key.id);
        }
    }
    rows.sort_by_key(|r| -r.session.updated_at_ms);
    Ok(rows)
}

fn cwd_definitively_missing(cwd: Option<&Path>) -> bool {
    let Some(cwd) = cwd else {
        return false;
    };
    if !matches!(cwd.try_exists(), Ok(false)) {
        return false;
    }
    if !cwd.parent().is_some_and(Path::exists) {
        return false;
    }
    let mut components = cwd.components();
    if components.next() == Some(std::path::Component::RootDir)
        && components
            .next()
            .is_some_and(|c| c.as_os_str() == "Volumes")
        && let Some(volume) = components.next()
        && !Path::new("/Volumes").join(volume.as_os_str()).exists()
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::ToolAdapter;
    use crate::core::{Enrichment, LaunchSpec, TitleKind, ToolId};
    use crate::store::upsert_session_facts;

    struct StubAdapter {
        sessions: Vec<SessionSummary>,
    }

    impl ToolAdapter for StubAdapter {
        fn id(&self) -> ToolId {
            ToolId::Claude
        }

        fn is_installed(&self) -> bool {
            true
        }

        fn store_roots(&self) -> Vec<PathBuf> {
            Vec::new()
        }

        fn discover(&self) -> anyhow::Result<Vec<SessionSummary>> {
            Ok(self.sessions.clone())
        }

        fn refresh_usage(
            &self,
            _session: &SessionSummary,
            _ckpt: &mut CheckpointTxn,
        ) -> anyhow::Result<Enrichment> {
            Ok(Enrichment::default())
        }

        fn liveness(&self, _session: &SessionSummary) -> Liveness {
            Liveness::Idle
        }

        fn resume_command(&self, _session: &SessionSummary) -> LaunchSpec {
            LaunchSpec {
                program: "true".to_string(),
                args: Vec::new(),
                cwd: PathBuf::from("/"),
            }
        }

        fn new_session_command(&self, cwd: &Path) -> LaunchSpec {
            LaunchSpec {
                program: "true".to_string(),
                args: Vec::new(),
                cwd: cwd.to_path_buf(),
            }
        }
    }

    fn test_store(name: &str) -> Store {
        let dir = PathBuf::from(".tmp/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join(format!("snapshot-{name}.db"));
        let _ = std::fs::remove_file(&db);
        Store::open(Some(&db)).unwrap()
    }

    fn summary(id: &str, title: Option<&str>) -> SessionSummary {
        SessionSummary {
            key: SessionKey {
                tool: ToolId::Claude,
                id: id.to_string(),
            },
            cwd: None,
            title: title.map(str::to_string),
            preview: None,
            model: None,
            origin_url: None,
            created_at_ms: None,
            updated_at_ms: 1,
            native_archived: false,
            native_tokens_used: None,
            usage_files: Vec::new(),
        }
    }

    fn adapters_with(sessions: Vec<SessionSummary>) -> Vec<Box<dyn ToolAdapter>> {
        vec![Box::new(StubAdapter { sessions })]
    }

    fn seed_meta(store: &Store, id: &str, last_seen_at: Option<i64>) {
        upsert_session_facts(
            store.conn(),
            ToolId::Claude,
            id,
            None,
            Some(("Mine", TitleKind::Custom)),
            None,
            None,
            None,
            None,
            "rename",
        )
        .unwrap();
        store
            .conn()
            .execute(
                "UPDATE sessions_meta SET last_seen_at = ?2 WHERE session_id = ?1",
                rusqlite::params![id, last_seen_at],
            )
            .unwrap();
    }

    #[test]
    fn prune_respects_the_grace_period_and_null_last_seen() {
        let mut store = test_store("prune-grace");
        let now = now_ms();
        seed_meta(&store, "gone-old", Some(now - PRUNE_GRACE_MS - 60_000));
        seed_meta(&store, "gone-recent", Some(now - 1_000));
        seed_meta(&store, "gone-null", None);
        let adapters = adapters_with(vec![summary("kept", None)]);
        collect_sessions_with(&adapters, &mut store, &mut |_| {}).unwrap();
        let meta = store.session_meta_map().unwrap();
        let has = |id: &str| {
            meta.contains_key(&SessionKey {
                tool: ToolId::Claude,
                id: id.to_string(),
            })
        };
        assert!(!has("gone-old"));
        assert!(has("gone-recent"));
        assert!(has("gone-null"));
        assert!(has("kept"));
    }

    #[test]
    fn stored_meta_survives_when_discovery_returns_nothing() {
        let mut store = test_store("prune-empty");
        seed_meta(&store, "precious", Some(1));
        let adapters = adapters_with(Vec::new());
        collect_sessions_with(&adapters, &mut store, &mut |_| {}).unwrap();
        let meta = store.session_meta_map().unwrap();
        assert_eq!(meta.len(), 1);
    }

    #[test]
    fn discovered_sessions_get_their_last_seen_stamped() {
        let mut store = test_store("touch-seen");
        seed_meta(&store, "kept", None);
        let adapters = adapters_with(vec![summary("kept", None)]);
        collect_sessions_with(&adapters, &mut store, &mut |_| {}).unwrap();
        let meta = store.session_meta_map().unwrap();
        let stored = meta
            .get(&SessionKey {
                tool: ToolId::Claude,
                id: "kept".to_string(),
            })
            .unwrap();
        assert!(stored.last_seen_at.is_some());
    }

    #[test]
    fn stored_custom_title_overrides_the_adapter_title() {
        let mut store = test_store("custom-title");
        seed_meta(&store, "titled", Some(now_ms()));
        let adapters = adapters_with(vec![summary("titled", Some("auto guess"))]);
        let rows = collect_sessions_with(&adapters, &mut store, &mut |_| {}).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session.title.as_deref(), Some("Mine"));
    }

    #[test]
    fn cwd_missing_detection_requires_an_existing_parent() {
        assert!(!cwd_definitively_missing(None));
        let existing = PathBuf::from(".tmp/fixtures");
        std::fs::create_dir_all(&existing).unwrap();
        assert!(!cwd_definitively_missing(Some(&existing)));
        let deleted = existing.join("snapshot-deleted-dir");
        let _ = std::fs::remove_dir_all(&deleted);
        assert!(cwd_definitively_missing(Some(&deleted)));
        let unmounted = existing.join("snapshot-no-mount").join("inner");
        assert!(!cwd_definitively_missing(Some(&unmounted)));
        assert!(!cwd_definitively_missing(Some(Path::new(
            "/Volumes/szpont-test-not-mounted/project"
        ))));
    }
}
