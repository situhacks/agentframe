# DOCX vendor record

- Upstream: `https://github.com/anthropics/skills`
- Source ref: `dbd4588f9e1033efb41dad4bef2f7947c8993d44`
- Upstream path: `skills/docx/`
- Snapshot date (UTC): `2026-10-24`
- License: `LICENSE.txt`

## Scope

Upstream source mirror plus a thin AgentFrame overlay at `AGENTS.md`, which owns the validation route, the repack route, and the native-render boundary. Nothing inside `scripts/` is AgentFrame-modified.

## Refresh

Replace this directory with `skills/docx/` from the source ref, so files upstream deleted do not survive, then restore this record and `AGENTS.md`, which is AgentFrame-owned and not part of the upstream tree. Verify `SKILL.md`, `scripts/`, and `LICENSE.txt` are present. Refresh `system/vendor/pptx/` from the same ref: `system/tools/docx_validate.py` validates `.pptx` through this copy of `scripts/office/`, which must stay identical to the pptx copy.

Verify with `python -m pytest system/tests/test_docx_validate_wrapper.py -q -p no:cacheprovider`, then recheck the overlay's containments against the new ref:

- Non-UTF-8 I/O: run `scripts/office/validate.py` directly, without `PYTHONUTF8`, on a `.docx` whose `word/fontTable.xml` carries `ＭＳ 明朝`. A `charmap` finding means the wrapper is still needed.
- The `w:zoom` false positive: validate a `.docx` whose `word/settings.xml` carries `<w:zoom w:val="bestFit"/>`.
- The repack route: `SKILL.md` repacks with `zip`; confirm the `rezip` helper still exists in `scripts/office/helpers/__init__.py`.
