# `system/skills/`

All AgentFrame skills live here. Some are AgentFrame-owned; others are vendored snapshots. Skill bodies load on demand. Use the state/intent trigger in `Load when`, not a phrase match.

`system/skills/` is canonical. The pilot skills `humanizer`, `deep-research`, and `agentframe-structure` are projected into Claude, Codex, and Cursor native directories by `python system/af.py sync-harnesses --write`; see [`system/harnesses/README.md`](../harnesses/README.md). Never edit a generated projection.

## Skills index

| Skill | Owns | Load when | Provenance |
|---|---|---|---|
| [`agentframe-structure/`](agentframe-structure/) | Structural changes to flows, deliverable types, processes, and ownership boundaries | Adding, renaming, defaulting, retiring, or moving an AgentFrame structural object | Owned by AgentFrame |
| [`deliverable-scaffolding/`](deliverable-scaffolding/) | New deliverable folders, template files, and system registration | A genuinely new reusable deliverable type has been approved | Owned by AgentFrame |
| [`system-improvement/`](system-improvement/) | Earned patches to agent-facing system rules with audit and validation discipline | Applying a template, process, persona, voice, profile, or positioning patch from observed friction | Owned by AgentFrame |
| [`deliverable-harvest/`](deliverable-harvest/) | Deliverable-shape signal extraction and patch-candidate routing | Finished work or feedback should be mined for reusable deliverable-shape improvements | Owned by AgentFrame |
| [`voice-harvest/`](voice-harvest/) | Voice-signal extraction into annotated contrastive pairs | Source material contains a person's voice signal worth adding to the voice system | Owned by AgentFrame |
| [`upstream-sync/`](upstream-sync/) | Commit-by-commit or bulk adoption from upstream AgentFrame | Pulling upstream changes into a customized downstream copy | Owned by AgentFrame |
| [`project-consolidate/`](project-consolidate/) | Project dream pass, governance archive, truth recompilation, and people promotion | `af doctor` reports consolidation due or the operator requests a project consolidation | Owned by AgentFrame |
| [`deep-research/`](deep-research/) | Architect/specialist/synthesis research workflow with grounded citations | A question needs multi-angle investigation beyond a bounded lookup | Owned by AgentFrame; prompts lifted from upstream (see [`deep-research/PROVENANCE.md`](deep-research/PROVENANCE.md)) |
| [`doc-export/`](doc-export/) | ATS-safe PDF/DOCX export and filing for career materials | A finished resume or cover-letter head must become submission files | Adapted from the operator's extern kit (see [`doc-export/VENDOR.md`](doc-export/VENDOR.md)) |
| [`job-scout/`](job-scout/) | Public-ATS job discovery and recency-first triage | The operator requests role discovery against the career search profile | Adapted from the operator's extern kit find-job skill |
| [`manage-lenses/`](manage-lenses/) | Source-backed lens package creation and mutation | The requested outcome explicitly builds, ingests into, refreshes, rebuilds, versions, approves or activates, retires, or exports a lens; not for listing or applying an active one | Owned by AgentFrame |
| [`humanizer/`](humanizer/) | Removal of AI-writing patterns from prose | A public-facing template routes an agent-authored prose region through `humanizer-integration.md` | Vendored (see [`humanizer/VENDOR.md`](humanizer/VENDOR.md)) |
| [`docx/`](docx/) | Generic `.docx` creation and editing; read [`docx/AGENTS.md`](docx/AGENTS.md) first for the validation route and render boundary | The requested output or source is a Word document | Vendored (see [`docx/VENDOR.md`](docx/VENDOR.md)) |
| [`pptx/`](pptx/) | Native `.pptx` inspection and editing | Inspecting or editing PowerPoint files; new deck creation still routes through `deck-production.md` | Vendored (see [`pptx/VENDOR.md`](pptx/VENDOR.md)) |
| [`hyperframes/`](hyperframes/) | HyperFrames video runtime and routed video skills | The active video workflow explicitly selects the HyperFrames path | Vendored (see [`hyperframes/VENDOR.md`](hyperframes/VENDOR.md)) |
| [`browser-harness/`](browser-harness/) | Local browser-control mechanics | An approved browser-fallback recipe requires the local harness | Vendored (see [`browser-harness/VENDOR.md`](browser-harness/VENDOR.md)) |
| [`open-design/`](open-design/) | Advanced image/deck generation runtime | A routed visual workflow explicitly selects Open Design | Vendored (see [`open-design/VENDOR.md`](open-design/VENDOR.md)) |
| [`extract-design/`](extract-design/) | Website design-language extraction through `designlang` | A task needs colors, typography, tokens, themes, or accessibility evidence extracted from a website | Vendored thin overlay (see [`extract-design/VENDOR.md`](extract-design/VENDOR.md)) |
| [`d2-diagrams/`](d2-diagrams/) | Static D2 flowcharts, decision trees, and system explainers | A static relationship or process diagram is the requested artifact | Owned by AgentFrame; uses the vendored D2 CLI ([`system/tools/d2/VENDOR.md`](../tools/d2/VENDOR.md)) |
| [`ppt-master/`](ppt-master/) | Multi-role source-to-SVG-to-PPTX conversion | Converting source documents or pages into a PowerPoint deck through the PPT Master pipeline | Vendored (see [`ppt-master/VENDOR.md`](ppt-master/VENDOR.md)) |

## Operator Notes

For vendored skills, use each skill-local `VENDOR.md` as the canonical refresh procedure. After a vendor refresh, run the targeted smoke test and append a `system_changes` row through `system/audit/writer.py`.
