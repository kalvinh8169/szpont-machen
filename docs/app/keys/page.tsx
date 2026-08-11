import type { Metadata } from "next";

export const metadata: Metadata = { title: "TUI keys — szpont docs" };

export default function Keys() {
  return (
    <>
      <h1>TUI keys</h1>

      <table>
        <tbody>
          <tr>
            <th>Key</th>
            <th>Action</th>
          </tr>
          <tr>
            <td>
              <code>↑</code> / <code>↓</code>
            </td>
            <td>move selection</td>
          </tr>
          <tr>
            <td>
              <code>Enter</code>
            </td>
            <td>
              resume the selected session (suspends szpont, returns on exit)
            </td>
          </tr>
          <tr>
            <td>
              <code>E</code>
            </td>
            <td>resume and replace the szpont process</td>
          </tr>
          <tr>
            <td>
              <code>n</code>
            </td>
            <td>new session (tool picker)</td>
          </tr>
          <tr>
            <td>
              <code>c</code>
            </td>
            <td>
              mark as completed: enters marking mode on the selected session;
              keep marking with <code>c</code>, move with <code>↑</code> /{" "}
              <code>↓</code>, confirm with <code>Enter</code>, cancel with{" "}
              <code>Esc</code>
            </td>
          </tr>
          <tr>
            <td>
              <code>u</code>
            </td>
            <td>reopen (archive screen)</td>
          </tr>
          <tr>
            <td>
              <code>d</code>
            </td>
            <td>
              mark for deletion (works on every session screen, including the
              live list): same marking flow with <code>d</code> /{" "}
              <code>Enter</code> / <code>Esc</code>. Deletion removes the
              session from szpont <strong>and</strong> from the tool&apos;s own
              storage (<code>codex delete</code>; file removal for Claude Code
              and Kimi, which have no delete command). Sessions still open in a
              tool are skipped.
            </td>
          </tr>
          <tr>
            <td>
              <code>v</code>
            </td>
            <td>
              tree view — sessions nested by directory with branch lines; works
              from monitor, repo scope and archive; <code>v</code> or{" "}
              <code>Esc</code> returns
            </td>
          </tr>
          <tr>
            <td>
              <code>p</code>
            </td>
            <td>
              toggle scope — global (all repos) vs local (current repo); shown
              only when inside a repo
            </td>
          </tr>
          <tr>
            <td>
              <code>o</code>
            </td>
            <td>
              options screen — set the context window per model (e.g.{" "}
              <code>200K</code>, <code>1M</code>); persisted, used for the CTX
              bar when the tool does not report a window (Claude Code)
            </td>
          </tr>
          <tr>
            <td>
              <code>a</code>
            </td>
            <td>archive screen</td>
          </tr>
          <tr>
            <td>
              <code>m</code>
            </td>
            <td>
              rename the selected session (szpont-side custom title; survives
              rescans)
            </td>
          </tr>
          <tr>
            <td>
              <code>Esc</code>
            </td>
            <td>
              close popup · back from archive · clear marks · clear filters
            </td>
          </tr>
          <tr>
            <td>
              <code>l</code>
            </td>
            <td>limit usage popup</td>
          </tr>
          <tr>
            <td>
              <code>i</code>
            </td>
            <td>session detail popup</td>
          </tr>
          <tr>
            <td>
              <code>f</code>
            </td>
            <td>
              find (fzf-style fuzzy search: subsequence match over title,
              prompt, path and tool; filters live while typing, best matches
              first; works on every session view)
            </td>
          </tr>
          <tr>
            <td>
              <code>Tab</code>
            </td>
            <td>cycle tool filter</td>
          </tr>
          <tr>
            <td>
              <code>r</code>
            </td>
            <td>
              force rescan (min 3s between scans; idle tick every 15s; file
              changes trigger scans automatically)
            </td>
          </tr>
          <tr>
            <td>
              double <code>Ctrl+C</code>
            </td>
            <td>quit</td>
          </tr>
        </tbody>
      </table>

      <h2>Reading the display</h2>
      <p>The STATUS column shows:</p>
      <table>
        <tbody>
          <tr>
            <th>Status</th>
            <th>Meaning</th>
          </tr>
          <tr>
            <td>
              <code>RUNNING</code> (green)
            </td>
            <td>the tool is working</td>
          </tr>
          <tr>
            <td>
              <code>BLOCKED</code> (yellow)
            </td>
            <td>waiting for your input</td>
          </tr>
          <tr>
            <td>
              <code>IDLE</code> (cyan)
            </td>
            <td>open at the prompt</td>
          </tr>
          <tr>
            <td>(empty)</td>
            <td>session not loaded in any process</td>
          </tr>
        </tbody>
      </table>
      <p>
        The list always sits in a tinted frame that names the mode: neutral
        &quot; sessions &quot;, magenta &quot; archive &quot;, yellow
        &quot; marking as completed &quot;, red &quot; marking for deletion
        &quot;, green &quot; new session &quot;, cyan &quot; options &quot;.
      </p>
      <p>
        Background rescans never reorder the list under you: updates that keep
        the visible row order (token counts, status changes) apply live, while
        updates that would add, remove or reorder rows are held — the top bar
        shows &quot;⟳ list changed — press r to load&quot;. <code>r</code>{" "}
        applies the held update instantly and rescans; your own actions
        (resume, complete, delete) always apply immediately too.
      </p>
      <p>
        The <code>CTX</code> column shows how full the session context window
        is: a percentage when the window size is known (Codex reports it in its
        rollout; Kimi&apos;s comes from <code>~/.kimi-code/config.toml</code>),
        otherwise the raw token count of the last request (Claude Code does not
        record its window size locally — set it once per model in the{" "}
        <code>o</code> options screen).
      </p>
    </>
  );
}
