## Browser Harness Vendor Record

- Upstream repository: `https://github.com/browser-use/browser-harness`
- Vendored commit: `6d20866664ea3d9691b27bbf64f42ae097437dc3`
- Snapshot date (UTC): `2026-05-27`
- Prior vendored commit: `560ce7a6f508ba2eca915e6863bd075c249fda71` (2026-05-12)
- Source location in AgentFrame: `system/skills/browser-harness/`
- License: MIT (preserved at `system/skills/browser-harness/LICENSE`)

### Purpose

Track exactly what was vendored and what was excluded so future refreshes can repeat the same cut with minimal decision churn.

### Applied Exclusions

The following clone/build/runtime surfaces were excluded from the vendored copy:

- `.git/`
- `.github/`
- `__pycache__/`
- `*.pyc`
- `browser_harness.egg-info/`

### AgentFrame Sanitization

AgentFrame applies a small public-share cleanup on top of the upstream snapshot:

- Removed cloud-provider marketing/setup language from the vendored README.
- Reworded incidental references to retired browser automation approaches in docs and domain examples.
- Added AgentFrame boundary notes that route local work through the controlled Edge Work Browser in `system/browser/`.
- Kept source, license, install docs, harness skill instructions, and reusable domain-skill examples otherwise intact.

### Refresh Procedure

1. Clone upstream to a temporary directory at the target commit.
2. Remove `system/skills/browser-harness/`.
3. Copy the upstream snapshot into `system/skills/browser-harness/`.
4. Re-apply the exclusion list above.
5. Re-apply the AgentFrame sanitization notes above.
6. Restore this `VENDOR.md` with the new commit hash and snapshot date.
7. Verify `LICENSE` remains present in the vendored subtree.

### Notable upstream delta (560ce7a → 6d20866)

- **Breaking:** `-c '...'` removed; pass Python via stdin/heredoc only (`browser-harness <<'PY'` on bash; PowerShell: `"print(page_info())" | browser-harness`).
- `SKILL.md` documents heredoc as the preferred multi-line form.
- Domain-skill preload rule when `BH_DOMAIN_SKILLS=1`.
- Additional domain skills and interaction-skills from upstream (QBO report export, etc.).
- Windows stdout UTF-8 fix in `run.py` (upstream issue #124).
