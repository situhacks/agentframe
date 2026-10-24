## PPT Master Skill Vendor Record

- Upstream repository: `https://github.com/hugohe3/ppt-master`
- Upstream version: release `v6.7.3` @ `622bf0148bca4f697b0184322c7e2aa2a04520a2` (pin the release commit hash, not the tag or the moving branch)
- Release date (UTC): `2026-10-22`
- Snapshot date (UTC): `2026-10-23`
- Source locations in AgentFrame: `system/vendor/ppt-master/` (upstream `skills/ppt-master/`) and `system/docs/` (all authored upstream `docs/**/*.md`, preserving the upstream relative layout)
- Excluded from the docs snapshot: binary screenshots, sponsor images, and other non-Markdown presentation assets; they carry no agent-facing operating knowledge
- License: MIT - see `LICENSE.txt`

### Purpose

Track the upstream source and refresh procedure for the vendored PPT Master deck-generation skill. Vendor files own deck workflow and design knowledge. AgentFrame-specific integration boundaries live only in `AGENTS.md` next to this file.

### Refresh Procedure

1. Clone upstream to a temporary directory and check out the exact commit being adopted.
2. Preserve the AgentFrame overlay files (`VENDOR.md`, `AGENTS.md`), then remove `system/vendor/ppt-master/` and `system/docs/`.
3. Copy upstream `skills/ppt-master/` into `system/vendor/ppt-master/` in full. Keep the skill's `LICENSE`, `SPONSORS.md`, `SPONSORS_CN.md`, and `SKILL.md` frontmatter byte-for-byte: every vendor entry point runs `scripts/attribution_guard.py` and fails closed when one is missing or altered.
4. Copy every upstream `docs/**/*.md` into `system/docs/`, preserving relative paths. These files are vendor-owned dependencies, not AgentFrame summaries.
5. Copy the upstream root `LICENSE` to `system/vendor/ppt-master/LICENSE.txt`, then restore the two overlay files.
6. Remove the temporary clone directory.
7. Run `python system/vendor/ppt-master/scripts/attribution_guard.py` (exit 0), `python -m unittest system.tests.test_ppt_master_vendor`, `cargo test --test hooks_ppt_guard` in `system/af/`, and `af doctor`. If upstream changed an integration contract, update the AgentFrame boundary or guard; never restate or patch vendor-owned workflow guidance.
8. Run `git status --short --ignored system/vendor/ppt-master system/docs`. Any `!!` line other than `__pycache__/` is a vendor file a `.gitignore` rule hides; the repo-wide `docs/` rule catches `scripts/docs/`. Re-include it in `.gitignore` before committing.
