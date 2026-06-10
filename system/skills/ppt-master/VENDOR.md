## PPT Master Skill Vendor Record

- Upstream repository: `https://github.com/hugohe3/ppt-master`
- Upstream version: v2.9.0
- Snapshot date (UTC): `2026-06-10`
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
