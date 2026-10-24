# `system/vendor/`

Vendored third-party runtimes. Each folder is a snapshot of an upstream plus the few AgentFrame overlay files its `VENDOR.md` lists. A process file routes to each runtime; load one only through that route. AgentFrame-owned skills live in [`.claude/skills/`](../../.claude/skills/README.md).

| Runtime | What it is | Routed by | Refresh |
|---|---|---|---|
| [`ppt-master/`](ppt-master/) | Multi-role source-to-SVG-to-PPTX deck pipeline. Read its [`AGENTS.md`](ppt-master/AGENTS.md) overlay first; its upstream docs live in `system/docs/` | [`deck-production.md`](../../library/process/deck-production.md) | [`VENDOR.md`](ppt-master/VENDOR.md) |
| [`pptx/`](pptx/) | Native `.pptx` inspection, validation, and small edits | [`deck-production.md`](../../library/process/deck-production.md), [`reference-grounded-deck-redesign.md`](../../library/process/reference-grounded-deck-redesign.md) | [`VENDOR.md`](pptx/VENDOR.md) |
| [`docx/`](docx/) | Generic `.docx` creation and editing. Read its [`AGENTS.md`](docx/AGENTS.md) overlay first; validate through `system/tools/docx_validate.py` | the export sections of the marketing `business-brief` and `campaign-brief` templates; the [`doc-export`](../../.claude/skills/doc-export/SKILL.md) skill | [`VENDOR.md`](docx/VENDOR.md) |
| [`hyperframes/`](hyperframes/) | HyperFrames video runtime: Studio, the CLI, the composition engine, and the upstream video skills under `source/skills/` | [`video-production.md`](../../library/process/video-production.md), [`short-form-edit.md`](../../library/process/short-form-edit.md) | [`VENDOR.md`](hyperframes/VENDOR.md) |
| [`diagram-design/`](diagram-design/) | Editorial diagrams across 39 layout grammars as self-contained HTML/SVG, brand-tokenized per design language | [`diagram-production.md`](../../library/process/diagram-production.md) | [`VENDOR.md`](diagram-design/VENDOR.md) |

Vendored binaries and wrappers outside this folder carry their own records: the D2 CLI ([`system/tools/d2/VENDOR.md`](../tools/d2/VENDOR.md)), `agy_call.py` ([`agy_call.VENDOR.md`](../tools/agy_call.VENDOR.md)), and `transcribe.py` ([`transcribe.VENDOR.md`](../tools/transcribe.VENDOR.md)).

## Rules

- Do not edit vendored files to change behaviour. AgentFrame rules live in the overlay files and the routing process file.
- Refresh a runtime by its `VENDOR.md`, then run the smoke test it names.
- PPTX and DOCX rendering goes through [`native-office-render.md`](../../library/process/native-office-render.md); the vendored `soffice` recipes do not run.
- Deck projects and exports never stage inside `ppt-master/`. `system/hooks/ppt_master_guard.py` denies the staging and `af doctor` reports strays.
