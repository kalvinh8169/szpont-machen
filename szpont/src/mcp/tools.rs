use serde_json::{Value, json};

use crate::core::{SessionKey, ToolId, now_ms};
use crate::session_resolve::{newest_session_in, valid_session_id};
use crate::store::Store;

const MAX_AUDIT_PAYLOAD_BYTES: usize = 4096;
const MAX_AUDIT_FIELD_CHARS: usize = 500;

pub fn definitions() -> Vec<Value> {
    let tool_enum = json!({ "type": "string", "enum": ["claude", "codex", "kimi"] });
    vec![
        json!({
            "name": "report_session_start",
            "description": "Report that an AI CLI session started, so the szpont session manager tracks it. Pass your tool name, your working directory as cwd, and your session id if you know it. Call this once at the start of a task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool": tool_enum,
                    "session_id": { "type": "string", "description": "The session id, if known" },
                    "cwd": { "type": "string", "description": "Absolute path of the session working directory" },
                    "title": { "type": "string", "description": "Short human-readable task title" }
                },
                "required": ["tool", "cwd"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "mark_session_completed",
            "description": "Mark an AI CLI session as completed in the szpont session manager; it moves from the active monitor to the archive. Call this when the task the session was opened for is done. If session_id is unknown, pass cwd and the most recent session of that tool in that directory is marked.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool": tool_enum,
                    "session_id": { "type": "string" },
                    "cwd": { "type": "string" },
                    "reason": { "type": "string", "description": "Why the session is complete" }
                },
                "required": ["tool"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "reopen_session",
            "description": "Clear the completed mark of a session in the szpont session manager so it shows in the active monitor again.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool": tool_enum,
                    "session_id": { "type": "string" }
                },
                "required": ["tool", "session_id"],
                "additionalProperties": false
            }
        }),
    ]
}

pub fn call(store: &mut Store, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let outcome = match name {
        "report_session_start" => report_session_start(store, &args),
        "mark_session_completed" => mark_session_completed(store, &args),
        "reopen_session" => reopen_session(store, &args),
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };
    Ok(match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(err) => json!({
            "content": [{ "type": "text", "text": format!("error: {err:#}") }],
            "isError": true
        }),
    })
}

fn report_session_start(store: &Store, args: &Value) -> anyhow::Result<String> {
    let tool = parse_tool(args)?;
    let cwd = args
        .get("cwd")
        .and_then(Value::as_str)
        .map(crate::core::sanitize_for_terminal)
        .ok_or_else(|| anyhow::anyhow!("cwd is required"))?;
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .map(crate::core::sanitize_for_terminal);
    let session_id = explicit_session_id(args).or_else(|| newest_session_in(store, tool, &cwd));
    audit(
        store,
        tool,
        session_id.as_deref(),
        "report_session_start",
        args,
    )?;
    match session_id {
        Some(id) => {
            let stored_cwd = store.session_meta_map().ok().and_then(|meta| {
                meta.get(&SessionKey {
                    tool,
                    id: id.clone(),
                })
                .and_then(|m| m.cwd.clone())
            });
            let cwd_update = if stored_cwd.is_some() {
                None
            } else {
                Some(cwd.as_str())
            };
            crate::store::upsert_session_facts(
                store.conn(),
                tool,
                &id,
                cwd_update,
                title.as_deref().map(|t| (t, crate::core::TitleKind::Auto)),
                None,
                None,
                None,
                None,
                "mcp",
            )?;
            Ok(format!("tracking {} session {id}", tool.as_str()))
        }
        None => Ok(format!(
            "no {} session found in {cwd} yet; it will be picked up by the next scan",
            tool.as_str()
        )),
    }
}

fn mark_session_completed(store: &Store, args: &Value) -> anyhow::Result<String> {
    let tool = parse_tool(args)?;
    let session_id = explicit_session_id(args).or_else(|| {
        args.get("cwd")
            .and_then(Value::as_str)
            .and_then(|cwd| newest_session_in(store, tool, cwd))
    });
    let Some(id) = session_id else {
        anyhow::bail!("could not resolve a session: pass session_id, or cwd of the session");
    };
    audit(store, tool, Some(&id), "mark_session_completed", args)?;
    store.mark_completed(tool, &id, "mcp")?;
    Ok(format!(
        "{} session {id} marked as completed and moved to the archive",
        tool.as_str()
    ))
}

fn reopen_session(store: &Store, args: &Value) -> anyhow::Result<String> {
    let tool = parse_tool(args)?;
    let id = explicit_session_id(args).ok_or_else(|| anyhow::anyhow!("session_id is required"))?;
    audit(store, tool, Some(&id), "reopen_session", args)?;
    if store.reopen(tool, &id)? {
        Ok(format!("{} session {id} reopened", tool.as_str()))
    } else {
        Ok(format!(
            "{} session {id} had no szpont completed mark",
            tool.as_str()
        ))
    }
}

fn parse_tool(args: &Value) -> anyhow::Result<ToolId> {
    let tool = args
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("tool is required"))?;
    ToolId::parse(tool)
}

fn explicit_session_id(args: &Value) -> Option<String> {
    args.get("session_id")
        .and_then(Value::as_str)
        .filter(|s| valid_session_id(s))
        .map(std::string::ToString::to_string)
}

fn audit(
    store: &Store,
    tool: ToolId,
    session_id: Option<&str>,
    kind: &str,
    payload: &Value,
) -> anyhow::Result<()> {
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .map(|v| crate::core::clean_text_fact(v, MAX_AUDIT_FIELD_CHARS))
    };
    let audited = json!({
        "tool": tool.as_str(),
        "session_id": session_id,
        "cwd": field("cwd"),
        "title": field("title"),
        "reason": field("reason"),
    });
    let mut payload_json = serde_json::to_string(&audited)?;
    if payload_json.len() > MAX_AUDIT_PAYLOAD_BYTES {
        let mut cut = MAX_AUDIT_PAYLOAD_BYTES;
        while !payload_json.is_char_boundary(cut) {
            cut -= 1;
        }
        payload_json.truncate(cut);
    }
    store.conn().execute(
        "INSERT INTO mcp_events (at, tool, session_id, kind, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![now_ms(), tool.as_str(), session_id, kind, payload_json],
    )?;
    let _ = store.conn().execute(
        "DELETE FROM mcp_events WHERE id <= (
           SELECT COALESCE(MAX(id), 0) - ?1 FROM mcp_events
         )",
        rusqlite::params![super::MAX_AUDIT_ROWS],
    );
    Ok(())
}
