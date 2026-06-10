# PPT Master — AgentFrame Boundary Notes

Vendored deck-generation skill (see `VENDOR.md`). `SKILL.md` owns the generation pipeline; these notes govern how it runs inside AgentFrame.

- **Pick it through the menu.** Deck path selection lives in `library/process/deck-production.md`. This skill is the from-scratch, high-design generation path — not the default for editing or filling an existing `.pptx` (that stays with `system/skills/pptx/`).
- **Dedicated session only.** The pipeline loads several thousand lines of references and generates SVG pages sequentially in main context. Run it as its own working session; never load it mid-campaign-turn alongside campaign context.
- **Project workspace.** Stage its `<project_path>` inside the calling campaign (e.g. `workspace/campaigns/{slug}/phase-4-production/decks/{deck-name}/`) or `C:\tmp` for throwaway runs — not in this skill folder and not in a repo-root `projects/` directory.
- **Outputs return to the campaign.** The exported `.pptx` (and any speaker notes/audio) lands in the calling deliverable's folder; record it like any media output. Lock criteria stay owned by the calling deliverable template.
- **API keys.** Its `image_gen.py` reads the current process env first — the repo root `.env` (`GEMINI_API_KEY`) works with `IMAGE_BACKEND=gemini`. Don't create a second key store inside the skill folder.
- **Design language.** When the deck belongs to a campaign with a locked design language, hand the palette/typography into the Strategist confirmations (Step 4) rather than letting it invent a new identity.
