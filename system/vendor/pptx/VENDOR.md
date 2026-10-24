# PPTX vendor record

- Upstream: `https://github.com/anthropics/skills`
- Source ref: `dbd4588f9e1033efb41dad4bef2f7947c8993d44`
- Upstream path: `skills/pptx/`
- Snapshot date (UTC): `2026-10-24`
- License: `LICENSE.txt`

## Scope

This is an upstream source mirror with no AgentFrame overlay. `library/process/deck-production.md` owns the two Windows containments for this skill: validation through `system/tools/docx_validate.py` and repacking through the vendored `rezip` helper.

## Refresh

Replace this directory with `skills/pptx/` from the source ref, so files upstream deleted do not survive, then restore this record. Verify `SKILL.md`, `scripts/`, and `LICENSE.txt` are present. Refresh `system/vendor/docx/` from the same ref and confirm the two `scripts/office/` trees are identical, because `system/tools/docx_validate.py` validates `.pptx` through the docx copy.

Smoke test: unzip a real deck, run `scripts/add_slide.py` and `scripts/clean.py` on it, repack with the `rezip` command in `deck-production.md`, and validate the result with `python system/tools/docx_validate.py <out.pptx> --original <deck.pptx>`.
