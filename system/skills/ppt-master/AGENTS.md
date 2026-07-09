# PPT Master - AgentFrame Boundary Notes

Vendored deck-generation skill (see `VENDOR.md`). `SKILL.md` owns the generation pipeline; these notes govern how it runs inside AgentFrame. Read this file before running the skill - `library/process/deck-production.md` routes you here, and it is a required read whenever ppt-master is invoked, however the run started.

## Routing

- **Deck routing lives in `library/process/deck-production.md`.** That file owns the default PowerPoint path and all exceptions. Change deck policy there, not in deliverable templates or skill-local notes.
- **PPT Master is AgentFrame's default for new `.pptx` creation/export.** This overlay starts after `deck-production.md` has routed the work to PPT Master and selected the workflow.
- **Dedicated session only.** The pipeline loads several thousand lines of references and generates SVG pages sequentially in main context. Run it as its own working session; never load it mid-campaign-turn alongside campaign context.
- **Project workspace.** Stage its `<project_path>` inside the calling campaign (e.g. `workspace/projects/{slug}/phase-4-production/decks/{deck-name}/`) or `C:\tmp` for throwaway runs - not in this skill folder and not in a repo-root `projects/` directory.

## Run contract

- **Speaker notes off by default.** Skip the Step 6 Logic Construction phase (`notes/total.md`) and Step 7.1 (`total_md_split.py`); export with `svg_to_pptx.py --no-notes`. Generate notes or narration only on explicit operator request.
- **Paragraph authoring.** A multi-line paragraph is ONE `<text>` with dy-stacked `<tspan>` lines — sibling per-line `<text>` elements export as separate PowerPoint text boxes. The export guard denies violations; self-check any page with `python system/hooks/svg_paragraph_lint.py <project_path>/svg_output`.
- **Deterministic guards.** `system/hooks/ppt_master_guard.py` (wired via the tracked `.claude/settings.json`) denies `project_manager.py init` staged inside `system/`, denies exports whose `svg_output/` fails the paragraph lint (prefix `AF_PPT_LINT=off` to skip once for genuine label stacks), and re-injects the promotion contract after each export. `af doctor` backstops any stray that slips through.
- **Operator drafts stay canonical, pass a copy.** The skill's `import-sources --move` would absorb the operator's draft into `sources/`. For operator-authored files, copy into the run instead - the draft (storyboard, slide-content) stays put in the deliverable folder as the source of truth.
- **Divergence pinning.** Operator-authored storyboard / slide-content -> set `content_divergence` to *stay close* (track the source's structure and wording). A redesign-only request additionally pins "no content compression; resize fonts to fit." Both are set at the Strategist confirmation stage, Stage 1 (direction anchors).
- **Voice handoff.** When the run will author or reword copy (anything beyond stay-close verbatim), load `library/context/operator/voice/` and carry it into the Strategist confirmation stage's Identity / Voice & Tone field (Stage 2, design system). Content already drafted in AgentFrame has voice baked in - stay-close preserves it; do not re-apply.
- **Design language.** When the deck belongs to a campaign with a locked design language, hand the palette/typography into the Strategist confirmation stage (Step 4, Stage 2 design system) rather than letting it invent a new identity.
- **Brand assets.** Before design confirmation, check `library/assets/logos/` for every required company/product mark. If missing, follow the sourcing order in `library/assets/README.md`, vendor the credible asset when reuse is likely, and use the real file. Never hand-draw, trace, approximate, or recreate a logo as custom SVG. If no credible asset exists, use a text label or ask the operator.
- **API keys.** Its `image_gen.py` reads the current process env first - the repo root `.env` (`GEMINI_API_KEY`) works with `IMAGE_BACKEND=gemini`. Don't create a second key store inside the skill folder.

## Outputs

- **Promote each export.** After Step 7.3, copy the completed `.pptx` from the working folder's `exports/` up into the calling deliverable folder, keeping its timestamped filename. That copy is the operator's to edit in place; the twin in `exports/` stays frozen as the agent's reference. Versioning and the edit round-trip (extraction-diff before any agent pass, splice vs full regen) are owned by `library/process/deck-production.md`.
