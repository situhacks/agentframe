## PPTX Skill Vendor Record

- Upstream repository: `https://github.com/anthropics/skills`
- Snapshot date (UTC): `2026-04-18`
- Source location in AgentFrame: `system/skills/pptx/`
- License: see `system/skills/pptx/LICENSE.txt`

### Purpose

Track the upstream source and refresh procedure for the vendored PPTX skill.

### Refresh Procedure

1. Clone upstream to a temporary directory (depth 1 is acceptable for routine refreshes).
2. Remove `system/skills/pptx/` from this repo.
3. Copy upstream `skills/pptx` into `system/skills/pptx/`.
4. Reapply AgentFrame-specific overlays (for example Open Design routing notes) and verify this file still exists.
5. Remove the temporary clone directory.
