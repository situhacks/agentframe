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
- `status`: `live` (a hook touched the row within two hours), `idle` (older, not ended), or `ended <ts> <reason>` with reason `clear`, `resume`, `logout`, `exit`, or `other`.
- Timestamps are local ISO minutes.

Each name has a folder, `system/sessions/<name>/`, holding an optional `brief.md` ([`handoff.md`](handoff.md) has both shapes), the chat's `handoff.md`, and for `system/*` and `worker/*` an optional `report.md`.

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
| `af session register <name> --session <id> [--model <m>] [--brief <path>]` | Writes the row, creates the folder, copies the brief |
| `af session list [--json]` | Prints the rows with their effective status |
| `af session stop`, `af hook stop` | Stamp `last_seen`; ask for a handoff when it is stale (never for `main`) |
| `af session end`, `af hook session-end` | Set `ended` |
| `af session reach <name> [--from <name>] [--confirm "<phrase>" --since <time>] [--json]` | Prints what a message would meet, what to do next, and the `--since` value; `--confirm` exits 0 once the target's Claude read an inbound message holding that phrase at or after that time, 3 until then, and refuses a phrase under 12 characters |
| `af hook session-start`, `af hook notification` | Stamp `last_seen`; session start also revives an ended row |
