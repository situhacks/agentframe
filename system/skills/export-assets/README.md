# `system/skills/export-assets/` — Export Assets

This folder is the **agent-first export capability** for AgentFrame Marketing. The agent reads from here when the user asks to export a deliverable to `.docx` or `.pptx`. There are no Python scripts in this folder. All export logic happens in chat: the agent loads a vendored skill, reads the master template + brand config, writes inline `python-docx` / `python-pptx` code, and saves the result.

## Folder Layout

```
system/skills/export-assets/
  templates/              Master design files
    business-brief.docx
    business-brief.pptx
    campaign-brief.docx
    campaign-brief.pptx
    README.md
  config.yaml             Brand palette, fonts, author, versioning, layout catalog
  README.md               This file
```

## Strict Knowledge Separation

This is the contract. Don't collapse layers.

| Concern | Where it lives | When loaded |
|---|---|---|
| How to use `python-docx` / `python-pptx` (generic capability) | `system/skills/{docx,pptx}/SKILL.md` | On demand, when exporting |
| Master design files (the design system) | `templates/` | Read by agent during export |
| Brand constraints (palette, fonts, author, versioning) | `config.yaml` | Read by agent during export |
| PPT-MD format spec + example (the .pptx pre-render contract) | `system/skills/pptx/{pptx-md.md,examples/business-brief.pptx-md}` | Read by agent during .pptx export |
| Lock-event trigger (when to ask about exports) | `AGENTS.md` (workspace root) | Always loaded |
| Per-deliverable export targets (which formats supported) | `library/deliverables/{type}/template-vF.md` "Lock + export" section | Loaded when working on that deliverable |
| Campaign state (`status: locked` etc.) | `workspace/campaigns/{slug}/campaign.md` frontmatter + `phase-2-strategy/{type}/draft.md` frontmatter | Loaded on state checks + work sessions |
| Lock event audit | `workspace/campaigns/{slug}/activity.md` (per campaign — campaign-scoped per Loop 4 redistribution 2026-04-23; `lock_event` + `export_generated` live here only, no central-log dual-write) | Append-only |

**No wrapper skill exists for marketing-OS-specific intelligence.** Generic capability lives in `system/skills/`; AgentFrame-specific rules (when to fire, what to populate, where outputs land, how to version) live in `AGENTS.md` + deliverable templates + `campaign.md`.

## Export Flow (canonical)

1. **Trigger fires** (per `AGENTS.md` lock-event behavior): brief frontmatter `status: locked` OR phrase trigger.
2. **Agent verifies** lock criteria pass (revisions applied, success criteria binary, reviewer feedback applied or waived).
3. **Agent updates** `draft.md` frontmatter `status: locked`.
4. **Agent asks** user: ".docx, .pptx, both, or neither?"
5. **For `.docx`**:
   - Load `system/skills/docx/SKILL.md`
   - Read `system/skills/export-assets/templates/{type}.docx` + `system/skills/export-assets/config.yaml`
   - Write inline `python-docx` code to populate the master with content from `draft.md`
   - Save to `workspace/campaigns/{slug}/phase-2-strategy/{type}/exports/{type}-v{N}.docx`
6. **For `.pptx`**:
   - Load `system/skills/pptx/SKILL.md` + `system/skills/pptx/pptx-md.md`
   - Draft PPT-MD intermediate in chat (paste, don't disk-write yet)
   - Iterate with user until approved
   - Save final PPT-MD alongside the .pptx for source-of-truth
   - Render `.pptx` using vendored skill + master template
7. **Update** `draft.md` frontmatter `exports:` array with paths + timestamps.
8. **Append** `lock_event` + `export_generated` events to `workspace/campaigns/{slug}/activity.md` (campaign-scoped per Loop 4 redistribution 2026-04-23 — no central-log dual-write).
9. **Surface** output paths to user.

## Versioning

Output filenames follow `{deliverable_name}-v{N}.{ext}` per `config.yaml#versioning`. Don't overwrite — stakeholders cite versions in email threads. If `business-brief-v1.docx` exists, the next export becomes `-v2.docx`.

When a new version is generated, surface: "Generated `business-brief-v2.docx` — want me to draft a follow-up email pointing to v2?" (Email coordination is Phase D scope; surfacing the offer is here.)

## Per-Campaign Template Overrides

If `workspace/campaigns/{slug}/exports/templates/{type}.{ext}` exists, it's used INSTEAD of the system master. Common case: the campaign is for a stakeholder whose team has its own brand template. Drop the override `.docx`/`.pptx` in the campaign's `exports/templates/` folder and the next export uses it automatically.

Override is per-deliverable-type, per-campaign. No multi-level inheritance.

## Dependencies

The agent installs Python libraries ad-hoc on first export — no `requirements.txt` is shipped here because (a) the libraries are imported inline by agent-written code, not by a long-lived script, and (b) the vendored skills include their own install guidance.

Required libraries (`pip install ...` if missing):

- `python-docx` (≥ 1.0)
- `python-pptx` (≥ 1.0)
- `pyyaml` (≥ 6.0)

Agent verifies these are importable on first export and installs them if not.

## What This Folder Does NOT Contain

- **No `md_to_docx.py` / `md_to_pptx.py`** — the agent writes inline code per export, using vendored skills + templates + config. Custom scripts would duplicate the skill's logic and fragment the iteration surface.
- **No marketing-OS wrapper skill** — would collapse constraints into a tool. Skills stay generic; marketing logic stays in `AGENTS.md` + deliverable templates.
- **No headless rendering of HTML carousels to PNG** — that's `system/server/` territory, not exports.
- **No SharePoint / Google Drive sync** — out of scope for POC.
- **No Word change-tracking / comment ingestion** — out of scope for POC.

## Spec Reference

Architecture is documented in [`docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/exports.md`](../../../docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/exports.md). When this README and the spec disagree, this README wins (it's closer to the implementation). Patch the spec when that happens — it's the long-term durable record.
