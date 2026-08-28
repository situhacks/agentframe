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

## Design language handoff

When the project names a design language, that identity is routed rather than remembered. A context that threads only palette and typography reproduces the colours and loses the archetypes, which is why decks built in a fresh session stop resembling the showcase they were supposed to match.

Resolve the name to `library/assets/design-languages/<name>/` (schema: [`library/assets/README.md`](../assets/README.md)).

**Check for `package/` first.** A language whose README declares it reference-grade has none: skip steps 1–2, clone exemplars from its `reference-slides/` per that README, and apply steps 3–4 as normal. The missing package is a tracked Builder task, not a defect to repair mid-build.

Otherwise, before the vendor's confirmation gate:

1. **Preflight the package** from its root: `python system/skills/ppt-master/scripts/svg_quality_checker.py library/assets/design-languages/<name>/package/templates --template-mode`. Any error blocks the run. A package that passed at capture and fails now means the vendor pipeline tightened underneath it, so repair or recapture it here instead of meeting the failure mid-build with a cryptic converter error.
2. **Supply the absolute `package/` root as an exact workspace root for the run.** The vendor records it under `explicit_workspace_roots`, opens Stage 1 in template mode with it preselected, installs it through `apply-template-workspace`, and authors new pages from the installed roster in Stage 2. That install path is what reproduces the archetypes. Never hand-copy roster SVGs into the project instead.
3. **Read the package `README.md` and `imagery/manifest.yaml`.** Select imagery by `slots` and `theme`, and honour `restriction`: `reference-only` never reaches a rendered slide, and `project-scoped` needs this client confirmed as covered before external delivery.
4. **Check `library/assets/logos/`** for the marks the identity's chrome expects.

The language owns identity and structure. This project still owns its content, its narrative, and its own imagery; a named language is not licence to reuse another client's material. In a sealed bounded-autonomy run, put the `package/` root in both `context_sources` and `frozen_context`, because an identity the run cannot read is an identity it will invent.

When no design language is named, the vendor's ordinary identity confirmation governs and none of the above applies. Capture runs the other direction and is owned by [`library/assets/README.md`](../assets/README.md): when a deck identity proves out and reuse is likely, save it as a package rather than leaving it in the chat that made it.

## Diagram handoff

A slide whose point is a structure — architecture, process, decision, flywheel — is a diagram, and PPT Master has no grammar for one. [`diagram-production.md`](diagram-production.md) owns that route, including the flatten step that lands it as native editable shapes instead of a raster.

Two constraints belong to this route: run `diagram_profile.py` for the named design language *before* any diagram is drawn, since one drawn first arrives in the wrong skin and must be redrawn; and keep charts on PPT Master's Chart executor, so their numbers stay editable.

## Bounded visual-quality runs

When a bounded-autonomy run's `goal` or `done_when` depends on deck appearance, the approved run contract counts as an explicit request for route-appropriate rendered visual QA. The operator does not need to repeat tool-specific invocation wording.

Before the run can checkpoint for review:

1. Run the selected route's static and structural checks.
2. Render and inspect the complete current deck through its actual viewing surface. For PPT Master, invoke its visual-review workflow after the SVG checker, then inspect a render of the exported PPTX so conversion defects remain visible. Render the exported PPTX natively per [`native-office-render.md`](native-office-render.md) — the SVG stage is not the PPTX, and a LibreOffice render of it would invent defects that are not in the file.
3. Fix the findings, rerender the affected slides, and inspect them again. A successful export or a first-pass claim of "no issues" is not completion evidence.
4. Record the render/review artifact paths and a concise finding -> fix -> recheck result in the autonomy evidence. If a finding requires a decision outside the approved charter, checkpoint with exact outcome `blocked` instead of asking during the run.

## Front-loaded PPT Master confirmation

The normal vendor confirmation gate remains the default. A repo-contained PPT project may use a noninteractive gate only when an approved bounded-autonomy run front-loads the complete confirmation:

1. Put every Step 4 content source, required `analysis/` identity/slide-library file, template/brand input, and the future sealed wrapper path in both the run's `context_sources` and `frozen_context`. Copy external inputs into the repo first.
2. Restrict `allowed_paths` to the mutable PPT project. It may not equal or contain the sibling sealed wrapper.
3. Write `<ppt-project>/agentframe-confirmation.draft.json` using the current final-result shape owned by the vendor's `scripts/docs/confirm_ui.md`. Use `mode: fixed-values` for exact operator-approved choices. Use `mode: delegate-strategist` only with exact delegation `{ "fields": "all", "constraints": {} }`; partial or constrained delegation uses the normal UI.
4. Before starting the run, seal it:

```powershell
python system/tools/ppt_master_contract.py seal <ppt-project> `
  --run <run-file> --draft <draft-json>
python system/af.py autonomy start <project> <run-id> `
  --session-binding <harness>:<session-id>
```

