use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

use crate::core::{SessionKey, ToolId};

const SUPPORTED_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct SessionAllowlist {
    entries: HashMap<SessionKey, SessionAliases>,
}

#[derive(Clone, Debug)]
pub struct SessionAliases {
    pub project: String,
    pub title: Option<String>,
    pub archived: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistFile {
    version: u32,
    sessions: Vec<AllowlistEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistEntry {
    tool: ToolId,
    session_id: String,
    real_title: Option<String>,
    real_path: Option<String>,
    project_alias: String,
    title_alias: Option<String>,
    #[serde(default)]
    archived: bool,
    enabled: bool,
    notes: Option<String>,
}

impl SessionAllowlist {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = crate::core::read_to_string_capped(path, crate::core::MAX_CONFIG_BYTES)
            .ok_or_else(|| anyhow::anyhow!("cannot read allowlist {}", path.display()))?;
        let file: AllowlistFile = serde_json::from_str(&text)
            .map_err(|err| anyhow::anyhow!("invalid allowlist {}: {err}", path.display()))?;
        if file.version != SUPPORTED_VERSION {
            anyhow::bail!(
                "unsupported allowlist version {}, expected {SUPPORTED_VERSION}",
                file.version
            );
        }

        let mut seen = HashSet::new();
        let mut entries = HashMap::new();
        for entry in file.sessions {
            let id = entry.session_id.trim();
            if !crate::core::valid_session_id(id, None) {
                anyhow::bail!(
                    "invalid {} session id {:?} in allowlist",
                    entry.tool.as_str(),
                    entry.session_id
                );
            }
            let key = SessionKey {
                tool: entry.tool,
                id: id.to_string(),
            };
            if !seen.insert(key.clone()) {
                anyhow::bail!("duplicate allowlist entry {}:{}", key.tool.as_str(), key.id);
            }
            let project = clean_required_alias(&entry.project_alias, "project_alias", &key)?;
            let title = entry
                .title_alias
                .as_deref()
                .map(|value| clean_required_alias(value, "title_alias", &key))
                .transpose()?;
            let _context_only = (
                entry.real_title.as_deref(),
                entry.real_path.as_deref(),
                entry.notes.as_deref(),
            );
            if entry.enabled {
                entries.insert(
                    key,
                    SessionAliases {
                        project,
                        title,
                        archived: entry.archived,
                    },
                );
            }
        }
        Ok(Self { entries })
    }

    pub fn aliases(&self, key: &SessionKey) -> Option<&SessionAliases> {
        self.entries.get(key)
    }

    pub fn contains(&self, key: &SessionKey) -> bool {
        self.entries.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn clean_required_alias(value: &str, field: &str, key: &SessionKey) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!(
            "empty {field} for {}:{} in allowlist",
            key.tool.as_str(),
            key.id
        );
    }
    Ok(crate::core::clean_text_fact(
        value,
        crate::core::MAX_TEXT_FACT_CHARS,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(name: &str, text: &str) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(".tmp/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("allowlist-{name}.json"));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn loads_enabled_entries_and_aliases() {
        let path = write_fixture(
            "valid",
            r#"{"version":1,"sessions":[
                {"tool":"claude","session_id":"abc","real_title":"old","real_path":"/real","project_alias":" Demo ","title_alias":" Safe title ","archived":true,"enabled":true,"notes":null},
                {"tool":"codex","session_id":"disabled","real_title":null,"real_path":null,"project_alias":"Hidden","title_alias":null,"enabled":false,"notes":"skip"}
            ]}"#,
        );
        let allowlist = SessionAllowlist::load(&path).unwrap();
        let key = SessionKey {
            tool: ToolId::Claude,
            id: "abc".to_string(),
        };
        assert_eq!(allowlist.len(), 1);
        assert_eq!(allowlist.aliases(&key).unwrap().project, "Demo");
        assert_eq!(
            allowlist.aliases(&key).unwrap().title.as_deref(),
            Some("Safe title")
        );
        assert!(allowlist.aliases(&key).unwrap().archived);
    }

    #[test]
    fn rejects_duplicates_even_when_one_is_disabled() {
        let path = write_fixture(
            "duplicate",
            r#"{"version":1,"sessions":[
                {"tool":"kimi","session_id":"session_x","real_title":null,"real_path":null,"project_alias":"One","title_alias":null,"enabled":true,"notes":null},
                {"tool":"kimi","session_id":"session_x","real_title":null,"real_path":null,"project_alias":"Two","title_alias":null,"enabled":false,"notes":null}
            ]}"#,
        );
        assert!(
            SessionAllowlist::load(&path)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }

    #[test]
    fn rejects_bad_version_and_empty_alias() {
        let version = write_fixture("version", r#"{"version":2,"sessions":[]}"#);
        assert!(
            SessionAllowlist::load(&version)
                .unwrap_err()
                .to_string()
                .contains("version")
        );
        let alias = write_fixture(
            "empty",
            r#"{"version":1,"sessions":[{"tool":"codex","session_id":"abc","real_title":null,"real_path":null,"project_alias":" ","title_alias":null,"enabled":true,"notes":null}]}"#,
        );
        assert!(
            SessionAllowlist::load(&alias)
                .unwrap_err()
                .to_string()
                .contains("project_alias")
        );
    }
}
