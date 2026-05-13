# Image Production

Available image-creation paths for AgentFrame Marketing. Load this when a deliverable needs to pick an image path. Usage rules, lock criteria, and provenance details are owned by the deliverable that calls this menu.

Used by `library/deliverables/carousel-spec/template.md` and any post that needs `image-prompt-v{N}.md` records.

| Path | Use when | Owner deliverable | Outputs |
|---|---|---|---|
| HTML render + browser screenshot | Slide-shaped visuals from tokens/layout are enough (covers, diagrams, doc-style panels). | `carousel-spec` | `carousel-slide-{i}.html` and manual PNG screenshots |
| Gemini Nano Banana 2 (`gemini-3.1-flash-image-preview`) | Default raster generation for standard illustrations and fast A/B/C variants. | `image-prompt` | `image-variant-{a|b|c}.png` via `system/server/lib/image_generate.py` |
| Gemini Nano Banana Pro (`gemini-3-pro-image-preview`) | Prompt fidelity and text-in-image quality matter more than speed (hero or leadership-facing visuals). | `image-prompt` | `image-variant-{a|b|c}.png` via `system/server/lib/image_generate.py` |
| Open Design (bundled) | You want higher-fidelity designed outputs, or you want to render a whole social carousel as a single design pass. | `image-prompt`, `carousel-spec` | Exported files (HTML / PNG / PDF / PPTX / ZIP) saved to the calling post's `visuals/imports/`; use `system/skills/open-design/SKILL.md` |
| External / stock / operator-provided | Existing assets are already the right fit; no model generation needed. | calling deliverable | Referenced asset path(s) in the post folder |

## Record convention

When an image is generated for a post, save `phase-4-production/posts/post-{n}/image-prompt-v{N}.md` as the absolute-tiniest record:

```yaml
---
status: <drafting | locked>
last_updated: <ISO-8601 timestamp>
current_version: <integer>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason>"}
image_method: <gemini_nano_banana_2 | gemini_nano_banana_pro>
format: <format-name or omit>
variants: <count>
---
```

Body: the exact prompt text submitted to the model. Nothing else. Gemini API docs own prompt best practices; `design-language-v{N}.md`, `tokens.yaml`, and any `formats/{name}.md` own reusable composition constraints.
