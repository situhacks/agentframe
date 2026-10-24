# Image Production

Available image-creation paths for AgentFrame. Load this when a project or post needs to pick an image path—typically when the design language becomes ready (the project-wide preference lands in `project.md` `post_manifest`) or when a post's imagery work starts. Usage rules, readiness criteria, and provenance details are owned by the deliverable that calls this menu.

| Path | Use when | Owner deliverable | Outputs |
|---|---|---|---|
| D2 diagram renderer | A flow, decision tree, handoff, or system explainer is primarily a graph of nodes and edges. | Calling deliverable; load [`diagram-production.md`](diagram-production.md) | `diagram.d2` plus a derived SVG beside the owning visual artifact |
| HTML render + browser screenshot | Slide-shaped visuals from tokens/layout are enough (covers, diagrams, doc-style panels). | `design-language` (tokens + layout guidance) | `carousel-slide-{i}.html` and manual PNG screenshots in the post's `visuals/` |
| Gemini Nano Banana 2 (`gemini-3.1-flash-image-preview`) | Default raster generation for standard illustrations and fast A/B/C variants. | `image-prompts` | `image-variant-{a|b|c}.png` via `system/tools/image_generate.py` |
| Gemini Nano Banana Pro (`gemini-3-pro-image-preview`) | Prompt fidelity and text-in-image quality matter more than speed (hero or leadership-facing visuals). | `image-prompts` | `image-variant-{a|b|c}.png` via `system/tools/image_generate.py` |
| External / stock / operator-provided | Existing assets are already the right fit; no model generation needed. The operator's own captures live on the shared media shelf (`library/assets/media/`, schema in [`library/assets/README.md`](../assets/README.md)) and are found with `af search "<scene>"`; reference the card's path, never copy the file. | calling deliverable | Referenced asset path(s) in the post folder |

## Record convention

Prompts and their iteration trail are owned by the post's `image-prompts-v{N}.md` — shape, constraints, and readiness criteria in [`library/deliverables/image-prompts/template.md`](../deliverables/image-prompts/template.md). Working renders and variants land in the post's `images/` or `visuals/` folder beside it; approved finals land in the deliverable folder's `media/` and are recorded in the owning deliverable's `exports[]` frontmatter. Gemini API docs own general prompt practice; the design language owns the reusable treatment block the prompts build on.

## Nano Banana prompts

- Name known products, styles, and palettes plainly ("GMK Modern Dolch Light palette"); the model searches, and a paragraph describing a known mark reads worse than its name.
- Keep internal names (project slugs, design-language file names) out of prompts; they mean nothing to the model.
- A mark newer than the model's training renders wrong from its name: download the real artwork, look at it, and describe its shape, colour, and features plainly.
- Mix photoreal physical objects (index cards, foil stars, vinyl stickers, sticky notes, folders) with flat UI screenshots and real type; an all-flat vector slide reads as AI slop.
- Name each sticker and star on the slide that carries it and say "each appearing once"; a shared style line that lists them scatters them over every slide.
- When type sits over a generated plate, reprompt with an exact band (artwork between two height percentages, side and bottom margins, nothing touching an edge, one even background colour) instead of padding, mirroring, or re-cutting the plate.
- Check every render for seams and hard cut-offs, and flag them before the operator reviews it.
