## Humanizer Skill Vendor Record

- Upstream source: `https://agentskill.sh/@blader/humanizer`
- Upstream project: `https://github.com/blader/humanizer`
- Snapshot date (UTC): `2026-04-19`
- Source location in AgentFrame: `system/skills/humanizer/`

### Purpose

Track the upstream source and refresh procedure for the vendored humanizer skill.

### Refresh Procedure

1. Fetch the latest `blader/humanizer` skill payload from agentskill.sh (or re-vendor from the upstream repository if the skill feed changes).
2. Remove `system/skills/humanizer/` from this repo.
3. Copy the refreshed skill payload into `system/skills/humanizer/`.
4. Reapply AgentFrame-specific overlays (if any) and verify this file still exists.
5. Verify behavior with a small prose humanization smoke test.
