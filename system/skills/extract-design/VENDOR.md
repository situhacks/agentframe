## Extract-Design Skill Vendor Record

- Upstream repository: `https://github.com/Manavarya09/design-extract`
- Upstream version: v12.15.0
- Snapshot date (UTC): `2026-06-10`
- Source location in AgentFrame: `system/skills/extract-design/` (upstream `skills/extract-design/SKILL.md` only — the CLI itself is npm-distributed and runs via `npx designlang`, so no source is vendored)
- License: MIT — see `LICENSE.txt`

### Purpose

Track the upstream source for the thin extraction skill. The actual tool is the `designlang` CLI (Node 20+, Playwright headless Chromium pulled on first run). AgentFrame-specific routing and distillation rules live in `AGENTS.md` next to this file.

### Refresh Procedure

1. Download upstream `skills/extract-design/SKILL.md` and root `LICENSE` (as `LICENSE.txt`) into this folder, replacing the existing copies.
2. Reapply the AgentFrame overlay files (`VENDOR.md`, `AGENTS.md`) and verify both still exist.
3. If upstream output filenames changed, update the distillation table in `AGENTS.md`.
