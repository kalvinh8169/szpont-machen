use serde_json::{Value, json};

const SUPPORTED_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
const DEFAULT_VERSION: &str = "2025-06-18";

const INSTRUCTIONS: &str = "\
szpont is a session manager for AI CLI tools. It tracks the sessions a user browses, resumes and \
archives in a terminal UI.

Marking a session completed is user-triggered only. Call mark_session_completed when the user asks \
for it in words - \"mark this done\", \"archive this session\", \"szpont complete\". Never call it \
because the work looks finished, and never at the end of a turn. A session archived early \
disappears from the view the user is watching.

Call report_session_start once when you begin a task, with your tool name and your working \
directory as cwd. The surrounding harness may have registered the session already; repeating it is \
harmless.

Prefer an exact session_id on every call. Without one, szpont falls back to the most recently \
updated session of that tool in that cwd - ambiguous when several sessions share a repository, and \
it can archive the wrong one. If your context already names a szpont session id, pass it.

reopen_session needs an explicit session_id. It clears only szpont's own completed mark; a session \
archived inside the AI CLI tool itself cannot be reopened from here.

There is deliberately no tool for listing sessions, so titles and paths of unrelated work stay \
private. Do not try to enumerate sessions.

A running szpont UI reflects your writes on its next refresh tick, 15 seconds by default. Seeing no \
immediate change is expected - do not retry.";

pub fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        DEFAULT_VERSION
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "szpont",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": INSTRUCTIONS
    })
}
