# AgentFrame bridge (VS Code extension)

A small extension that lets agents manage Claude Code chat tabs in the VS Code window that holds the vault, without any synthetic keyboard or mouse input.

It activates only when the first workspace folder contains `system/af` and `AGENTS.md`. In any other window it does nothing. If the folder is a git worktree under `.claude/worktrees/`, the real vault root above it is used.

## The inbox

Under the vault root, `system/logs/panel-bridge/`:

- `requests/<id>.json` in: `{"action": "...", "args": {...}, "ts": <epoch ms>}`
- `results/<id>.json` out: `{"id", "ok", "action", "result" or "error", "ms", "at"}`, written to a temp file and renamed
- `bridge.log`: one line per request (time, id, action, ok or error). The prompt text is never logged.

The request file is deleted after it is handled. A request older than 2 minutes is answered with an error and not run. A file watcher picks requests up at once; a 2 second poll is the fallback. Requests run one at a time.

Write a request to a temp name and rename it to `<id>.json`, or use the client: `python system/tools/panel_bridge.py <action> [--json '<args>'] [--timeout 20]` (exit 0 ok, 1 error, 2 timeout).

## Actions

There is no generic "run this command" action.

| Action | Args | What it does |
|---|---|---|
| `ping` | none | Bridge version, workspace root, VS Code version, the Claude extension's version |
| `tabs` | none | Every tab: group, index, label, isActive, kind, viewType (webview), `claude: true` for Claude chat tabs |
| `open` | `sessionId?`, `prompt?`, `groupId?` | `claude-vscode.editor.open(sessionId, prompt, undefined, groupId)`. A new chat, or a session with no open tab, gets the prompt prefilled (never sent). A session that already has a tab is only revealed and the prompt is dropped |
| `reveal` | `sessionId` | The same command with only the session id: brings that chat's tab to the front |
| `close-tab` | `label`, `all?` | Closes Claude chat tabs whose label equals `label` exactly; `all: false` closes the first only. Returns how many. Any tab that is not a Claude chat tab is never touched |
| `group-pick` | `steps` | Calls `claude-vscode.addSessionTabToGroup` without awaiting, waits for the pick, runs `quickOpenSelectNext` `steps` times, then `acceptSelectedQuickOpenItem` |

## Build and install

`python system/vscode-bridge/build.py` writes `system/logs/panel-bridge/af-bridge-<version>.vsix` in the main checkout (no npm). Install with `code --install-extension <file.vsix>`. Never reload the window to load it: other chats run in that window.
