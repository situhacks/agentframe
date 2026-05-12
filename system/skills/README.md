# `system/skills/`

All AgentFrame skills live here. Some are AgentFrame-owned, some are vendored snapshots.

## Skills index

| Skill | Purpose | Provenance |
|---|---|---|
| [`agentframe-structure/`](agentframe-structure/) | Structural changes to flows, deliverable types, and ownership boundaries. | Owned by AgentFrame Marketing |
| [`deliverable-scaffolding/`](deliverable-scaffolding/) | Scaffold new deliverable folders/files using system conventions. | Owned by AgentFrame Marketing |
| [`system-improvement/`](system-improvement/) | Small and medium system-level improvements with verification discipline. | Owned by AgentFrame Marketing |
| [`humanizer/`](humanizer/) | Remove AI-writing patterns from prose. | Vendored (see [`humanizer/VENDOR.md`](humanizer/VENDOR.md)) |
| [`docx/`](docx/) | Generic `.docx` creation/editing capability. | Vendored (see [`docx/VENDOR.md`](docx/VENDOR.md)) |
| [`pptx/`](pptx/) | Generic `.pptx` creation/editing capability. | Vendored (see [`pptx/VENDOR.md`](pptx/VENDOR.md)) |
| [`hyperframes/`](hyperframes/) | HyperFrames composition authoring for HTML-based video production. | Vendored (see [`hyperframes/VENDOR.md`](hyperframes/VENDOR.md)) |
| [`hyperframes-cli/`](hyperframes-cli/) | HyperFrames CLI workflow (`init`, `lint`, `inspect`, `preview`, `render`). | Vendored (see [`hyperframes-cli/VENDOR.md`](hyperframes-cli/VENDOR.md)) |
| [`gsap/`](gsap/) | GSAP animation reference used by HyperFrames compositions. | Vendored (see [`gsap/VENDOR.md`](gsap/VENDOR.md)) |
| [`open-design/`](open-design/) | Bundled Open Design runtime for advanced image/deck generation workflows. | Vendored (see [`open-design/VENDOR.md`](open-design/VENDOR.md)) |

## Export Asset Pack

- [`export-assets/`](export-assets/) holds AgentFrame-specific export config and master templates used alongside `docx/` and `pptx/` skills.
- HyperFrames launch-video references live at [`hyperframes/references/launch-video/`](hyperframes/references/launch-video/) and are loaded only when needed.

## Operator Notes

For vendored skills, use each skill-local `VENDOR.md` as the canonical refresh procedure (upstream source, cut notes, and re-vendor steps).

After any vendor refresh, run a targeted smoke test for the affected skill(s) and append a `system_changes` row via `system/audit/writer.py`.
