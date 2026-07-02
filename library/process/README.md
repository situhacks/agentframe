# `library/process/`

Catalog of reusable process files: what each owns and when to load it. Check here before writing a new procedure — if a row covers the job, load that file instead. New or materially reshaped process files follow [`process-authoring.md`](process-authoring.md), which requires updating this catalog in the same change.

## Process index

| Process | Owns | Load when |
|---|---|---|
| [`flows/`](flows/README.md) | Project flow registry (phase structures per domain) | Selecting or executing a project flow; see the flow registry's own README |
| [`browser-fallback.md`](browser-fallback.md) | Browser-workflow fallback principle and routing to `system/browser/` recipes | An approved API/MCP/CLI path is unavailable and browser automation is the fallback |
| [`composio-notes.md`](composio-notes.md) | Publish-prep and performance-capture procedures via Composio/Rube MCP, plus platform quirks | A flow phase coordinates with connected tools (publish, metrics) |
| [`deck-production.md`](deck-production.md) | Deck/PPTX path selection menu + follow-up-pass versioning and round-trip rules | A deliverable needs deck-shaped output and no path is picked, or a delivered deck gets a revision pass |
| [`deliverable-versioning.md`](deliverable-versioning.md) | Naming, head-file, and iteration shape for every versioned deliverable instance | Drafting or iterating any deliverable under `workspace/projects/` |
| [`flow-authoring.md`](flow-authoring.md) | Authoring standard for project flows | Adding or materially reshaping a flow under `flows/` (Builder) |
| [`humanizer-integration.md`](humanizer-integration.md) | Gate timing, scope, and logging for `## Humanizer Pass` sections | A loaded template declares a Humanizer Pass with non-empty guidance |
| [`image-production.md`](image-production.md) | Image-creation path selection menu | A project or post picks an image path (design-language lock, imagery work starts) |
| [`knowledge-base.md`](knowledge-base.md) | Per-project `sources/` + `knowledge/` substrate schema and workflows | Ingesting sources or maintaining governance docs / project knowledge |
| [`lock-event.md`](lock-event.md) | Generic lock mechanics and the post-lock judgment checklist | A deliverable is being locked by state change or clear operator intent |
| [`operator-context-setup.md`](operator-context-setup.md) | First-time generation of operator context surfaces | Loading `library/context/operator/` finds a surface missing |
| [`preview-server.md`](preview-server.md) | Preview offering and hub hygiene for previewable artifacts | A turn writes a hub-supported file (HTML, image, PDF, video) under `workspace/projects/*/` |
| [`process-authoring.md`](process-authoring.md) | Authoring standard for process files, incl. the catalog-row requirement | Creating or materially reshaping a file in this folder (Builder) |
| [`project-frontmatter.md`](project-frontmatter.md) | Canonical project frontmatter schema + drift check + `phase_override` shape | Reading or writing `project.md` frontmatter for state decisions |
| [`research-and-signals.md`](research-and-signals.md) | Shared kickoff research procedure (workspace scan, MCP scan, research-method offer) | Any flow's research phase, or new-project kickoff |
| [`substack-distribution-notes.md`](substack-distribution-notes.md) | Substack MCP setup + republish conventions (working notes; hardening tracked at `BB-2026-06-16-01`) | Publishing or back-publishing to Substack |
| [`video-production.md`](video-production.md) | Video path selection and composition (talking-head, HyperFrames, generated assets, hybrids) | A post or deliverable is video-shaped |
| [`voice-mini-retro.md`](voice-mini-retro.md) | Lock-event eligibility gate for voice harvesting | Called by `lock-event.md`; also after shipped copy materially differs from locked copy |
| [`voice-setup.md`](voice-setup.md) | First-time build of the operator voice system (samples → corpus mine → taste interview → compile) | Loading `library/context/operator/voice/` finds it missing or unbuilt |
