# Deck Production

Deck and presentation route selection. Load whenever a deliverable needs a `.pptx` or deck-shaped output, or a follow-up pass is requested on an already-shared deck. Deliverable templates call this file instead of naming individual deck tools.

Default: use **PPT Master** for new PowerPoint creation and exports. Use another path only when the input state below requires it or the operator explicitly asks for it.

| Path | Use when | Skill | Outputs |
|---|---|---|---|
| Reference-grounded redesign | An existing PPTX is the visual target and named deltas must not simplify or reinterpret its layouts, diagrams, or native object structure | [`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md); uses PPTX natively and PPT Master only for isolated rebuild slides | Verified 1:1 redesign assembled on a source-deck copy |
| PPT Master | Default for new `.pptx` creation/export from source material, storyboard, Markdown deliverable, research, or an existing deck as source | `system/skills/ppt-master/SKILL.md`; read `system/skills/ppt-master/AGENTS.md` first | Native `.pptx` promoted into the calling deliverable folder |
| PPTX skill (vendored Anthropic) | Native `.pptx` inspection, validation, small edits, extraction-diff, or slide splicing after a PPT Master round trip | `system/skills/pptx/SKILL.md` | Edited `.pptx` via OOXML or pptxgenjs |
| Open Design (bundled) | The work benefits from Open Design's interactive revise-in-UI loop, or the project already runs other visuals through Open Design | `system/skills/open-design/SKILL.md` (defaults in [`image-production.md`](image-production.md)) | Exported `.pptx`, PDF, or PNG in the calling deliverable's media location |

Recommend the default route first; name an exception only when it applies. Record a project-wide preference in `project.md` notes when decks recur. Mixing paths after the initial route is normal: generate with PPT Master or Open Design, then edit or splice with the PPTX skill.

## PPT Master handoff

After selecting PPT Master, read the local overlay then follow the vendor's `SKILL.md`. Its `workflows/routing.md` owns workflow choice, ambiguity handling, and continuous-versus-split session mode; AgentFrame does not duplicate that matrix.

Reference-grounded redesign is selected before this handoff. It is a hybrid preservation path the vendor's rebuild-from-scratch beautify workflow does not own.

## Design language handoff

When the project names a design language, route the identity rather than remember it. A context threading only palette and typography reproduces the colours and loses the archetypes, which is why decks built in a fresh session stop resembling their showcase.

Resolve the name to `library/assets/design-languages/<name>/` (schema: [`library/assets/README.md`](../assets/README.md)).

**Check for `package/` first.** A language whose README declares it reference-grade has none: skip steps 1–2, clone exemplars from its `reference-slides/` per that README, and apply steps 3–4 as normal. The missing package is a tracked Builder task, not a defect to repair mid-build.

Otherwise, before the vendor's confirmation gate:

1. **Preflight the package** from its root: `python system/skills/ppt-master/scripts/svg_quality_checker.py library/assets/design-languages/<name>/package/templates --template-mode`. Any error blocks the run. A package that passed at capture and fails now means the vendor pipeline tightened underneath it, so repair or recapture it here rather than meeting the failure mid-build.
2. **Supply the absolute `package/` root as an exact workspace root for the run.** The vendor records it under `explicit_workspace_roots`, opens Stage 1 in template mode with it preselected, installs it through `apply-template-workspace`, and authors new pages from the installed roster in Stage 2. That install path is what reproduces the archetypes. Never hand-copy roster SVGs into the project instead.
3. **Read the package `README.md` and `imagery/manifest.yaml`.** Select imagery by `slots` and `theme`, and honour `restriction`: `reference-only` never reaches a rendered slide, and `project-scoped` needs this client confirmed as covered before external delivery.
4. **Check `library/assets/logos/`** for the marks the identity's chrome expects.

The language owns identity and structure; this project still owns its content, narrative, and imagery. A named language is not licence to reuse another client's material.

When none is named, the vendor's ordinary identity confirmation governs. Capture runs the other direction, owned by [`library/assets/README.md`](../assets/README.md): when a deck identity proves out and reuse is likely, save it as a package rather than leaving it in the chat that made it.

## Diagram handoff

A slide whose point is a structure — architecture, process, decision, flywheel — is a diagram, and PPT Master has no grammar for one. [`diagram-production.md`](diagram-production.md) owns that route, including the flatten step that lands it as native editable shapes instead of a raster.

Two constraints belong to this route: run `diagram_profile.py` for the named design language *before* any diagram is drawn, since one drawn first arrives in the wrong skin and must be redrawn; and keep charts on PPT Master's Chart executor, so their numbers stay editable.

## Bounded-autonomy runs

A run whose `goal` or `done_when` depends on deck appearance owes rendered visual QA before it may checkpoint, plus a front-loaded confirmation if it needs PPT Master's gate noninteractively. [`deck-bounded-autonomy.md`](deck-bounded-autonomy.md) owns both; ordinary interactive deck work never loads it.

## Deck rows point at a Markdown head

A deck's canonical artifact is a `.pptx`, but the lifecycle buttons stamp frontmatter, so a tracker row pointing straight at the binary has no status to write. `af ready`, `af publish`, and `af version` refuse such a row and name this convention.

The row points at a Markdown head beside the deck, `{deck-name}-v{N}.md`, carrying the ordinary deliverable frontmatter. The exported `.pptx` lands in the deliverable's `media/` folder and is recorded in that head's `exports[]` — the shape the export gate already checks, so a deck gains the same version chain, draft notes, readiness criteria, and export verification every other deliverable has.

Keep the head short: frontmatter, one line naming the current export, and any per-version note. Slide copy lives in the deck and is not restated here.

`af doctor` notes a row still pointing at a binary, so a migration list is discoverable rather than found at closeout.

## Rebuilding a deck after a vendor upgrade

A deck that exported cleanly months ago can fail at rebuild on conventions the vendor tightened underneath it, and those defects arrive mid-build as converter errors that invite editing project content to satisfy a shifting gate. Preflight before authoring.

1. **Run the SVG checker over the existing roster first:** `python system/skills/ppt-master/scripts/svg_quality_checker.py <ppt-project>/svg_output`. It reports every legacy hazard at once with the exact required value. Gradients are the common one and they hard-reject: `gradientUnits` must be `objectBoundingBox` with normalized coordinates, and `gradientTransform`, `spreadMethod`, and `href`-inherited stops are each refused outright.
2. **Read `spec_lock.md` by hand.** No vendor tool validates it before export, so it is the gap the checker does not cover. Two legacy shapes are known: `pptx_structure.mode: baseline`, where the current values are `flat` and `structured`, and a missing `## communication` section.
3. **Migrate the mechanical cases, never the content.** `baseline` becomes `flat`. A full-canvas `userSpaceOnUse` gradient becomes normalized `objectBoundingBox` coordinates inside the range the checker prints. If a fix appears to require cutting or restructuring slide content, stop and say so instead.
4. **Lockless re-export, flat rosters only.** `--quick-generate` is the only path that skips the `spec_lock.md` release gate; naming a source directory with `-s` does not, and fails identically. It needs a matching final quality report and exports `svg_output/` as a flat package, which is why it **cannot export a structured deck**: flat mode forbids `data-pptx-master`, `data-pptx-layout`, `data-pptx-layer`, `data-pptx-carrier`, and `data-pptx-placeholder`, and a structured roster carries all five. A structured deck has no lockless path; it goes through the release gate. Treat any quick-generate output as evidence rather than a release.

A design-language `package/` has no `spec_lock.md` at all — that file is a per-deck-project artifact — so a package cannot be exported to PPTX on its own by any route. Install it into a deck project through `generate-pptx` and export from there.

The SVG checker resolves `design_spec.md` by that exact name, so `--template-mode` targets a package's own `templates/`, never a project that installed one — installation renames the spec to `design_spec.<kind>.<id>.md`, and the checker then reports a missing canvas plus unresolvable `../images/` on every page. Those errors mean the target is wrong, not the package.

## Versioning and round trip

Version identity is the filename's generation timestamp (`deck_20260615_205814.pptx`) and the latest version is the highest one; never use filesystem dates for identity. The promoted copy in the deliverable folder is the operator's working file and may be edited in place. Its same-named twin frozen in the PPT Master project's `exports/` is the agent's reference.

Edit detection: a deliverable copy whose modified time is newer than its filename timestamp has been hand-edited since generation.

Round trip by requested scope:

- **Small wording or formatting:** the operator edits the deliverable copy directly; no agent pass.
- **Specific-slide redo, new slides, or per-slide rewording:** regenerate the affected pages in the PPT Master project, re-export, and splice changed slides into the operator-edited deck via the PPTX skill so untouched slides retain manual work.
- **Deck-wide restructure:** fully regenerate after folding the operator's text edits back into the slide-content source; manual shape edits must be re-specified.

Before any agent pass on an operator-edited deck, extraction-diff the edited copy against the same-named frozen export using the vendored PPTX intake/conversion route. Compare text deltas, shape moves, additions, and deletions; do not use visual or binary diffing. Never overwrite or delete the operator's edited file; each agent pass adds a new timestamped export beside the previous one.
