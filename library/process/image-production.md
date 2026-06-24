# Image Production

Available image-creation paths for AgentFrame. Load this when a project or post needs to pick an image path — typically when the design language locks (the project-wide preference lands in `project.md` `post_manifest`) or when a post's imagery work starts. Usage rules, lock criteria, and provenance details are owned by the deliverable that calls this menu.

| Path | Use when | Owner deliverable | Outputs |
|---|---|---|---|
| HTML render + browser screenshot | Slide-shaped visuals from tokens/layout are enough (covers, diagrams, doc-style panels). | `design-language` (tokens + layout guidance) | `carousel-slide-{i}.html` and manual PNG screenshots in the post's `visuals/` |
| Gemini Nano Banana 2 (`gemini-3.1-flash-image-preview`) | Default raster generation for standard illustrations and fast A/B/C variants. | `image-prompts` | `image-variant-{a|b|c}.png` via `system/server/lib/image_generate.py` |
| Gemini Nano Banana Pro (`gemini-3-pro-image-preview`) | Prompt fidelity and text-in-image quality matter more than speed (hero or leadership-facing visuals). | `image-prompts` | `image-variant-{a|b|c}.png` via `system/server/lib/image_generate.py` |
| Open Design (bundled) | You want higher-fidelity designed outputs, or you want to render a whole social carousel as a single design pass. | `image-prompts` | Exported files (HTML / PNG / PDF / PPTX / ZIP) saved to the calling post's `visuals/imports/`; use `system/skills/open-design/SKILL.md` |
| External / stock / operator-provided | Existing assets are already the right fit; no model generation needed. | calling deliverable | Referenced asset path(s) in the post folder |

### Open Design mode + skill defaults

When the Open Design path is picked, stage the project with these defaults (operator can override):

| Calling deliverable shape | OD mode | OD skillId | Why |
|---|---|---|---|
| `image-prompts` single image (e.g. 1080x1080 LinkedIn hero) | `image` | `canvas-design` | Image-mode skill for posters, illustrations, and static pieces. Custom canvas sizes are first-class. |
| Square social carousel (locked slide copy + design language) | `image` | `canvas-design` per slide | Best canvas fidelity for LinkedIn/IG square slides. Stage one project per slide, or one project iterated slide-by-slide, so every slide honors the project DL. |
| Content-heavy carousel, 16:9 acceptable | `deck` | `simple-deck` | Bulk slide generation in one project. Use only when the operator confirms 16:9 is acceptable. |
| Deck / PPT long-form presentation | `deck` | `magazine-web-ppt` | OD's bundled deck default. PPTX/PDF export is first-class; swap to `pptx` if PPTX editability matters more than visual style. Deck path selection across all tools lives in [`deck-production.md`](deck-production.md). |

## Record convention

Prompts and their iteration trail are owned by the post's `image-prompts-v{N}.md` — shape, constraints, and lock criteria in [`library/deliverables/image-prompts/template.md`](../deliverables/image-prompts/template.md). Generated files land in the post's `images/` or `visuals/` folder beside it. Gemini API docs own prompt best practices; the design language owns the reusable treatment block the prompts build on.
