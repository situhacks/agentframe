# DOCX - AgentFrame Boundary Notes

Vendored document skill (see `VENDOR.md`). The vendor's `SKILL.md` and `scripts/` own DOCX authoring knowledge. This overlay contains only AgentFrame integration boundaries. Read it whenever AgentFrame routes work to this skill.

## Validation route

- **Validate through the wrapper, not the vendor script directly.** Run `python system/tools/docx_validate.py <file.docx>` wherever `SKILL.md` says `python scripts/office/validate.py doc.docx`. The vendored validator reads package XML and prints findings with the platform default encoding, so on a cp1252 Windows console it aborts with `UnicodeEncodeError` on its own message glyphs and fails to decode `word/fontTable.xml` with a `charmap` error. It reports nothing in either case, which is worse than a finding. The wrapper forces UTF-8 and selects an interpreter carrying `defusedxml` and `lxml`; set `AGENTFRAME_DOCX_PYTHON` when the default interpreter lacks them.
- **`w:zoom` without `@w:percent` is a known false positive.** The validator flags `<w:zoom w:val="bestFit"/>` in `word/settings.xml` as a schema error for a missing `@w:percent`, but Word itself emits it and such files open cleanly. Any document descending from a Word-authored donor template carries it, so it fires on documents that already shipped fine. Treat it as clean, or write `w:percent="100"` when a run must report zero findings. Never restructure a working document to satisfy this check.
- **Never hand-edit the mirror.** Contain a validator defect here or fix it upstream. A local divergence inside `scripts/` is silently lost on the next vendor refresh.

## Rendering

- **Render natively; never LibreOffice.** Ignore any `soffice` plus `pdftoppm` recipe in the vendor's docs. LibreOffice substitutes fonts it cannot resolve and shows defects the file does not have. Render through installed Word per [`library/process/native-office-render.md`](../../../library/process/native-office-render.md), which owns the DOCX path too.
- Reading a document's *content* needs no renderer; that comes from the OOXML package.
