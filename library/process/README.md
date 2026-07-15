# `library/process/`

Catalog of reusable process files: what each owns and when to load it. Check here before writing a new procedure — if a row covers the job, load that file instead. New or materially reshaped process files follow [`process-authoring.md`](process-authoring.md), which requires updating this catalog in the same change.

## Process index

| Process | Owns | Load when |
|---|---|---|
| [`flows/`](flows/README.md) | Project flow registry (phase structures per domain) | Selecting or executing a project flow; see the flow registry's own README |
| [`browser-fallback.md`](browser-fallback.md) | Browser-workflow fallback principle and routing to `system/browser/` recipes | An approved API/MCP/CLI path is unavailable and browser automation is the fallback |
| [`bounded-autonomy.md`](bounded-autonomy.md) | Readiness, authority bounds, tiered model routing, checkpoints, and stop rules for one bounded autonomous goal | The operator authorizes iterative autonomous work on a defined project outcome |
| [`career-harvest.md`](career-harvest.md) | Moving real wins into the career bank (proof-points, master-cv bullets, stories) with origin links | A project closeout/system retro runs, or the operator names a win or asks to update the resume bank |
| [`composio-notes.md`](composio-notes.md) | Publish-prep and performance-capture procedures via Composio/Rube MCP, plus platform quirks | A flow phase coordinates with connected tools (publish, metrics) |
| [`deck-production.md`](deck-production.md) | Central Deck/PPTX route, PPT Master default, and follow-up-pass versioning/round-trip rules | A deliverable needs deck-shaped output, or a delivered deck gets a revision pass |
| [`deliverable-versioning.md`](deliverable-versioning.md) | First-draft scaffolding, row/nested-artifact addresses, immutable snapshots, and surgical-versus-replacement iteration | Before the first write or rewrite to a kept project deliverable, including every resumed drafting context |
| [`diagram-production.md`](diagram-production.md) | Static graph-shaped explainers through the D2 renderer | A process, decision, handoff, dependency, or system needs connected nodes and edges |
| [`flow-authoring.md`](flow-authoring.md) | Authoring standard for project flows | Adding or materially reshaping a flow under `flows/` (Builder) |
| [`humanizer-integration.md`](humanizer-integration.md) | Authorship-aware timing and scope for humanizing public-facing agent prose | A public-facing template directly requires it before writing, during agent rewriting, or at lock verification |
| [`image-production.md`](image-production.md) | Image-creation path selection menu | A project or post picks an image path (design-language lock, imagery work starts) |
| [`knowledge-base.md`](knowledge-base.md) | Per-project `sources/` + `knowledge/` substrate schema and workflows | Ingesting sources or maintaining governance docs / project knowledge |
| [`lock-event.md`](lock-event.md) | Generic lock mechanics and the post-lock judgment checklist | A deliverable is being locked by state change or clear operator intent |
| [`operator-context-setup.md`](operator-context-setup.md) | First-time generation of operator context surfaces | Loading `library/context/operator/` finds a surface missing |
| [`preview-server.md`](preview-server.md) | Workspace Dashboard start-or-open, preview deep links, stop, noise hygiene | The operator explicitly asks for the dashboard, calendar, or a browser preview |
| [`project-automation.md`](project-automation.md) | Project-attached standing automations: contract, lifecycle, deployment join, and promotion boundary | Project work becomes recurring or event-driven managed execution, or `project.md` has an `automations` row |
| [`process-authoring.md`](process-authoring.md) | Authoring standard for process files, incl. the catalog-row requirement | Creating or materially reshaping a file in this folder (Builder) |
| [`project-frontmatter.md`](project-frontmatter.md) | Canonical project frontmatter schema + drift check + `phase_override` shape | Reading or writing `project.md` frontmatter for state decisions |
| [`reference-grounded-deck-redesign.md`](reference-grounded-deck-redesign.md) | Native-first preservation, isolated rebuild, assembly, and verification for reference PPTX redesigns | An existing PPTX is the visual target and its layouts or intricate diagrams must survive except for named deltas |
| [`research-and-signals.md`](research-and-signals.md) | Shared kickoff research procedure (workspace scan, MCP scan, research-method offer) | Any flow's research phase, or new-project kickoff |
| [`substack-publishing.md`](substack-publishing.md) | Substack draft preparation, MCP/editor handoff, back-publishing, and live-result reconciliation | A project targets Substack and locked copy is ready to draft, update, publish, or back-publish |
| [`technical-build.md`](technical-build.md) | Technical-build lifecycle: external repo + umbilical stub, derived status, BDRs, graduation compile | `project.md` has `build_repo` set and the build is not graduated (a phase turns into a code POC/app) |
| [`video-production.md`](video-production.md) | Video path selection and composition (talking-head, HyperFrames, generated assets, hybrids) | A post or deliverable is video-shaped |
| [`voice-mini-retro.md`](voice-mini-retro.md) | Lock-event eligibility gate for voice harvesting | Called by `lock-event.md`; also after shipped copy materially differs from locked copy |
| [`voice-setup.md`](voice-setup.md) | First-time build of the operator voice system (samples → corpus mine → taste interview → compile) | Loading `library/context/operator/voice/` finds it missing or unbuilt |
