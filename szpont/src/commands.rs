use std::path::Path;

use serde::Serialize;

use crate::cli::{Cli, SessionsArgs};
use crate::core::snapshot::{SessionRow, collect_sessions};
use crate::core::{
    Liveness, SessionSummary, ToolId, Usage, format_age, format_tokens, now_ms, truncate,
};
use crate::store::Store;

const NO_TOOLS_HINT: &str = "no AI CLI session stores found (looked in ~/.claude/projects, ~/.codex, ~/.kimi-code/sessions; override with CLAUDE_CONFIG_DIR / CODEX_HOME / KIMI_CODE_HOME)";

pub fn sessions(cli: &Cli, args: &SessionsArgs) -> anyhow::Result<()> {
    let mut store = Store::open(cli.db.as_deref())?;
    if crate::adapters::installed().is_empty() {
        eprintln!("{NO_TOOLS_HINT}");
    }
    let mut rows = collect_sessions(&mut store, &mut |_| {})?;
    if !args.all {
        rows.retain(|r| !r.completed);
    }
    if let Some(repo) = &args.repo {
        let repo = repo.canonicalize().unwrap_or_else(|_| repo.clone());
        rows.retain(|r| {
            r.session
                .cwd
                .as_deref()
                .is_some_and(|cwd| cwd.starts_with(&repo))
        });
    }
    if args.json {
        print_json(&rows)
    } else {
        print_table(&rows);
        Ok(())
    }
}

pub fn complete(cli: &Cli, tool: &str, session_id: &str) -> anyhow::Result<()> {
    let tool = ToolId::parse(tool)?;
    let store = Store::open(cli.db.as_deref())?;
    store.mark_completed(tool, session_id, "cli")?;
    let session_id = crate::core::sanitize_for_terminal(session_id);
    println!("marked {} session {session_id} as completed", tool.as_str());
    Ok(())
}

pub fn reopen(cli: &Cli, tool: &str, session_id: &str) -> anyhow::Result<()> {
    let tool = ToolId::parse(tool)?;
    let store = Store::open(cli.db.as_deref())?;
    let reopened = store.reopen(tool, session_id)?;
    let session_id = crate::core::sanitize_for_terminal(session_id);
    if reopened {
        println!("reopened {} session {session_id}", tool.as_str());
    } else {
        println!(
            "{} session {session_id} had no szpont completed mark (a native archive flag cannot be cleared from szpont)",
            tool.as_str()
        );
    }
    Ok(())
}

pub fn limits(cli: &Cli, json: bool) -> anyhow::Result<()> {
    let store = Store::open(cli.db.as_deref())?;
    if crate::adapters::installed().is_empty() {
        eprintln!("{NO_TOOLS_HINT}");
    }
    let limits = crate::limits::collect(&store);
    if json {
        println!("{}", serde_json::to_string_pretty(&limits)?);
        return Ok(());
    }
    for tool in &limits {
        let plan = tool
            .plan
            .as_deref()
            .map(|p| format!(" ({} plan)", crate::core::sanitize_for_terminal(p)))
            .unwrap_or_default();
        println!("{}{plan} — {}", tool.tool.display_name(), tool.source);
        for window in &tool.windows {
            let pct = window.used_percent.map_or_else(
                || "?".to_string(),
                |p| format!("{p:.0}%{}", if window.estimated { " est" } else { "" }),
            );
            let tokens = window
                .tokens
                .map(|t| format!("  {} tokens", format_tokens(t)))
                .unwrap_or_default();
            let resets = window
                .resets_at
                .map(|r| {
                    let remaining = r.saturating_mul(1000).saturating_sub(now_ms());
                    if remaining > 0 {
                        format!("  resets in {}", format_age(remaining))
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();
            println!("  {:<5} {pct}{tokens}{resets}", window.label);
        }
        if let Some(note) = &tool.note {
            println!("  note: {}", crate::core::sanitize_for_terminal(note));
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct JsonRow<'a> {
    tool: ToolId,
    session_id: &'a str,
    cwd: Option<&'a Path>,
    title: Option<&'a str>,
    preview: Option<&'a str>,
    model: Option<&'a str>,
    created_at_ms: Option<i64>,
    updated_at_ms: i64,
    completed: bool,
    completed_at_ms: Option<i64>,
    native_archived: bool,
    tokens_total: Option<u64>,
    usage: Option<Usage>,
    context_tokens: Option<u64>,
    context_window: Option<u64>,
    liveness: Liveness,
}

fn print_json(rows: &[SessionRow]) -> anyhow::Result<()> {
    let out: Vec<JsonRow> = rows
        .iter()
        .map(|r| JsonRow {
            tool: r.session.key.tool,
            session_id: &r.session.key.id,
            cwd: r.session.cwd.as_deref(),
            title: r.session.title.as_deref(),
            preview: r.session.preview.as_deref(),
            model: r.session.model.as_deref(),
            created_at_ms: r.session.created_at_ms,
            updated_at_ms: r.session.updated_at_ms,
            completed: r.completed,
            completed_at_ms: r.completed_at,
            native_archived: r.session.native_archived,
            tokens_total: r.tokens_total(),
            usage: r.usage,
            context_tokens: r.context_tokens,
            context_window: r.context_window,
            liveness: r.liveness,
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn print_table(rows: &[SessionRow]) {
    let now = now_ms();
    for r in rows {
        let status = match (r.completed, r.liveness) {
            (true, _) => "DONE",
            (false, Liveness::Running) => "RUNNING",
            (false, Liveness::WaitingForInput) => "BLOCKED",
            (false, Liveness::Open) => "IDLE",
            (false, Liveness::Idle) => "",
        };
        println!(
            "{status:<8} {:<7} {:<44} {:<28} {:>10} {:>6}",
            r.session.key.tool.as_str(),
            crate::core::sanitize_for_terminal(&truncate(
                r.session.title.as_deref().unwrap_or("-"),
                44
            )),
            crate::core::sanitize_for_terminal(&truncate(&dir_label(&r.session), 28)),
            r.tokens_total()
                .map_or_else(|| "-".to_string(), format_tokens),
            format_age(now - r.session.updated_at_ms),
        );
    }
}

fn dir_label(session: &SessionSummary) -> String {
    session
        .cwd
        .as_deref()
        .and_then(Path::file_name)
        .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned())
}
