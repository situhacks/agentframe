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

There is no generic "run this command" action. This is the source, version 0.2.0; see below for what the installed 0.1.0 does differently.

| Action | Args | What it does |
|---|---|---|
| `ping` | none | Bridge version, workspace root, VS Code version, the Claude extension's version |
| `tabs` | none | Every tab: group, index, label, isActive, kind, viewType (webview), `claude: true` for Claude chat tabs |
| `open` | `sessionId?`, `prompt?`, `groupId?` | `claude-vscode.editor.open(sessionId, prompt, undefined, groupId)`. A new chat, or a session with no open tab, gets the prompt prefilled (never sent). A session that already has a tab is only revealed and the prompt is dropped |
| `reveal` | `sessionId` | The same command with only the session id: brings that chat's tab to the front |
| `close-tab` | `label`, `all?`, `requireActive?` | Closes the first Claude chat tab whose label equals `label` exactly; only `all: true` closes every match. The label is required. Unless `requireActive: false`, it checks again, right before the close, that the tab is the active tab of its group, and closes nothing otherwise (the inbox is polled every 2 seconds, so the driver's own check can be that old). Returns how many matched and how many closed. Any tab that is not a Claude chat tab is never touched |

## Limits seen in the live proof

- A bridge action works while the VS Code window is minimized or behind another window, but the Chromium accessibility tree (what UI Automation reads) goes stale while the window is minimized. A caller that reads the screen after an action must first make sure the window is not minimized.
- `open` with a prompt only prefills. A chat that already has a tab is revealed and the prompt is dropped with a toast, so close the tab first (`close-tab`), wait for the chat's session record to disappear, then `open`.
- The bridge never presses Send. The caller reads the input back and invokes `Send message` itself.

## Build and install

`python system/vscode-bridge/build.py` writes `system/logs/panel-bridge/af-bridge-<version>.vsix` in the main checkout (no npm; `--out <folder>` writes elsewhere). Install it once with `code --install-extension system/logs/panel-bridge/af-bridge-<version>.vsix`. A first install is picked up by the running window without a reload, and the extension answers `ping` within a few seconds.

An update of an installed bridge needs an extension host restart (the host keeps the old copy running until then), and a restart ends every running chat in the window. So update it only at a moment when no chat is running, never to fix something in passing, and never reload the window to load it.

## Version 0.1.0 (installed) and 0.2.0 (this source)

The installed bridge is 0.1.0. It differs from this source in two ways, and the panel driver (`system/tools/claude_panel.ps1`) is built against what 0.1.0 does:

- 0.1.0 has a sixth action, `group-pick` (`steps`): it calls `claude-vscode.addSessionTabToGroup` without awaiting, presses `quickOpenSelectNext` `steps` times, then `acceptSelectedQuickOpenItem`. If the group command fails to open its pick, the accept runs whatever quick-open list is showing at that moment, which can be the command palette, so a request can end up running an arbitrary command. Who can reach it: any process that can write a file into `system/logs/panel-bridge/requests/`, which is a process already running as the operator and already able to run any code as him; nothing outside his account can. The driver never sends `group-pick` (a static test checks it), and 0.2.0 removes it.
- 0.1.0's `close-tab` closes every match unless `all: false` is passed, and does not check the tab is still active. The driver always passes `all: false` and `requireActive: true` (0.1.0 ignores it); 0.2.0 closes one tab unless `all: true` is asked for, and only the active one.

Install 0.2.0 at the next moment when no chat is running (the extension host restart ends every chat in the window): `python system/vscode-bridge/build.py`, then `code --install-extension system/logs/panel-bridge/af-bridge-0.2.0.vsix`, then restart the extension host. The driver works the same with either version.
