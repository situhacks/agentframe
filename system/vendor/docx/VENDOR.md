# DOCX vendor record

- Upstream: `https://github.com/anthropics/skills`
- Source ref: `9d2f1ae187231d8199c64b5b762e1bdf2244733d`
- Upstream path: `skills/docx/`
- Snapshot date (UTC): `2026-07-11`
- License: `LICENSE.txt`

## Scope

Upstream source mirror plus a thin AgentFrame overlay at `AGENTS.md`, which owns the validation route and the native-render boundary. Nothing inside `scripts/` is AgentFrame-modified.

## Refresh

Copy `skills/docx/` from the source ref and restore this record. Verify `SKILL.md`, `scripts/`, and `LICENSE.txt` are present, and keep `AGENTS.md`, which is AgentFrame-owned and not part of the upstream tree. Recheck the overlay's two contained defects against the new ref: the validator's non-UTF-8 I/O and its `w:zoom` false positive.
