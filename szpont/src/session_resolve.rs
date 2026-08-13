use std::path::{Path, PathBuf};

use crate::adapters;
use crate::core::ToolId;
use crate::store::Store;

pub fn valid_session_id(id: &str) -> bool {
    crate::core::valid_session_id(id, None)
}

pub fn newest_session_in(store: &Store, tool: ToolId, cwd: &str) -> Option<String> {
    let cwd = Path::new(cwd)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(cwd));
    let meta = store.session_meta_map().ok()?;
    let adapter = adapters::by_id(tool)?;
    let sessions = adapter.discover().ok()?;
    sessions
        .into_iter()
        .filter(|s| {
            let stored_cwd = meta
                .get(&s.key)
                .and_then(|m| m.cwd.clone())
                .map(PathBuf::from);
            s.cwd.as_deref().is_some_and(|c| c == cwd)
                || stored_cwd.as_deref().is_some_and(|c| c == cwd)
        })
        .max_by_key(|s| s.updated_at_ms)
        .map(|s| s.key.id)
}
