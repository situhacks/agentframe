# doc-export — provenance

Adapted 2026-07-10 from the operator's `extern-resume-and-job-search-agent` kit (`skills/doc-export/` + `scripts/export_pdf.mjs`), built on the operator's real resume/cover-letter formatting.

Changes from the extern source:

- **Format rule updated to the July-2026 ATS research**: extern defaulted to PDF everywhere except Taleo; empirical parser data shows Workday/iCIMS/SuccessFactors also need DOCX. The decision now keys off the pipeline board's `ats` column.
- **Resume template restructured to the operator's format**: Work Experience first (company umbrella + highlighted-project nesting), Applied AI Projects, short Education, Skills & Certifications; no Professional Summary by default. Font switched Georgia → Calibri (99-100% parse completeness).
- **Paths rewired** to `workspace/pipeline/applications/{slug}/{deliverable}/media/` + `exports[]` frontmatter (AgentFrame exports gate), replacing extern's flat application folders.
- DOCX path routes to AgentFrame's existing vendored `docx/` skill instead of a bundled copy; `docx-generator-reference.js` kept as the layout reference.
- `scripts/export_pdf.mjs` vendored verbatim.

Refresh: none expected — the extern kit is a frozen source. If its templates improve, re-diff by hand.
