# Antigravity CLI connector vendor record

- Tool: `system/tools/agy_call.py` (AgentFrame-owned wrapper; stdlib only)
- Upstream: Antigravity CLI (`agy`) by Google, verified 1.1.27 on 2026-09-08
- Install (Windows): `irm https://antigravity.google/cli/install.ps1 | iex` → `%LOCALAPPDATA%\agy\bin\agy.exe`; macOS/Linux: `curl -fsSL https://antigravity.google/cli/install.sh | bash` → `~/.local/bin/agy`
- Auth: the operator's Google account through the OS keyring; first interactive run signs in. Print mode reused the existing Antigravity IDE sign-in on this machine without prompting. Entitlement: Google AI Pro (the Gemini CLI is no longer served to individual accounts; its own error message says to move to Antigravity).
- Settings: `~/.gemini/antigravity-cli/settings.json` (none needed for this connector)
- Model slugs (from `agy models`): `gemini-3.8-flash-{high,medium,low}`, `gemini-3.7-flash-*`, `gemini-3.6-flash-*`, `gemini-3.1-pro-{high,low}`, plus `claude-sonnet-4-6`, `claude-opus-4-6-thinking`, `gpt-oss-120b-medium`. Effort is the slug suffix; `--effort` alongside a suffixed slug is rejected.

## What was measured (2026-09-08, one 4 s synthetic clip)

| Recipe | Result |
|---|---|
| `@clip.mp4` in the prompt, no permission flags | agent asked for RunCommand, denied, empty response |
| `@clip.mp4` + `--dangerously-skip-permissions` | correct description after searching the filesystem: 2m35s, ~250k tokens |
| absolute path in prose + skip-permissions | correct description: 11 s, ~31k tokens |
| view-only `permissions.allow`, no skip flag | viewed the video **and wrote a file anyway** |
| `--mode plan` + skip flag | viewed the video **and wrote a file anyway** |
| **shipped fence:** read-only copies in a disposable workspace, `--add-dir` that workspace only, no skip flag | correct description, write landed only in the discarded workspace, RunCommand denied: 7 s, ~45k tokens |

Print mode does not gate file writes inside the agent's workspace. Only shell commands are gated. The fence is therefore structural, never a flag.

## Refresh

`agy update`, re-run the fence check (a prompt that asks the agent to describe a staged clip, write a file, and run `dir`; expect a description, `workspace_leftovers` listing the file, `RunCommand` in `denied_actions`, and the source folder untouched), and update the model table from `agy models`.
