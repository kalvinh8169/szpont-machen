# szpont 🐝

```
      _  _
      \\//
    .-(oo)-.
  =[ ≡≡≡≡≡ ]=
    '-(__)-'
     szpont
```

szpont (short for "szpont machen", pronounced *"shpont mah-khen"*) is a terminal manager for AI CLI tool sessions. It tracks sessions of **Claude Code**, **Codex CLI** and **Kimi Code** on your machine, shows token usage and rate-limit state, resumes any session with the right tool, and archives sessions you are done with.

## Install

```sh
cargo install --path szpont
```

Optional: exact limit data from the providers' usage endpoints — Anthropic's OAuth endpoint (token from the macOS Keychain, falling back to `~/.claude/.credentials.json`) and Kimi's `/usages` endpoint (token from `~/.kimi-code/credentials/kimi-code.json`). Both are unofficial and degrade silently:

```sh
cargo install --path szpont --features online-limits
```

## Usage

```sh
szpont              # TUI; repo view when the current directory is a repo with sessions
szpont --global     # TUI; all sessions across all repos
szpont --repo PATH  # TUI; sessions of a specific repo
```

Headless commands:

```sh
szpont sessions [--json] [--all] [--repo PATH]   # list sessions
szpont limits [--json]                           # rate-limit / usage state
szpont complete <tool> <session-id>              # mark completed
szpont reopen <tool> <session-id>                # clear the completed mark
szpont install-mcp [--tool all|claude|codex|kimi] [--dry-run]
szpont mcp                                       # MCP server on stdio (used by the tools)
szpont completions <shell>                       # shell completions (bash, zsh, fish, …)
```

Shell completions, zsh with Homebrew for example:

```sh
szpont completions zsh > /opt/homebrew/share/zsh/site-functions/_szpont
```

## Keys

| Key | Action |
|---|---|
| `↑` / `↓` | move selection |
| `Enter` | resume the selected session (suspends szpont, returns on exit) |
| `E` | resume and replace the szpont process |
| `n` | new session (tool picker) |
| `c` | mark as completed: enters marking mode on the selected session; keep marking with `c`, move with `↑` / `↓`, confirm with `Enter`, cancel with `Esc` |
| `u` | reopen (archive screen) |
| `d` | mark for deletion (works on every session screen, including the live list): same marking flow with `d` / `Enter` / `Esc`. Deletion removes the session from szpont **and** from the tool's own storage (`codex delete`; file removal for Claude Code and Kimi, which have no delete command). Sessions still open in a tool are skipped. |
| `v` | tree view — sessions nested by directory with branch lines; works from monitor, repo scope and archive; `v` or `Esc` returns |
| `p` | toggle scope — global (all repos) vs local (current repo); shown only when inside a repo |
| `o` | options screen — set the context window per model (e.g. `200K`, `1M`); persisted, used for the CTX bar when the tool does not report a window (Claude Code) |
| `a` | archive screen |
| `m` | rename the selected session (szpont-side custom title; survives rescans) |
| `Esc` | close popup · back from archive · clear marks · clear filters |
| `l` | limit usage popup |
| `i` | session detail popup |
| `f` | find (fzf-style fuzzy search: subsequence match over title, prompt, path and tool; filters live while typing, best matches first; works on every session view) |
| `Tab` | cycle tool filter |
| `r` | force rescan (min 3s between scans; idle tick every 15s; file changes trigger scans automatically) |
| double `Ctrl+C` | quit |

The STATUS column shows: `RUNNING` (green, the tool is working) · `BLOCKED` (yellow, waiting for your input) · `IDLE` (cyan, open at the prompt) · empty (session not loaded in any process).

The list always sits in a tinted frame that names the mode: neutral " sessions ", magenta " archive ", yellow " marking as completed ", red " marking for deletion ", green " new session ", cyan " options ".

Background rescans never reorder the list under you: updates that keep the visible row order (token counts, status changes) apply live, while updates that would add, remove or reorder rows are held — the top bar shows "⟳ list changed — press r to refresh". `r` applies the held update instantly and rescans; your own actions (resume, complete, delete) always apply immediately too.

The `CTX` column shows how full the session context window is: a percentage when the window size is known (Codex reports it in its rollout; Kimi's comes from `~/.kimi-code/config.toml`), otherwise the raw token count of the last request (Claude Code does not record its window size locally — set it once per model in the `o` options screen).

## How it works

szpont scans each tool's native session store read-only:

| Tool | Sessions | Usage | Live status | Resume |
|---|---|---|---|---|
| Claude Code | `~/.claude/projects/**/*.jsonl` | `assistant` entries, deduplicated by `requestId`, subagents included | `~/.claude/sessions/<pid>.json` + PID validation | `claude --resume <id>` |
| Codex | `~/.codex/state_*.sqlite` `threads` table (read-only) | last cumulative `token_count` event | rollout mtime | `codex resume <id>` |
| Kimi Code | `~/.kimi-code/sessions/wd_*/session_*/state.json` | sum of `usage.record` events in `wire.jsonl` | `state.json` mtime | `kimi -S <id>` |

Parsing is incremental: per-file byte offsets are checkpointed in `~/.szpont/szpont.db` (SQLite, WAL), so after the first scan a refresh reads only appended data. szpont never writes into another tool's files. Sessions whose working directory no longer exists on disk are hidden from every view and their szpont-side metadata is removed, since they cannot be resumed; metadata of sessions deleted outside szpont is garbage-collected the same way (the tools' own files are never touched by cleanup).

Development: `cargo fmt` (4-space indentation, `rustfmt.toml`) and `cargo lint` (clippy with `pedantic` denied, see `Cargo.toml` `[lints]`). Licensed under The Unlicense (public domain).

"Completed" is a szpont-side mark stored in `~/.szpont/szpont.db`; sessions archived inside Codex or Kimi themselves are also treated as completed. An existing `~/.mleko` data directory (the tool's previous name) is migrated automatically on first run.

## Limits

- **Codex**: exact values (`used_percent`, window, reset time, plan) parsed from `rate_limits` events in the newest rollout file.
- **Claude Code**: token totals per rolling 5h / 7d window computed from local logs. Percentages are relative to the largest window ever observed and are labeled `est`; with `--features online-limits` the exact utilization comes from the OAuth usage endpoint — every window it reports is shown (5h, week, per-model weeks, extra usage).
- **Kimi Code**: no limit data on disk; with `--features online-limits` the quota cycle and every windowed limit (e.g. the 5h window) come from Kimi's `/usages` endpoint, with the membership level as the plan.

## MCP integration

`szpont install-mcp` registers `szpont mcp` with all three tools (`claude mcp add`, `codex mcp add`, `~/.kimi-code/mcp.json`). Agents can then call:

- `report_session_start` — track a session as soon as it starts
- `mark_session_completed` — move a finished session to the archive (works with just `cwd` if the agent does not know its session id)
- `reopen_session` — undo a completed mark

There is deliberately no `list_sessions` tool: connected agents can report and complete sessions, but they cannot enumerate sessions from other repos (titles and paths of unrelated private work stay private).

The MCP server writes to the shared database; the TUI picks changes up on its next refresh tick (15s default, `--refresh-secs`).
