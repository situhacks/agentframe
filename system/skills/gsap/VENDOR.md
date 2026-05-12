## GSAP Skill Vendor Record

- Upstream repository: `https://github.com/heygen-com/hyperframes`
- Snapshot date (UTC): `2026-05-10`
- Source location in AgentFrame: `system/skills/gsap/`

### Purpose

Track the upstream source and refresh procedure for the vendored GSAP reference skill used by HyperFrames workflows.

### Refresh Procedure

1. Clone upstream to a temporary directory at the target commit.
2. Remove `system/skills/gsap/` from this repo.
3. Copy the curated AgentFrame GSAP skill files into `system/skills/gsap/`.
4. Reapply AgentFrame-specific overlays (if any) and verify this file still exists.
5. Remove the temporary clone directory.
