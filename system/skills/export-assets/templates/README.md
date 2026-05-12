# Master Templates

Master `.docx` / `.pptx` files used as the design baseline for exported deliverables. The agent reads from this folder when rendering an export and applies brand values from [`../config.yaml`](../config.yaml).

## Files

| File | Used for |
|---|---|
| `cover.html` | **Cover page (page 1) for any .docx export.** Rendered via local Chrome headless to PNG, embedded full-page in the .docx. Two variants emerge from one template based on whether the campaign DL provides a `light_variant` block with display font + motif. See [`../../../../library/context/operator/design-language.md#cover-html--png-pipeline`](../../../../library/context/operator/design-language.md). |
| `business-brief.docx` | Word export of `library/deliverables/business-brief/` (body styles only — cover comes from `cover.html`) |
| `business-brief.pptx` | PowerPoint export of `library/deliverables/business-brief/` (consumed via PPT-MD intermediate) |
| `campaign-brief.docx` | Word export of `library/deliverables/campaign-brief/` (body styles only — cover comes from `cover.html`) |
| `campaign-brief.pptx` | PowerPoint export of `library/deliverables/campaign-brief/` |

## Resolution Order

The agent looks for templates in this order — first match wins:

1. `workspace/campaigns/{slug}/exports/templates/{type}.{ext}` (per-campaign override; rare)
2. `system/skills/export-assets/templates/{type}.{ext}` (system master; the default)

Per-campaign overrides exist so a campaign for a specific stakeholder can use *their* brand template without polluting the system masters. Drop a `.docx` / `.pptx` in the campaign's `exports/templates/` folder and the next export uses it automatically.

## Update Path

Two ways to evolve a template:

**(a) Edit in Word/PowerPoint directly.** Open the file, edit, save. The next export uses the new template. This is the right path for visual polish (colors, font choices, layout tweaks).

**(b) Re-vendor from a good example.** The agent can read an example `.docx`/`.pptx` you received from a colleague, strip its content, keep its styles, and save the result as a new master. Ask: "use this docx as the new business-brief template" — the agent will write inline `python-docx` code (using the vendored docx skill) to do the extraction.

Either way: if you change the template structure (add/remove named slide layouts, rename heading styles), update [`../config.yaml`](../config.yaml) and [`../../pptx/pptx-md.md`](../../pptx/pptx-md.md) to match. Those files are the contract the agent reads.

## Caveat: Seed Quality

The four files in this folder are **seed templates** — programmatically generated to be functional, NOT polished. They include:

- Cover with title/author/date placeholders
- Heading 1 / Heading 2 / Body styles using brand fonts + accent color
- Footer with deliverable name + page number
- Six PPT slides demonstrating each layout in the catalog (cover, section_divider, content, two_column, callout_quote, closing) — but as positioned shapes, NOT as named slide-master layouts (python-pptx makes that hard)

Phase C.5 (a future creative session) will rebuild the masters in PowerPoint itself, where named layouts are first-class and visual design is tractable. Until then: the exports work, they look corporate-functional, and they carry the brand palette correctly.

If you open a seed template and it looks bare, that's expected. It's a foundation, not a finished design.
