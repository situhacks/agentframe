# `system/skills/`

All AgentFrame skills live here. Some are AgentFrame-owned, some are vendored snapshots.

## Skills index

| Skill | Purpose | Provenance |
|---|---|---|
| [`agentframe-structure/`](agentframe-structure/) | Structural changes to flows, deliverable types, and ownership boundaries. | Owned by AgentFrame |
| [`deliverable-scaffolding/`](deliverable-scaffolding/) | Scaffold new deliverable folders/files using system conventions. | Owned by AgentFrame |
| [`system-improvement/`](system-improvement/) | Small and medium system-level improvements with verification discipline. | Owned by AgentFrame |
| [`deliverable-harvest/`](deliverable-harvest/) | Mine deliverable-SHAPE feedback from project source material; routes findings to template-patch candidates, the feedback-log, and backlog recurrence watches. Never patches templates directly. | Owned by AgentFrame |
| [`voice-harvest/`](voice-harvest/) | Extract a person's voice signal from source material into annotated contrastive pairs (ACPs) in the voice system. | Owned by AgentFrame |
| [`upstream-sync/`](upstream-sync/) | Pull updates from the upstream AgentFrame repo into this customized copy — commit-by-commit adoption or squashed bulk migration. | Owned by AgentFrame |
| [`project-consolidate/`](project-consolidate/) | Dream pass on one project: archive stale governance entries, re-synthesize compiled-truth headers from full timelines, and promote earned people context to `library/context/people/`. Operator-invoked; `af doctor` nudges when due. | Owned by AgentFrame |
| [`deep-research/`](deep-research/) | Native deep research on the agent's own tools: architect brief → role-specialized specialist wave → synthesis gate → grounded cited report. Harness-agnostic (parallel subagents or sequential passes); no research API. | Owned by AgentFrame; prompts lifted from upstream (see [`deep-research/PROVENANCE.md`](deep-research/PROVENANCE.md)) |
| [`doc-export/`](doc-export/) | Render a finished resume/cover-letter head into ATS-safe submission files (PDF via headless-browser print, DOCX via the `docx` skill), format keyed to the pipeline board's `ats` column, filed under `media/` + `exports[]`. | Adapted from the operator's extern kit (see [`doc-export/VENDOR.md`](doc-export/VENDOR.md)) |
| [`job-scout/`](job-scout/) | Operator-invoked job discovery: sweep the career search-profile watchlist via public ATS JSON feeds (tiered, login-free), ghost-filter, and write a recency-first triage report feeding `af pipe save`. Never scheduled; never auto-applies. | Adapted from the operator's extern kit find-job skill |
| [`humanizer/`](humanizer/) | Remove AI-writing patterns from prose. | Vendored (see [`humanizer/VENDOR.md`](humanizer/VENDOR.md)) |
| [`docx/`](docx/) | Generic `.docx` creation/editing capability. | Vendored (see [`docx/VENDOR.md`](docx/VENDOR.md)) |
| [`pptx/`](pptx/) | Native `.pptx` inspection/editing capability; deck creation routes through `library/process/deck-production.md`. | Vendored (see [`pptx/VENDOR.md`](pptx/VENDOR.md)) |
| [`hyperframes/`](hyperframes/) | Full upstream HyperFrames repository: Studio, CLI, engine, and routed video skill library. | Vendored (see [`hyperframes/VENDOR.md`](hyperframes/VENDOR.md)) |
| [`browser-harness/`](browser-harness/) | Local browser control harness used by AgentFrame browser fallback workflows. | Vendored (see [`browser-harness/VENDOR.md`](browser-harness/VENDOR.md)) |
| [`open-design/`](open-design/) | Bundled Open Design runtime for advanced image/deck generation workflows. | Vendored (see [`open-design/VENDOR.md`](open-design/VENDOR.md)) |
| [`extract-design/`](extract-design/) | Extract a website's full design language (colors, type, tokens, Tailwind/React/shadcn themes, WCAG score) via the `designlang` CLI. | Vendored thin overlay (see [`extract-design/VENDOR.md`](extract-design/VENDOR.md)) |
| [`d2-diagrams/`](d2-diagrams/) | Render static flowcharts, decision trees, and system explainers as SVG. | Owned by AgentFrame; uses the vendored D2 CLI ([`system/tools/d2/VENDOR.md`](../tools/d2/VENDOR.md)) |
| [`ppt-master/`](ppt-master/) | Convert source documents (PDF/DOCX/URL/Markdown) into SVG pages and export to PPTX via a multi-role pipeline. Read [`ppt-master/AGENTS.md`](ppt-master/AGENTS.md) boundary notes before running. | Vendored (see [`ppt-master/VENDOR.md`](ppt-master/VENDOR.md)) |

## Operator Notes

For vendored skills, use each skill-local `VENDOR.md` as the canonical refresh procedure (upstream source, cut notes, and re-vendor steps).

After any vendor refresh, run a targeted smoke test for the affected skill(s) and append a `system_changes` row via `system/audit/writer.py`.
