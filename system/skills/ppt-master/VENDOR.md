## PPT Master Skill Vendor Record

- Upstream repository: `https://github.com/hugohe3/ppt-master`
- Upstream version: `main` @ `b520a0a96f931d4a1ba173bfe769a39de4d46c6d` (pin the commit hash, not the moving branch)
- Snapshot date (UTC): `2026-07-10`
- Source locations in AgentFrame: `system/skills/ppt-master/` (upstream `skills/ppt-master/`) and `system/docs/` (all authored upstream `docs/**/*.md`, preserving the upstream relative layout)
- Excluded from the docs snapshot: binary screenshots, sponsor images, and other non-Markdown presentation assets; they carry no agent-facing operating knowledge
- Excluded from the skill snapshot: `references/ai-image-comparison/` (~43 MB of model-comparison sample PNGs; the pipeline does not consume it)
- License: MIT - see `LICENSE.txt`

### Purpose

Track the upstream source and refresh procedure for the vendored PPT Master deck-generation skill. Vendor files own deck workflow and design knowledge. AgentFrame-specific integration boundaries live only in `AGENTS.md` next to this file.

### Refresh Procedure

1. Clone upstream to a temporary directory and check out the exact commit being adopted.
2. Preserve the AgentFrame overlay files (`VENDOR.md`, `AGENTS.md`), then remove `system/skills/ppt-master/` and `system/docs/`.
3. Copy upstream `skills/ppt-master/` into `system/skills/ppt-master/`, excluding `references/ai-image-comparison/`.
4. Copy every upstream `docs/**/*.md` into `system/docs/`, preserving relative paths. These files are vendor-owned dependencies, not AgentFrame summaries.
5. Copy the upstream root `LICENSE` to `system/skills/ppt-master/LICENSE.txt`, then restore the two overlay files.
6. Remove the temporary clone directory.
7. Run `python -m unittest system.tests.test_ppt_master_guard system.tests.test_ppt_master_vendor` and `python system/af.py doctor`. If upstream changed an integration contract, update the AgentFrame boundary or guard; never restate or patch vendor-owned workflow guidance.
