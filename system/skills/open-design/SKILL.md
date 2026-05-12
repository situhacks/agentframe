---
name: open-design
description: "Bundled Open Design runtime. Use after selecting the Open Design path from the image or deck production menu."
---

# Open Design (Bundled)

Vendored source is in [`source/`](source/) and vendoring provenance/cuts are tracked in [`VENDOR.md`](VENDOR.md).

## AgentFrame Staging

Any AgentFrame deliverable that picks the Open Design path is staged by the agent through the OD daemon API. Load [`HANDOFF.md`](HANDOFF.md) for the staging procedure: project creation, design-system staging, prepared first prompt, project URL, and lock-time import. The operator does not create projects manually; they open the URL the agent provides and click Send.

## Launch

From `system/skills/open-design/source`, run:

`corepack pnpm exec tools-dev start`

## Setup Paths

- **Local CLI (default).** OD scans `PATH` for `claude`, `codex`, `gemini`, `cursor-agent`, `qwen`, `qoder`, and `copilot`. If any is installed and authenticated against an existing subscription (Claude Code, Codex, Gemini CLI, etc.), OD picks one automatically. Verify in Settings -> Execution & model.
- **BYOK fallback.** If no CLI is on `PATH` or detection fails, drop a provider key into Settings -> Execution & model. Any Anthropic, OpenAI, Azure, or Google key works; the `GEMINI_API_KEY` already in `.env` is the easiest reuse.

## First-Launch Sanity Check

From `system/skills/open-design/source`, run:

`corepack pnpm exec tools-dev status`

The output should show the daemon URL and web URL. Open the web URL once to confirm the daemon is reachable before staging a campaign project.

## Lifecycle

- First-time setup: `corepack pnpm install` (requires Node 24 per upstream engines).
- Before first install, run `corepack enable` if `corepack pnpm --version` does not respond.
- If Open Design is already running, reuse it instead of starting another instance.
- Check runtime state with `corepack pnpm exec tools-dev status`.
- Stop services with `corepack pnpm exec tools-dev stop`, then start again.
- Exports return to the calling deliverable's `visuals/imports/`; the calling deliverable owns lock criteria.

OD's `source/` tree carries its own `AGENTS.md` and `CLAUDE.md` hierarchy. Keep campaign/system agent work at the AgentFrame repo root and only enter `system/skills/open-design/source` for runtime control commands (`tools-dev` start/status/stop) to avoid loading OD-local agent instructions into broader AgentFrame tasks.
