---
name: doc-export
description: Render a finished resume/cover-letter markdown head into ATS-safe submission files (browser-printed PDF and/or docx), filed under the deliverable's media/ and recorded in exports[].
---

# doc-export

**The markdown is where the content gets right; this skill only makes it submission-shaped.** Iterate `resume/resume-v{N}.md` / `cover-letter/cover-letter-v{N}.md` until the jd-map verification passes—that is the work. Then export, file, record, and mark ready.

## 1. Pick the format from the board's `ats` value

| Board `ats` | Build | Why (July-2026 parser research) |
|---|---|---|
| workday, taleo, icims, successfactors, unknown | **DOCX** | XML structure parses natively; Workday's PDF extractor scrambles bullets and drops lines |
| greenhouse, lever, ashby, email/direct | **PDF** via Path A | 96%+ parse fidelity on text-layer PDFs; the recruiter sees the native file |
| unsure / both tracks | build both, submit DOCX | asymmetric downside |

## 2. Path A — HTML → PDF

1. Read the head markdown (the version the operator finished iterating).
2. Read [`resources/resume-template.html`](resources/resume-template.html) (or [`resources/cover-letter-template.html`](resources/cover-letter-template.html)).
3. **Fill the `<body>` from the markdown, keeping the `<style>` block byte-for-byte unchanged.** Follow the DOM pattern exactly — single column, `.row` flex pairs for org/dates, the company-umbrella nesting for multiple roles or highlighted projects. Never add columns, tables, icons, images, or skill bars. If the filled HTML overflows the page budget (1 page US; 2 allowed for senior Canadian-enterprise), tell the operator what to cut — never shrink the font.
4. Write the filled HTML next to the exports: `{deliverable}/media/{name}-v{N}.html`.
5. Render: `node system/skills/doc-export/scripts/export_pdf.mjs <abs-path-to-html>` (finds Chrome/Edge, prints headless with real selectable text). Fallback: open the HTML → Ctrl+P → Save as PDF → margins Default → uncheck headers/footers.

## 3. Path B — DOCX

Use the vendored [`docx`](../docx/) skill to build the document, reading its [`AGENTS.md`](../docx/AGENTS.md) first; [`docx-generator-reference.js`](docx-generator-reference.js) in this folder is a runnable layout reference matching the resume template's structure (Calibri, section rules, right-tab dates, round bullets). Write to `{deliverable}/media/{name}-v{N}.docx`.

## 4. File, validate, ready

1. Exports land under the deliverable's `media/` and each path goes into the head file's `exports[]` frontmatter — `af ready` refuses without them.
2. **Plain-text paste test** (mandatory): open the PDF/DOCX, select-all, copy, paste into a plain-text editor. Reading order intact, headings present, dates intact, no garbled glyphs. Consumer "ATS score" sites are lead-gen theater — this test plus the jd-map verification is the gate.
3. `af ready <slug> resume` (and `cover-letter`) — the pack rules re-check verification + parse hazards.

## Boundaries

- Never export from a draft that hasn't passed the jd-map `## Verification`—the readiness gate will refuse it anyway.
- Exported binaries under `workspace/` are personal (gitignored); nothing here publishes anywhere.
- Submission itself is always the human, in a normal browser, on the company career site.
