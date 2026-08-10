# Native Office Render

## Purpose

Owns the single sanctioned way to turn a PPTX/DOCX into pixels or PDF: the installed Microsoft Office desktop application driven over COM by [`system/tools/office_render.ps1`](../../system/tools/office_render.ps1).

LibreOffice is not a fallback and must not be invoked. It substitutes fonts it cannot resolve — on a real client deck Calibri rendered as Cooper Black, overflowing and clipping every headline — so an agent reads defects the file does not have and "fixes" a deck that was already correct.

PowerPoint/Word is therefore a requirement of any AgentFrame instance producing deck or document deliverables. Where Office is absent, the move is an HTML deck, not a degraded render.

## When To Load

Load when a PPTX or DOCX must be seen rather than parsed:

- Rendered visual QA of an exported deck ([`deck-production.md`](deck-production.md)).
- Origin renders and candidate comparison for a preservation redesign ([`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md)).
- Any thumbnail, slide image, or PDF needed from an Office file, including template analysis.

Do not load for reading a deck's *content* — text and shape facts come from the OOXML package (`system/tools/reference_deck.py`, the `pptx` skill), which needs no renderer.

## Procedure

### 1. Confirm the renderer is available

```powershell
powershell -File system/tools/office_render.ps1 probe
```

Reports per-app availability and version as JSON. If an app reports `available: false`, stop and surface its message; do not seek another converter.

### 2. Render

Per-slide PNG (PowerPoint only; the visual-QA and origin-render form):

```powershell
powershell -File system/tools/office_render.ps1 png `
  -Source "<deck.pptx>" -OutputDir "<render-dir>" -Prefix slide
```

PDF (PPTX via PowerPoint, DOCX via Word; the preview/interchange form):

```powershell
powershell -File system/tools/office_render.ps1 pdf `
  -Source "<file>" -Output "<file.pdf>" [-Force]
```

Both print produced paths to stdout and exit non-zero with an actionable message on failure. `-Width`/`-Height` set PNG pixel size (default 2560×1440). `-Force` is required to overwrite an existing output.

Hidden slides are included, so page N and `<prefix>_NN` always mean slide N.

### 3. Read the render

Inspect the produced PNGs or PDF directly. Treat it as the deck's true appearance: it carries the file's real fonts, shapes, and text flow.

## Verification Or Logging

- A render is valid evidence only when `probe` showed the app available and the command exited zero.
- For fidelity-critical work, confirm the PDF embeds the deck's own fonts (no substitution) before trusting it as an origin reference.
- Keep origin renders and comparison sheets with the deck project, per the redesign process.
- The preview server's Office viewer uses the same tool through `system/server/lib/surface/convert.py`, which caches per path/mtime/size/converter-version; no separate render step is needed for a browser preview.

## Boundaries

- Does not own deck authoring, route selection, or preservation contracts — see [`deck-production.md`](deck-production.md) and [`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md).
- Does not own native slide replacement, which is `system/tools/reference_deck_com.ps1`.
- Does not own SVG-stage preview inside PPT Master; that stage renders its own SVG through its browser preview and never needs this tool.
- Does not extract text or shape facts; the OOXML package owns those.
- Vendored skill recipes that shell `soffice` + `pdftoppm` (`system/skills/pptx`, `system/skills/docx`) are superseded by this process. Do not run them, and do not edit those vendored mirrors to remove them.
