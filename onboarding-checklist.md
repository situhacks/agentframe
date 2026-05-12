# AgentFrame First-Run Checklist

Use this checklist only on a fresh clone. Builder mode owns this file and walks the operator through it step-by-step. Mark each item complete as you finish it. When every item is checked off, delete this file so future Builder sessions stay clean.

Builder guidance for each step lives inline so the agent can run the step without re-deriving context.

- [ ] **Import operator context into `library/context/operator/`.** AgentFrame needs operator positioning, voice, and profile information to write campaigns that sound like the operator. Ask the operator if they already maintain a memory file for a chat-bot like Claude or ChatGPT (those usually carry positioning, audience, tone preferences, and prior context). If yes, offer to read it directly or to write a short prompt the operator can paste into that chat-bot to dump the relevant context back, then save it to `library/context/operator/` in the canonical files (`positioning.md`, `voice.md`, `profile.md`). If no, ask the smallest set of questions needed to seed those files. Mark this item done once the canonical operator-context files exist with non-empty content.

- [ ] **Set recommended `.env` keys.** Open `.env` (copy `.env.example` first if it exists) and fill in:
  - `GEMINI_API_KEY` - get from [Google AI Studio](https://aistudio.google.com); free tier is generous enough for solo operators.
  - `COMPOSIO_API_KEY` and `COMPOSIO_MCP_URL` - sign up at [composio.dev](https://composio.dev/), enable the connectors the operator plans to use (LinkedIn, Gmail, Calendar, X, etc.), wire the MCP into their coding agent (Cursor, Codex, Claude Code), and copy the keys/URL Composio gives them.
  Builder can sanity-check key presence with a small request through the relevant skill. Mark this item done once the keys load without error.

- [ ] **Optional: install Open Design runtime dependencies.** Open Design source is already bundled at `system/skills/open-design/source/`. What's optional is installing runtime deps (`node_modules`, Electron + native modules, ~1-2 GB) so `tools-dev` can run locally. If the operator wants it, Builder can run the install: confirm Node 24 with `node -v`, run `corepack enable`, then `cd system/skills/open-design/source` and `corepack pnpm install`. Retry once on transient `ERR_PNPM_EPERM` (Windows antivirus). After install, ask whether the operator has a code-agent CLI on `PATH` (Claude Code, Codex, Gemini CLI, etc.). If yes, Open Design detects it automatically and their existing Claude Code or Codex subscription powers OD with no extra config. If no, a short install with their existing subscription gets them to the same place; OD picks up the CLI on next launch. Mark this item done once `corepack pnpm exec tools-dev status` runs cleanly and OD shows a detected CLI in Settings -> Execution & model, or skip if the operator opts out.

- [ ] **Self-clean and swap to CMO.** Delete this file, then swap to CMO with `Copy-Item AGENTS.cmo.md AGENTS.md -Force` and log a `mode_swap` row via `system/audit/writer.py`. CMO will take it from here for the operator's first campaign.

## Gate

Builder must complete this checklist before swapping to CMO or starting the first campaign. If the operator asks to start a campaign immediately, finish onboarding first, then swap and start.
