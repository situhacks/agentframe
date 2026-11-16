# Antigravity CLI connector vendor record

- Tool: `system/tools/agy_call.py` (AgentFrame-owned wrapper; stdlib only)
- Upstream: Antigravity CLI (`agy`) by Google; the CLI was 1.5.9 on 2026-11-15 (the `--web` probes; 1.5.7 on 2026-11-14; last fence check on 1.4.7, 2026-10-23; first verified 1.1.27 on 2026-09-08). The CLI updates itself in the background, so the installed version moves without a refresh; `agy update` only confirms it.
- Install (Windows): `irm https://antigravity.google/cli/install.ps1 | iex` → `%LOCALAPPDATA%\agy\bin\agy.exe`; macOS/Linux: `curl -fsSL https://antigravity.google/cli/install.sh | bash` → `~/.local/bin/agy`
- Auth: the operator's Google account through the OS keyring; first interactive run signs in. Print mode reused the existing Antigravity IDE sign-in on this machine without prompting. Entitlement: Google AI Pro (the Gemini CLI is no longer served to individual accounts; its own error message says to move to Antigravity).
- Settings: `~/.gemini/antigravity-cli/settings.json` (none needed for this connector)
- Model slugs (from `agy models`: the first two on 2026-11-14, the rest on 2026-10-23): `gemini-4-hydrogen-{low,medium,high,xhigh}`, `gemini-pro-orchestrator-g1-pro`, beside the older `gemini-4-argon-{high,medium,low}`, `gemini-3.8-flash-{high,medium,low}`, `claude-opus-5-5-{high,medium,low}`, `claude-sonnet-5-5-{high,medium,low}`, `gpt-oss-120b-medium`. Effort is the slug suffix; `--effort` alongside a suffixed slug is rejected (re-checked on 1.4.7).
- Measured 2026-11-14 on 1.5.7: one test call to `gemini-4-hydrogen-medium` through `agy_call.py` returned in 5 s for about 11,000 tokens. Gemini 4 Hydrogen is used for second drafts, second-opinion reviews and long reading in research; see `library/schema/models.md`, "Other models". Any `claude-*` slug in this list must be one of the ids in `system/models.json`.

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

Re-measured on 1.4.7 (2026-10-23, same clip shape, `gemini-3.8-flash-medium`):

| Recipe | Result |
|---|---|
| shipped fence, prompt asks for a description, a file write, and `dir` | write landed only in the discarded workspace, RunCommand denied, source folder untouched; **`response` empty**: a denied command now ends the turn with `no output produced` in `stderr_tail`: 10 s, ~24k tokens |
| shipped fence, describe-only prompt | correct description, nothing denied, no leftovers: 9 s, ~16k tokens |

## Web search (`--web`), measured 2026-11-15 on 1.5.9, `gemini-4-hydrogen-low` unless noted

Run Hydrogen through `agy_call.py`, never as a bare `agy -p` in the repo, because print mode does not gate file writes.

Built-in tools in the `init` event of `--output-format stream-json` (the plain `json` envelope never names tools on a success): `search_web` and `read_url_content` are listed beside `run_command`, `write_to_file`, the `browser_*` tools and `call_mcp_tool`. The model reports `read_url_content` as not available in print mode, so there is no page fetch and no video-from-URL: only `search_web` ran. Its sources come back as `vertexaisearch.cloud.google.com/grounding-api-redirect/...` links labelled with a site name, not as page URLs; a caller opens or searches for each source itself.

| Try (all in the fenced temp workspace) | Result | Tokens, time |
|---|---|---|
| no preamble, search-shaped question ("what changed in the latest stable Rust release, with URLs") | `search_web` ran, answer correct, nothing denied | ~11k, 12 s |
| no preamble, fetch-shaped task ("fetch the full text of the Rust blog index") | `run_command` (`Invoke-WebRequest`) attempted, denied, `response` empty. This is the 2026-11-15 failure | ~11k, 10 s |
| preamble naming only `search_web`, same fetch-shaped task | four `search_web` calls, no shell, a correct answer saying no page could be fetched | ~11k per turn, 22 s |
| `--mode accept-edits`, preamble, search question | worked; same as default mode (`init` still showed `permission_mode: request-review`) | 13.7k, 11 s |
| default mode, preamble, search question | worked | 13.7k, 13 s |
| `--mode auto` + `--settings '{"permissions":{"allow":["search_web"]}}'`, no preamble, fetch-shaped task | `run_command` ran as a step in state DONE (not ERROR) while `denied_actions` still listed `Bash` and the response was empty. Unclear whether the command executed; the allow-list did not stop the attempt | ~11k, 8 s |
| public YouTube URL, "describe the first 20 seconds" | "I cannot view video from a URL." | ~11k, 7 s |
| `--settings '{"permissions":{"deny":["search_web"]}}'`, default mode, search question | `search_web` still ran and answered. A permissions deny list naming tool names has no effect in print mode, so none is applied | ~13k, 11 s |
| deny list naming `read_url_content`, `open_browser_url`, `read_browser_page`, `call_mcp_tool`, search question | `search_web` ran and answered; nothing to observe on the named tools | ~13k, 10 s |
| "call `open_browser_url`, then `read_browser_page`", with and without that deny list | both times the model said those tools are not available; no tool step ran | ~11k, 21 s and 11 s |
| shipped `--web` through `agy_call.py`, `gemini-4-hydrogen-high`, the fetch-shaped task | no shell, a correct list of the five newest posts with site, title, date | 22.9k, 38 s |

Conclusion, from one run each of the shapes above: web search needs no mode and no settings. The only thing blocked is shell commands (denied, the turn ends). Everything else is steered, not enforced: a prompt preamble names `search_web` as the only web tool and forbids shell, MCP, browser and file writes, and without it a fetch-shaped task goes to `run_command`. `read_url_content`, the `browser_*` tools and `call_mcp_tool` are in the tool list but the model called them unavailable every time, and a `--settings` permissions deny list had no effect on tool names (it did not stop `search_web`), so nothing here blocks a network tool if the model did use one. `--web` therefore sets exactly the preamble, never passes `--dangerously-skip-permissions`, and refuses `--allow-commands`. Never use `--mode auto` for this. Native video and page reading need the file or media saved first and passed with `--file`. The bare `agy -p ... --mode accept-edits` call that also got web results ran unfenced in the real repo; its success came from the prompt, and the missing fence is what this wrapper exists to supply.

## Refresh

`agy update`, then run the fence check as three calls (the first two on a staged clip): (1) a prompt that asks the agent to describe the clip, write a file, and run `dir`; expect `workspace_leftovers` listing the file, `RunCommand` in `denied_actions`, and the source folder untouched (an empty `response` is the 1.4.7 behaviour); (2) a describe-only prompt; expect a correct description; (3) `--web` on a fetch-shaped task such as "fetch the full text of a blog index"; expect a non-empty response and no `denied_actions` (the json envelope does not name tools; use `--output-format stream-json` to see them). Then update the model table from `agy models` and run `python -m pytest system/tests/test_tools_media.py -q -p no:cacheprovider`.
