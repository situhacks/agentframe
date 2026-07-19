# Deck Production

Available deck and presentation paths for AgentFrame. Load this whenever a deliverable needs a `.pptx` or deck-shaped output, and whenever a follow-up pass is requested on an already-shared deck. Deliverable templates call this file instead of naming individual deck tools directly.

Default: use **PPT Master** for new PowerPoint creation and exports. Use another path only when the input state below requires it or the operator explicitly asks for it.

| Path | Use when | Skill | Outputs |
|---|---|---|---|
| Reference-grounded redesign | An existing PPTX is the visual target and named deltas must not simplify or reinterpret its layouts, diagrams, or native object structure | [`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md); uses PPTX natively and PPT Master only for isolated rebuild slides | Verified 1:1 redesign assembled on a source-deck copy |
| PPT Master | Default for new `.pptx` creation/export from source material, storyboard, Markdown deliverable, research, or an existing deck as source. The vendored router owns workflow and session-mode selection. | `system/skills/ppt-master/SKILL.md`; read `system/skills/ppt-master/AGENTS.md` first | Native `.pptx` promoted into the calling deliverable folder |
| PPTX skill (vendored Anthropic) | Native `.pptx` inspection, validation, small edits, extraction-diff, or slide splicing after a PPT Master round trip | `system/skills/pptx/SKILL.md` | Edited `.pptx` via OOXML or pptxgenjs |
| Open Design (bundled) | The work benefits from Open Design's interactive revise-in-UI loop, or the project already runs other visuals through Open Design | `system/skills/open-design/SKILL.md` (defaults in [`image-production.md`](image-production.md)) | Exported `.pptx`, PDF, or PNG in the calling deliverable's media location |

Recommend the default route first and name an exception only when it applies. Record a project-wide preference in `project.md` notes when decks recur. Mixing paths is normal after the initial route: generate with PPT Master or Open Design, then edit or splice with the PPTX skill.

## PPT Master handoff

After AgentFrame selects PPT Master, read the local overlay and then follow the vendor's `SKILL.md`. Its `workflows/routing.md` owns workflow choice, ambiguity handling, and continuous-versus-split session mode. AgentFrame does not duplicate that matrix.

Reference-grounded redesign is selected before this handoff. It is a hybrid preservation path the vendor's rebuild-from-scratch beautify workflow does not own.

## Bounded visual-quality runs

When a bounded-autonomy run's `goal` or `done_when` depends on deck appearance, the approved run contract counts as an explicit request for route-appropriate rendered visual QA. The operator does not need to repeat tool-specific invocation wording.

Before the run can checkpoint for review:

1. Run the selected route's static and structural checks.
2. Render and inspect the complete current deck through its actual viewing surface. For PPT Master, invoke its visual-review workflow after the SVG checker, then inspect a render of the exported PPTX so conversion defects remain visible.
3. Fix the findings, rerender the affected slides, and inspect them again. A successful export or a first-pass claim of "no issues" is not completion evidence.
4. Record the render/review artifact paths and a concise finding -> fix -> recheck result in the autonomy evidence. If a finding requires a decision outside the approved charter, checkpoint `bready` instead of asking during the run.

## Versioning and round trip

Version identity is the timestamp in the filename (generation time, for example `deck_20260615_205814.pptx`). Latest version is the highest sortable filename timestamp; never rely on filesystem created or modified dates for identity. The promoted copy in the deliverable folder is the operator's working file and may be edited in place. The same-named twin frozen in the PPT Master project's `exports/` is the agent's reference.

Edit detection: when the deliverable copy's modified time is newer than the timestamp in its filename, the operator has hand-edited it since generation.

Round trip by requested scope:

- **Small wording or formatting:** the operator edits the deliverable copy directly; no agent pass.
- **Specific-slide redo, new slides, or per-slide rewording:** regenerate the affected pages in the PPT Master project, re-export, and splice changed slides into the operator-edited deck via the PPTX skill so untouched slides retain manual work.
- **Deck-wide restructure:** fully regenerate after folding the operator's text edits back into the slide-content source; manual shape edits must be re-specified.

Before any agent pass on an operator-edited deck, extraction-diff the edited copy against the same-named frozen export using the vendored PPTX intake/conversion route. Compare text deltas, shape moves, additions, and deletions; do not use visual or binary diffing. Never overwrite or delete the operator's edited file. Each agent pass produces a new timestamped export beside the previous one.
