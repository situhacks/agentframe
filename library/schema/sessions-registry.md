# Sessions registry

Shapes `system/sessions/registry.md` (runtime state, gitignored). Owned by `af session`; never edit it by hand.

```
| name | role | session_id | model | started | last_seen | status | handoff |
|---|---|---|---|---|---|---|---|
| af-main | main | <uuid> | opus | 2026-10-25T09:12 | 2026-10-25T17:40 | live | system/sessions/af-main/handoff.md |
```

- One row per name. Re-registering a name replaces its row and keeps its folder.
- `name`: the chat's name as other chats address it with `SendMessage`: the VS Code session title (set with `/rename`), or `claude --name` from a terminal. One of `af-main`, `life/<topic>`, `project/<slug>`, `system/<topic>`, or `worker/<slug>`; a segment is `[A-Za-z0-9][A-Za-z0-9_.-]*`. The main chat is never named `main`, which `SendMessage` reserves for the sender itself; a legacy `main` row still resolves, and its session start asks for the rename to `af-main`.
- The main chat is the row whose role is `main` and whose name has no `/`; `life/*` rows share the role but are side chats.
- `role` follows the prefix: `af-main`, a legacy `main`, and `life/*` are `main`, `project/*` is `operator`, `system/*` is `builder`, `worker/*` is `worker`.
- `model`: what main proposed, or `-`.
- `class`: see [Session classes](#session-classes).
- `status`: `live` (a hook touched the row within two hours), `idle` (older, not ended), or `ended <ts> <reason>` with reason `clear`, `resume`, `logout`, `exit`, or `other`.
- Timestamps are local ISO minutes.

Each name has a folder, `system/sessions/<name>/`, holding an optional `brief.md` ([`handoff.md`](handoff.md) has both shapes), the chat's `handoff.md`, and for `system/*` and `worker/*` an optional `report.md`.

## Session classes

A class says how much context a chat should carry. It is not stored in the registry: the name gives the default, and a `class: main|side|build` line in the chat's `brief.md` frontmatter overrides it (`af session register <name> --session <id> --class <c>` writes that line into the brief; it refuses when there is no brief, and on a value outside the three).

| Class | Default for | Window | Soft nudge | Hard nudge |
|---|---|---|---|---|
| `main` | `af-main` (and a legacy `main`) | 350k | none | none |
| `side` | `life/*`, `worker/*` | 300k | none | none |
| `build` | `project/*`, `system/*` | 1M | 400k | 600k |

A hook cannot set a window or start a compaction. The window is the operator's (`/autocompact`); Claude Code's auto-compact handles `main` and `side`. For `build`, `af hook post-tool-use` runs after every tool, reads the last main-chain assistant usage from the transcript (input, cache read and cache creation tokens), and adds one nudge to the chat: soft once at the soft threshold, hard at the hard threshold and again each 50k above it. Each firing is claimed by creating a marker file in `system/logs/context-nudge/<session id>/` (`soft`, and `hard-<tokens>` for the hard threshold plus each 50k), so parallel tool calls say it once; a jump over several buckets fires the highest. Falling below the hard threshold removes the hard markers, falling below the soft threshold (or `af hook post-compact`, after every compaction) removes all of them. `af hook pre-compact` prints the chat's standing summary instructions (its name, role, brief, handoff and plan paths, the current step, decisions, open questions, files touched) on every compaction, manual or automatic. `af session context [name]` prints a chat's class, measured context, window and thresholds, and the auto-compact window that applies to its model (`modelSettings.<model id>.autoCompactWindow` in the user-level Claude settings, which it only reads): a chat that auto-compacts below its hard threshold never reaches that nudge, and the output says so.

## The Claude panel

`system/tools/claude_panel.ps1` drives the Claude Code panel of the VS Code window whose title holds the vault folder name, through Windows UI Automation and real key events. Actions: `list`, `send -Session <id> -TextFile <path>`, `new -TextFile <path> -Model <name>`, `model -Session <id> [-Model <name>]` (without `-Model` it lists the picker's options), and `selftest`, which the operator runs by hand on a scratch chat it creates, set to a Haiku option before its first message. It touches no other window, has no pointer action, and reads text from a UTF-8 file, never an argument. `af session compact` (on a running chat), `rename`, `start` and `panel` call it; `AF_PANEL_DRIVER` names another program, as the tests do.

- A text that starts with `/` is typed with one leading space (the chat still runs it as a command); without it the slash palette takes the keystrokes. A palette item is never invoked, because that sends the bare command at once. A line break is typed as Shift+Enter.
- A new chat inherits the default model, which is the expensive one, so `new` and `af session start` require a model (`--model`); `af session start` without one refuses and lists the picker's options. The driver sets and confirms the model on the new tab before the first message exists.
- The canary: before any real text the driver types one space and reads the input's value back through UI Automation. Only a value that is exactly that space proves the keys reach the message input; otherwise nothing more is typed and it exits 5, naming what had the keyboard focus. The canary is the leading space of a text that starts with `/`, and is taken back with one Backspace, verified, for any other text. After the whole text is typed the value is read back and must equal the intended text before Enter is pressed; on a mismatch the driver presses nothing, clears only what it typed (and only with the focus verified on the message input) and exits 5.
- Guards, each checked again right before every batch of keys: the operator has been idle for 120 seconds, not counting the driver's own input; for `send` and `model`, the chat's row and its `sessions/` record both say idle; the chat's input is empty; the foreground window is the VS Code window, the focus is the message input and the active chat is the target; no operator input since the checks. The first two wait up to `-WaitMinutes` (default 20) and then exit 3.
- Exit codes: 0 sent and the input cleared; 1 failed or unconfirmed; 2 window, chat or control not found (a chat running in a terminal has no panel row); 3 operator active or chat busy for the whole wait; 4 a draft in the chat's input; 5 a safety check failed. `af` turns 3, 4 and 5 into refusals; on any of them nothing was typed unless the message says text was left in the input.
- A run holds the screen for about 1.5 to 2.5 seconds, restores the previous chat, selected tab and foreground window, and appends one line to `system/logs/claude-panel.log`: time, action, session id, exit code, milliseconds held. Message text is never logged.
- The verbs mostly wait, so an orchestrator hands them to a Sonnet subagent and keeps its own context.

## Start a worker

Register it, then start it with Bash `run_in_background`, so its exit wakes the chat that started it:

```
af session register worker/<x> --session <new-uuid> --model <m> --brief <path>
claude -p "<goal>" --session-id <uuid> --agent <role> --name worker/<x> --model <m> --permission-mode acceptEdits --strict-mcp-config --allowedTools "<the commands it runs>"
```

Use `claude -p`, not `claude --bg`, which refuses a workspace whose trust prompt was never accepted in a terminal. Put the prompt first: `--allowedTools` and `--tools` take every word after them. `--strict-mcp-config` leaves it no connector; drop it only for a brief that names one. The exit notification is the worker's return; a `SendMessage` from a worker in another permission mode is held.

## Reach a chat

`af session reach <name>` prints one of these and what to do next. `SendMessage` answers `success: true` in every case but `closed`, so its result never proves a turn started; `af session reach <name> --confirm "<phrase>" --since <time>` does, with the `--since` value the first reading printed before you sent.

| State | Meaning | What to do |
|---|---|---|
| `live-idle` | Running, nothing in progress, same permission class | Send; a new turn starts there |
| `live-busy` | Running a turn, same permission class | Send; it lands between tool calls in that turn |
| `live-waiting` | Stopped at a permission prompt or question | Tell him; a message waits behind his answer |
| `holding` | The sender and the target differ in bypass versus prompting, or the setting is `hold` | Do not send; in a VS Code chat it expires unread in about five minutes. Tell him: the modes must match or `crossSessionInbound` must be `accept` in his `/config` |
| `refusing` | `crossSessionInbound` is `refuse` | Tell him; the message is dropped |
| `closed` | Not on this machine's roster | `SendMessage` refuses. Start a turn with Bash `run_in_background`: the command `af session reach` prints, which denies the connector's order and flow tools by name and takes `--allowedTools` for what the job runs, since a headless turn is denied anything else. Never resume a chat that is running |
| `unknown` | No `claude agents` roster | Say the state is unknown |

A live chat's held or expired message also reaches the sender as a system notice within seconds, naming the recipient. A side chat that needs main runs the same check with `--from <its name>`; when it says `holding` or `closed`, the point goes in its handoff and in its reply to him. `notify_when_idle` wakes the asker only for a target in its own permission class, or one it spawned; otherwise the notice is only logged.

| Verb | Effect |
|---|---|
| `af session register <name> --session <id> [--model <m>] [--brief <path>] [--class <c>]` | Writes the row, creates the folder, copies the brief, and with `--class` writes its `class:` line |
| `af session list [--json]` | Prints the rows with their effective status and class |
| `af session context [name]` | Prints a chat's class, context tokens, window, thresholds and auto-compact window; with no name, a table of every live chat that has a transcript |
| `af session stop`, `af hook stop` | Stamp `last_seen`; ask for a handoff when it is missing, has no `written:` line, or is stale, and say which and how to fix it (never for `main`) |
| `af session end`, `af hook session-end` | Set `ended` |
| `af session compact <name> [--model <m>] [--instructions "<text>"] [--force] [--dry-run] [--when-idle] [--wait-minutes <n>] [--then-file <path>]` | Compacts a chat. A chat with no live process is compacted from outside: `claude -p "/compact <text>" --resume <id>` from the vault root, then checks the transcript gained a `compact_boundary` line and prints the context before, the model and where it came from, and the cost. That route fails closed: it runs only when the session id is absent from the `claude agents --json --all` roster (an entry in any status counts as running) and no `sessions/*.json` record in the Claude config dir names it; no roster refuses it. It checks again after the run and exits 1 with a warning if the chat went live meanwhile. A running chat is compacted through [the panel](#the-claude-panel): the driver sends ` /compact <text>`, and the verb waits up to 10 minutes for a new `compact_boundary` line, failing honestly without one. `--when-idle` returns at once after starting a detached waiter that runs this once the chat and the operator are idle, within `--wait-minutes` (default 60; 20 without `--when-idle`); its outcome is appended to `system/logs/panel-waiters.log`, and this is how a chat compacts itself. `--then-file` sends that file's text to the chat as its next message after the compaction is confirmed, under the same guards. Also refuses a transcript under 50k tokens without `--force`, and on the closed route a haiku model. The closed-route summary model is `--model`, else the last reply's `requestedModel` (the `[1m]` form), else its `message.model`; the standing summary instructions come from `af hook pre-compact`, and `--instructions` adds to them |
| `af session rename <name> [--to <text>] [--wait-minutes <n>]` | Sends ` /rename <text>` through the panel (the text defaults to the registry name), then reads the panel row back and fails when it does not show the new name |
| `af session start <name> --brief-prompt-file <path> --model <m> [--wait-minutes <n>]` | Opens a new chat in the panel with the file as its first message, sets and confirms `--model` first (required; without it the verb refuses and lists the options), and prints the new session id; the chat's own first act is `af session register` |
| `af session panel [--json]` | The chats the panel lists (session id, panel name, idle or running, active), joined with registry names |
| `af session reach <name> [--from <name>] [--confirm "<phrase>" --since <time>] [--json]` | Prints what a message would meet, what to do next, and the `--since` value; `--confirm` exits 0 once the target's Claude read an inbound message holding that phrase at or after that time, 3 until then, and refuses a phrase under 12 characters |
| `af hook session-start`, `af hook notification` | Stamp `last_seen`; session start also revives an ended row |
| `af hook post-tool-use`, `af hook pre-compact`, `af hook post-compact` | The context nudge for a `build` chat; the standing summary instructions at compaction; the nudge re-arms after it |
