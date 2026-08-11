import type { Metadata } from "next";

export const metadata: Metadata = { title: "Usage — szpont docs" };

export default function Usage() {
  return (
    <>
      <h1>Usage</h1>

      <h2>TUI</h2>
      <pre>
        <code>{`szpont              # repo view when the current directory is a repo with sessions
szpont --global     # all sessions across all repos
szpont --repo PATH  # sessions of a specific repo`}</code>
      </pre>

      <h2>Headless commands</h2>
      <pre>
        <code>{`szpont sessions [--json] [--all] [--repo PATH]   # list sessions
szpont limits [--json]                           # rate-limit / usage state
szpont complete <tool> <session-id>              # mark completed
szpont reopen <tool> <session-id>                # clear the completed mark
szpont install-mcp [--tool all|claude|codex|kimi] [--dry-run]
szpont mcp                                       # MCP server on stdio (used by the tools)
szpont completions <shell>                       # shell completions (bash, zsh, fish, …)`}</code>
      </pre>

      <h2>Shell completions</h2>
      <p>zsh with Homebrew, for example:</p>
      <pre>
        <code>
          szpont completions zsh &gt;
          /opt/homebrew/share/zsh/site-functions/_szpont
        </code>
      </pre>
    </>
  );
}
