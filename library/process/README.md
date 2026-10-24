# `library/process/`

Catalog of reusable process files: what each owns and when to load it. Check here before writing a new procedure; if a row covers the job, load that file instead. New, reshaped, or retired process files follow [`authoring.md`](authoring.md), which requires updating this catalog in the same change.

## Process index

| Process | Owns | Load when |
|---|---|---|
| [`authoring.md`](authoring.md) | The process-file and deliverable-template shapes, the catalog-row rule, how a new deliverable type is born, and the size caps | Creating, reshaping, or retiring a process file or template, or adding a deliverable type (Builder) |
| [`career-harvest.md`](career-harvest.md) | Moving real wins into the career bank (proof-points, master-cv bullets, stories) with origin links | A project closes out, or the operator names a win or asks to update the resume bank |
| [`deck-production.md`](deck-production.md) | Central Deck/PPTX route, PPT Master default, saved design-language handoff, and follow-up-pass versioning/round-trip rules | A deliverable needs deck-shaped output, a project names a design language, or a shared deck gets a revision pass |
| [`deliverable-versioning.md`](deliverable-versioning.md) | First-draft scaffolding, row/nested-artifact addresses, immutable snapshots, and the version-before-every-content-change rule | Before the first write or rewrite to a kept project deliverable, including every resumed drafting context |
| [`diagram-production.md`](diagram-production.md) | Diagram route selection (Diagram Design default, D2 for regenerated graphs), the PPT Master native-shape handoff, and design-language brand projection | A system, process, decision, hierarchy, or comparison needs a structural visual |
| [`image-production.md`](image-production.md) | Image-creation path selection menu | A project or post picks an image path (design language becomes ready, imagery work starts) |
| [`knowledge-base.md`](knowledge-base.md) | Per-project `sources/` + `knowledge/` substrate schema and workflows | Ingesting sources or maintaining project knowledge |
| [`media-intake.md`](media-intake.md) | Intake of the operator's own recordings, footage and photos onto the shared media shelf: inventory, dedupe, placement, proxies, review contract, cards | The operator hands over captures to organize or index, or a post/project recording should be registered as reusable |
| [`native-office-render.md`](native-office-render.md) | Native Microsoft Office COM rendering of PPTX/DOCX to PNG/PDF; the no-LibreOffice rule | A PPTX/DOCX must be seen rather than parsed (visual QA, origin renders, thumbnails, preview) |
| [`ready-event.md`](ready-event.md) | Generic readiness/export gate, post-ready judgment checklist, and the voice-harvest gate | A deliverable is being marked good enough to use or share |
| [`operator-context-setup.md`](operator-context-setup.md) | First-time generation of operator context surfaces | Loading `workspace/life/` finds a surface missing |
| [`performance-capture.md`](performance-capture.md) | Post-publication metrics procedure (connector-first, manual gap-fill) and the raw performance CSV shape | About 14 days after a post ships, or a project plan reaches its performance-capture step |
| [`project-activity.md`](project-activity.md) | Material-event line shapes and unresolved Attention shortlist | Appending or interpreting a project event, resolving Attention, or answering a history/why question |
| [`project-flow.md`](project-flow.md) | The one project shape: kickoff plan, runway loop, files, tracker updates, closeout | Starting a project, working its runway, or closing it out |
| [`project-frontmatter.md`](project-frontmatter.md) | Compact current-state index, fresh-context load order, schema, and drift check | Reading or writing `project.md` frontmatter for state, routing, or next-action decisions |
| [`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md) | Native-first preservation, isolated rebuild, assembly, and verification for reference PPTX redesigns | An existing PPTX is the visual target and its layouts or intricate diagrams must survive except for named deltas |
| [`research-and-signals.md`](research-and-signals.md) | Shared kickoff research procedure (workspace scan, MCP scan, research-method offer) | A project's research phase, or new-project kickoff |
| [`short-form-edit.md`](short-form-edit.md) | Footage-first talking-head production: transcribe, cut, edit-decision grammar, compose, render, QC, caption path, creator studies | A recording must become a short-form post, or a hook study is requested |
| [`substack-publishing.md`](substack-publishing.md) | Substack draft preparation, MCP/editor handoff, back-publishing, and live-result reconciliation | A project targets Substack and ready copy needs drafting, updating, publishing, or back-publishing |
| [`video-production.md`](video-production.md) | Spec-first video path selection and composition through HyperFrames | A project plans a video deliverable from a brief; raw footage that needs a cut goes to `short-form-edit.md` |
| [`voice-setup.md`](voice-setup.md) | First-time build of the operator voice system (samples → corpus mine → taste interview → compile) | Loading `workspace/life/voice/` finds it missing or unbuilt |
