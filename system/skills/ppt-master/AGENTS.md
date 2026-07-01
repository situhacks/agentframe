# PPT Master — AgentFrame Boundary Notes

Vendored deck-generation skill (see `VENDOR.md`). `SKILL.md` owns the generation pipeline; these notes govern how it runs inside AgentFrame. Read this file before running the skill — `library/process/deck-production.md` routes you here, and it is a required read whenever ppt-master is invoked, however the run started.

## Routing

- **Pick the workflow through the menu.** Deck path + ppt-master workflow selection (main pipeline / `beautify` / `template-fill` / `create-template`) lives in `library/process/deck-production.md`. This skill is the from-scratch, high-design generation path — not the default for editing or filling an existing `.pptx` (that stays with `system/skills/pptx/`).
- **Dedicated session only.** The pipeline loads several thousand lines of references and generates SVG pages sequentially in main context. Run it as its own working session; never load it mid-campaign-turn alongside campaign context.
- **Project workspace.** Stage its `<project_path>` inside the calling campaign (e.g. `workspace/projects/{slug}/phase-4-production/decks/{deck-name}/`) or `C:\tmp` for throwaway runs — not in this skill folder and not in a repo-root `projects/` directory.

## Run contract

- **Speaker notes off by default.** Skip the Step 6 Logic Construction phase (`notes/total.md`) and Step 7.1 (`total_md_split.py`); export with `svg_to_pptx.py --no-notes`. Generate notes or narration only on explicit operator request.
- **Operator drafts stay canonical, pass a copy.** The skill's `import-sources --move` would absorb the operator's draft into `sources/`. For operator-authored files, copy into the run instead — the draft (storyboard, slide-content) stays put in the deliverable folder as the source of truth.
- **Divergence pinning.** Operator-authored storyboard / slide-content → set `content_divergence` to *stay close* (track the source's structure and wording). A redesign-only request additionally pins "no content compression; resize fonts to fit." Both are set at the Strategist Eight Confirmations.
- **Voice handoff.** When the run will author or reword copy (anything beyond stay-close verbatim), load `library/context/operator/voice/` and carry it into the Strategist Voice & Tone confirmation. Content already drafted in AgentFrame has voice baked in — stay-close preserves it; do not re-apply.
- **Design language.** When the deck belongs to a campaign with a locked design language, hand the palette/typography into the Strategist confirmations (Step 4) rather than letting it invent a new identity.
- **Logos.** Before generating or asking for a brand logo, check `library/assets/logos/` first; else fetch from an open source (Simple Icons, gilbarbara/logos, Wikimedia) and save it into `library/assets/logos/` as `<brand>-<variant>.<ext>`; ask the operator only as a last resort. See `library/assets/README.md`.
- **API keys.** Its `image_gen.py` reads the current process env first — the repo root `.env` (`GEMINI_API_KEY`) works with `IMAGE_BACKEND=gemini`. Don't create a second key store inside the skill folder.

## Outputs

- **Promote each export.** After Step 7.3, copy the completed `.pptx` from the working folder's `exports/` up into the calling deliverable folder, keeping its timestamped filename. That copy is the operator's to edit in place; the twin in `exports/` stays frozen as the agent's reference. Versioning and the edit round-trip (extraction-diff before any agent pass, splice vs full regen) are owned by `library/process/deck-production.md`.
