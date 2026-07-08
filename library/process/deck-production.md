# Deck Production

Available deck/presentation paths for AgentFrame. Load this whenever a deliverable needs a `.pptx` (or deck-shaped) output, **and** whenever a follow-up pass is requested on an already-delivered deck (redo a slide, add slides, restructure, reword) — the versioning and round-trip rules below govern those passes. Deliverable templates call this file instead of naming individual deck tools directly. Usage rules, lock criteria, and export records are owned by the deliverable that calls this menu — project posts record deck media like any other delivered asset.

Default: use **PPT Master** for new PowerPoint creation and exports. Use another path only when the input state below requires it or the operator explicitly asks for it.

| Path | Use when | Skill | Outputs |
|---|---|---|---|
| PPT Master | Default for new `.pptx` creation/export, from source material, storyboard, markdown deliverable, research, or existing deck-as-source. Best visual quality, native-editable output, charts, optional speaker notes/narration. Heavy: runs as its own dedicated session. Pick the specific workflow from the table below. | `system/skills/ppt-master/SKILL.md`; read the overlay `system/skills/ppt-master/AGENTS.md` first | Native `.pptx` from its SVG pipeline, promoted into the calling deliverable's folder |
| PPTX skill (vendored Anthropic) | Native `.pptx` inspection, validation, small edits, extraction-diff, or slide splicing after a PPT Master round-trip. Use only when this file routes there. | `system/skills/pptx/SKILL.md` | Edited `.pptx` via OOXML or pptxgenjs |
| Open Design (bundled) | Deck work that benefits from OD's interactive revise-in-UI loop, or when the project is already running other visuals through OD. | `system/skills/open-design/SKILL.md` (mode/skill defaults in [`image-production.md`](image-production.md)) | Exported `.pptx` / PDF / PNG to the calling post's `visuals/imports/` |

Recommend the default route first and name the exception only when it applies. Record a project-wide preference in `project.md` notes when decks recur in a project. Mixing paths is normal after the initial route — generate with PPT Master or OD, then edit/splice the result with the PPTX skill.

## PPT Master workflow selection

When PPT Master is the path, recommend the specific built-in workflow — do not default to the main pipeline for everything. Confirm the workflow with the operator before running.

| Operator intent | Workflow |
|---|---|
| Storyboard / slide-content → new designed deck (authored in AgentFrame, or a storyboard a coworker handed over) | Main pipeline, `content_divergence` = stay close |
| Redesign an existing `.pptx` — keep every page, its order, and its wording; fix only layout / hierarchy / whitespace | `beautify` (strictly 1:1) |
| Put my content into a coworker's native `.pptx` template | `template-fill` (clones slides, fills text in OOXML, no SVG generation) |
| Save a deck design or brand identity I like for reuse | `create-template` / `create-brand` → package stored in `library/assets/deck-templates/` |

Discriminator (redesign vs restructure): if page count or order changes at all, it is the main pipeline, never `beautify`. On an ambiguous ask ("make this deck more professional"), ask one question — preserve page split + wording, or treat as source and restructure? — then route.

## Versioning & round-trip

Version identity is the timestamp **in the filename** (generation time, e.g. `deck_20260615_205814.pptx`). Latest version = highest filename timestamp, sortable by name; never rely on filesystem created/modified dates for identity. The promoted copy in the deliverable folder is the operator's working file — edited in place, no rename. The same-named twin frozen in the working folder's `exports/` is the agent's reference.

Edit detection: when the deliverable copy's modified time is newer than the timestamp in its own filename, the operator has hand-edited it since generation.

Round-trip by scope of the requested change:

- **Small wording / formatting** — operator edits the deliverable copy directly; no agent pass.
- **Specific-slide redo, new slides, per-slide reword** — agent regenerates the affected pages in the working folder (SVG sources persist there), re-exports, and splices the changed slides into the operator's edited deck via the `pptx` skill so untouched slides keep the operator's manual work.
- **Deck-wide restructure** — full regeneration; fold the operator's text edits back into the slide-content source first; manual shape edits are acknowledged as needing re-specification.

Before any agent pass on an operator-edited deck, extraction-diff to see what changed: run `pptx_intake.py` + `ppt_to_md.py` on the edited copy and on the same-named frozen `exports/` twin, then diff the extractions (text deltas, shape moves, adds/deletes). No visual/binary diffing. The operator's edited file is never overwritten or deleted; each pass produces a new timestamped export, promoted beside the previous one.
