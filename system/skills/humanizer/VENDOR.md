## Humanizer Skill — Vendor Record

- Upstream: `https://github.com/blader/humanizer`
- Version: `2.7.0` · contentSha `a2ace14` · vendored `2026-06-01` · MIT
- Files (upstream, unmodified): `SKILL.md`, `README.md`, `AGENTS.md`, `LICENSE`

**Vendored whole, never edited.** AgentFrame additions live in `library/process/humanizer-integration.md` and `library/context/operator/voice/`, never in the skill body. The only AgentFrame change to `SKILL.md` is the vendor header above its frontmatter.

**Voice-calibration hook:** SKILL.md "Voice Calibration (Optional)" accepts a writing sample inline or by file path. The voice system passes a Brandon sample / points at `voice/pairs/` to calibrate rewrites.

### Re-vendor

1. `git clone --depth 1 https://github.com/blader/humanizer.git` (temp).
2. Copy `SKILL.md`, `README.md`, `AGENTS.md`, `LICENSE` in (overwrite).
3. Re-prepend the vendor header to `SKILL.md`; update version/sha/date here.
4. Smoke-test a prose pass.
