# Deck Production

Available deck/presentation paths for AgentFrame Marketing. Load this when a deliverable needs a `.pptx` (or deck-shaped) output and a path hasn't been picked. Usage rules, lock criteria, and export records are owned by the deliverable that calls this menu — brief templates own their own export mechanics; campaign posts record deck media like any other shipped asset.

| Path | Use when | Skill | Outputs |
|---|---|---|---|
| PPTX skill (vendored Anthropic) | Editing, filling, or validating an existing `.pptx`; brief exports from a known template; quick decks where design polish matters less than turnaround. The default for anything that starts FROM a `.pptx`. | `system/skills/pptx/SKILL.md` | Edited/created `.pptx` via OOXML or pptxgenjs |
| PPT Master | Generating a designed, native-editable deck from source material (docs, research, campaign artifacts) — best visual quality, charts, speaker notes, optional narration. Heavy: runs as its own dedicated session. | `system/skills/ppt-master/SKILL.md` + boundary notes in `system/skills/ppt-master/AGENTS.md` | Native `.pptx` from its SVG pipeline, exported into the calling deliverable's folder |
| Open Design (bundled) | Deck work that benefits from OD's interactive revise-in-UI loop, or when the campaign is already running other visuals through OD. | `system/skills/open-design/SKILL.md` (mode/skill defaults in [`image-production.md`](image-production.md)) | Exported `.pptx` / PDF / PNG to the calling post's `visuals/imports/` |

Offer the options and let the operator narrow; record a campaign-wide preference in `project.md` `post_manifest` notes when decks recur in a campaign. Mixing paths is normal — generate with PPT Master or OD, then edit/fill the result with the PPTX skill.
