## PPT Master Skill Vendor Record

- Upstream repository: `https://github.com/hugohe3/ppt-master`
- Upstream version: `main` @ `b0beba5b659c664bdbf0c07227fbdee313698dd7` (24 commits ahead of the last tag, `v3.1.0`; upstream tags lag `main` — pin the commit hash, not the tag)
- Snapshot date (UTC): `2026-07-09`
- Source location in AgentFrame: `system/skills/ppt-master/` (upstream `skills/ppt-master/` only — the repo's `projects/`, `examples/`, and docs workspace are not vendored)
- Excluded from the snapshot: `references/ai-image-comparison/` (~43 MB of model-comparison sample PNGs; two "see also" pointers in `references/strategist.md` reference it, nothing in the pipeline consumes it)
- License: MIT — see `LICENSE.txt`

### Purpose

Track the upstream source and refresh procedure for the vendored PPT Master deck-generation skill. AgentFrame-specific boundary rules live in `AGENTS.md` next to this file.

### Refresh Procedure

1. Clone upstream to a temporary directory (depth 1 is acceptable for routine refreshes).
2. Remove `system/skills/ppt-master/` from this repo.
3. Copy upstream `skills/ppt-master/` into `system/skills/ppt-master/`, excluding `references/ai-image-comparison/`.
4. Copy the upstream root `LICENSE` to `LICENSE.txt`.
5. Reapply the AgentFrame overlay files (`VENDOR.md`, `AGENTS.md`) and verify both still exist.
6. Remove the temporary clone directory.
7. Run `python -m unittest system.tests.test_ppt_master_guard` — the AgentFrame guard hooks (`system/hooks/ppt_master_guard.py`) match upstream's CLI command shapes (`project_manager.py init`, `svg_to_pptx.py <project_path>`). If upstream renamed or moved those entry points, update the guard hooks and their tests; never patch vendored files.
