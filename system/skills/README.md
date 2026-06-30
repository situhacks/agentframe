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
| [`project-consolidate/`](project-consolidate/) | Consolidate, compact, and archive stale project knowledge / governance docs to keep active context files lean. | Owned by AgentFrame |
| [`humanizer/`](humanizer/) | Remove AI-writing patterns from prose. | Vendored (see [`humanizer/VENDOR.md`](humanizer/VENDOR.md)) |
| [`docx/`](docx/) | Generic `.docx` creation/editing capability. | Vendored (see [`docx/VENDOR.md`](docx/VENDOR.md)) |
| [`pptx/`](pptx/) | Generic `.pptx` creation/editing capability. | Vendored (see [`pptx/VENDOR.md`](pptx/VENDOR.md)) |
| [`hyperframes/`](hyperframes/) | HyperFrames composition authoring for HTML-based video production. | Vendored (see [`hyperframes/VENDOR.md`](hyperframes/VENDOR.md)) |
| [`hyperframes-cli/`](hyperframes-cli/) | HyperFrames CLI workflow (`init`, `lint`, `inspect`, `preview`, `render`). | Vendored (see [`hyperframes-cli/VENDOR.md`](hyperframes-cli/VENDOR.md)) |
| [`gsap/`](gsap/) | GSAP animation reference used by HyperFrames compositions. | Vendored (see [`gsap/VENDOR.md`](gsap/VENDOR.md)) |
| [`browser-harness/`](browser-harness/) | Local browser control harness used by AgentFrame browser fallback workflows. | Vendored (see [`browser-harness/VENDOR.md`](browser-harness/VENDOR.md)) |
| [`open-design/`](open-design/) | Bundled Open Design runtime for advanced image/deck generation workflows. | Vendored (see [`open-design/VENDOR.md`](open-design/VENDOR.md)) |
| [`extract-design/`](extract-design/) | Extract a website's full design language (colors, type, tokens, Tailwind/React/shadcn themes, WCAG score) via the `designlang` CLI. | Vendored thin overlay (see [`extract-design/VENDOR.md`](extract-design/VENDOR.md)) |
| [`ppt-master/`](ppt-master/) | Convert source documents (PDF/DOCX/URL/Markdown) into SVG pages and export to PPTX via a multi-role pipeline. | Vendored (see [`ppt-master/VENDOR.md`](ppt-master/VENDOR.md)) |

## Asset References

- HyperFrames launch-video references live at [`hyperframes/references/launch-video/`](hyperframes/references/launch-video/) and are loaded only when needed.

## Operator Notes

For vendored skills, use each skill-local `VENDOR.md` as the canonical refresh procedure (upstream source, cut notes, and re-vendor steps).

After any vendor refresh, run a targeted smoke test for the affected skill(s) and append a `system_changes` row via `system/audit/writer.py`.