The run-unique sibling is `<deck-name>.<run-id>.agentframe-confirmation.json`. It records the pinned vendor commit, input hashes, approval record, and exact final result. The seal is immutable; `by: operator` is a durable record, not cryptographic authentication.

On the vendor's recognized confirmation waits (`stage1` and `final` since vendor `52e85a0`), `ppt_master_guard.py` validates the wrapper, input closure, run hash, and exact session, then blocks the redundant UI launch with the materialize command. Run that command and continue after the gate:

```powershell
python system/tools/ppt_master_contract.py materialize <sealed-wrapper> `
  --session-binding <harness>:<session-id>
```

Export revalidates the complete closure and exact materialized result. Any declared but malformed, drifted, ambiguous, or wrong-session contract blocks. No wrapper means the ordinary vendor UI proceeds. `C:\tmp`, untrusted/missing hooks, and Cursor unattended runs do not claim this guarantee.

## Deck rows point at a Markdown head

A deck's canonical artifact is a `.pptx`, but the lifecycle buttons stamp frontmatter, so a tracker row pointing straight at the binary has no status to write. `af ready`, `af publish`, and `af version` refuse such a row and name this convention.

The row points at a Markdown head beside the deck, `{deck-name}-v{N}.md`, carrying the ordinary deliverable frontmatter. The exported `.pptx` lands in the deliverable's `media/` folder and is recorded in that head's `exports[]`. This is the shape the export gate already checks, so a deck gains the same version chain, draft notes, readiness criteria, and export verification every other deliverable has, with no second lifecycle path to maintain.

Keep the head short: frontmatter, one line naming the current export, and any per-version note. Slide copy lives in the deck and is not restated here.

`af doctor` surfaces a row still pointing at a binary as an advisory note, so an existing project's migration list is discoverable rather than found at closeout.

## Rebuilding a deck after a vendor upgrade

PPT Master is vendored and under active upgrade, so a deck that exported cleanly months ago can fail at rebuild on conventions that tightened underneath it. Preflight before authoring. The same defects found at export arrive as converter errors mid-build, which invites editing project content to satisfy a shifting gate.

1. **Run the SVG checker over the existing roster first:** `python system/skills/ppt-master/scripts/svg_quality_checker.py <ppt-project>/svg_output`. It reports every legacy hazard at once with the exact required value. Gradients are the common one and they hard-reject: `gradientUnits` must be `objectBoundingBox` with normalized coordinates, and `gradientTransform`, `spreadMethod`, and `href`-inherited stops are each refused outright.
2. **Read `spec_lock.md` by hand.** No vendor tool validates it before export, so it is the gap the checker does not cover. Two legacy shapes are known: `pptx_structure.mode: baseline`, where the current values are `flat` and `structured`, and a missing `## communication` section.
3. **Migrate the mechanical cases, never the content.** `baseline` becomes `flat`. A full-canvas `userSpaceOnUse` gradient becomes normalized `objectBoundingBox` coordinates inside the range the checker prints. If a fix appears to require cutting or restructuring slide content, stop and say so instead.
4. **Diagnostic re-export escape hatch.** To re-export an already-approved deck without re-satisfying the release gate, name the final source directory explicitly: `python system/skills/ppt-master/scripts/svg_to_pptx.py <ppt-project> -s output/final/<name>`. The vendor's own help labels this path diagnostics-only, so treat the output as evidence rather than a release and record why the gate was bypassed.

A reference or showcase deck carrying one slide per archetype also trips release-gate bookkeeping that is not maintained for that kind of deck, such as `design_spec.md` §IX slide blocks. Use the diagnostic export for those and keep the bookkeeping burden off the reference roster.

## Versioning and round trip

Version identity is the timestamp in the filename (generation time, for example `deck_20260615_205814.pptx`). Latest version is the highest sortable filename timestamp; never rely on filesystem created or modified dates for identity. The promoted copy in the deliverable folder is the operator's working file and may be edited in place. The same-named twin frozen in the PPT Master project's `exports/` is the agent's reference.

Edit detection: when the deliverable copy's modified time is newer than the timestamp in its filename, the operator has hand-edited it since generation.

Round trip by requested scope:

- **Small wording or formatting:** the operator edits the deliverable copy directly; no agent pass.
- **Specific-slide redo, new slides, or per-slide rewording:** regenerate the affected pages in the PPT Master project, re-export, and splice changed slides into the operator-edited deck via the PPTX skill so untouched slides retain manual work.
- **Deck-wide restructure:** fully regenerate after folding the operator's text edits back into the slide-content source; manual shape edits must be re-specified.

Before any agent pass on an operator-edited deck, extraction-diff the edited copy against the same-named frozen export using the vendored PPTX intake/conversion route. Compare text deltas, shape moves, additions, and deletions; do not use visual or binary diffing. Never overwrite or delete the operator's edited file. Each agent pass produces a new timestamped export beside the previous one.
