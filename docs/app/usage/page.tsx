import type { Metadata } from "next";

export const metadata: Metadata = { title: "Usage — szpont machen docs" };

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
szpont complete <claude|codex|kimi> <session-id> # mark completed
szpont reopen <claude|codex|kimi> <session-id>   # clear the completed mark
szpont install-mcp [--tool all|claude|codex|kimi] [--dry-run]
szpont mcp                                       # MCP server on stdio (used by the tools)
szpont completions <shell>                       # shell completions (bash, zsh, fish, …)`}</code>
      </pre>

      <h2>Global flags</h2>
      <pre>
        <code>{`--db <PATH>         # override ~/.szpont/szpont.db
--refresh-secs <N>  # idle rescan interval (default 15)
--no-watch          # disable the file watcher
--log <PATH>        # append warnings to a log file`}</code>
      </pre>

      <h2>Shell completions</h2>
      <p>zsh with Homebrew, for example:</p>
      <pre>
        <code>
          szpont completions zsh &gt;
          /opt/homebrew/share/zsh/site-functions/_szpont
        </code>
      </pre>

      <h2>Session status</h2>
      <p>The STATUS column shows:</p>
      <table>
        <tbody>
          <tr>
            <th>Status</th>
            <th>Meaning</th>
          </tr>
          <tr>
            <td>
              <code>RUNNING</code>
            </td>
            <td>the tool is working</td>
          </tr>
          <tr>
            <td>
              <code>BLOCKED</code>
            </td>
            <td>waiting for your input (reported for Claude Code only)</td>
          </tr>
          <tr>
            <td>
              <code>IDLE</code>
            </td>
            <td>open at the prompt</td>
          </tr>
          <tr>
            <td>
              <code>DONE</code>
            </td>
            <td>
              completed (headless output, <code>szpont sessions --all</code>)
            </td>
          </tr>
          <tr>
            <td>(empty)</td>
            <td>session not loaded in any process</td>
          </tr>
        </tbody>
      </table>
    </>
  );
}
