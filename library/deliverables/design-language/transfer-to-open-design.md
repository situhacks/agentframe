# Transfer To Open Design

Bridge resource for the design-language deliverable. Read on demand when the operator wants to take a campaign's locked design language into Open Design.

This file documents the field-by-field mapping from `design-language-vF.md` (plus its `tokens.yaml` and `tokens.css`) into Open Design's canonical 9-section design-system schema. The mapping is one-way (DL ? OD); Open Design changes do not flow back automatically.

## Open Design canonical schema

Per [`system/skills/open-design/source/docs/design-systems.md`](../../../system/skills/open-design/source/docs/design-systems.md):

Header format (in this order):

```markdown
# <Title>

> Category: <category>
> One-line summary for the picker preview.
```

Then nine numbered section headings:

```
## 1. Visual Theme & Atmosphere
## 2. Color
## 3. Typography
## 4. Spacing
## 5. Layout & Composition
## 6. Components
## 7. Motion & Interaction
## 8. Voice & Brand
## 9. Anti-patterns
```

OD's parser matches `## [0-9].` so the number prefix is required and the suffix may be extended (for example `## 4. Spacing & Grid`). Empty section bodies are acceptable, but the nine numbered headings must all be present.

OD also requires:

- All CSS variables wrapped in `:root {}` (or `[data-theme="dark"]` for dark overrides). No bare CSS variable declarations.
- Font Labels block in the Typography section, exactly:

  ```
  Display: <CSS family stack>
  Body: <CSS family stack>
  Mono: <CSS family stack>
  ```

- Real hex codes for all colors (no `#REPLACE_ME`, no `currentColor`, no CSS variable names as values).
- WCAG AA contrast for text against its paired background.
- `:focus-visible` styles on interactive components.
- `prefers-reduced-motion` targets specific elements, not a global `*` selector.

## Field-by-field mapping

| OD section | Source in our DL | Notes |
|---|---|---|
| H1 title | Campaign name (or DL "Visual Theme & Atmosphere" title) | Use the campaign's human-readable name, not the slug. |
| `> Category:` | DL frontmatter `category` | Pick from OD's category list (AI & LLM, Developer Tools, Productivity & SaaS, Backend & Data, Design & Creative, Fintech & Crypto, E-Commerce & Retail, Media & Consumer, Automotive, Editorial & Print, Retro & Nostalgic, Bold & Expressive, Modern & Minimal, Professional & Corporate). |
| Picker summary line | DL frontmatter `summary` | One sentence. |
| 1. Visual Theme & Atmosphere | DL "Visual Theme & Atmosphere" paragraph | Direct copy. |
| 2. Color | DL "Palette" table + `tokens.css` `:root {}` block | Convert palette table to OD format and ensure `tokens.css` has `:root {}` wrapper; copy variables directly. Add `[data-theme="dark"]` block when `dark_variant` is defined. |
| 3. Typography | DL "Type System" table + Font Labels block | The Font Labels block in the DL template is already OD-shaped — copy verbatim. |
| 4. Spacing | DL "Layout & Composition" — safe margin and corner radius | If DL specifies more spacing scale, lift those tokens; otherwise infer from `tokens.yaml` `safe_margin` and `corner_radius` and label as `--space-*` variables. |
| 5. Layout & Composition | DL "Layout & Composition" body | Direct copy of canvas size, grid hints, hero placement notes. |
| 6. Components | Skip by default | Marketing DL does not carry component specs. Populate this section in OD with a one-line "Campaign DL does not specify components; defer to Open Design defaults" or borrow from a starter system. Operator can override per campaign. |
| 7. Motion & Interaction | Skip by default | Marketing DL is static unless declared. If the campaign has motion (video, animated cover), promote those notes here. Otherwise: "Static unless declared per asset." |
| 8. Voice & Brand | DL "Voice & Brand" paragraph | Direct copy. |
| 9. Anti-patterns | DL "Anti-patterns" section | Direct copy. The DL section name is already aligned with OD. |

## Transfer procedure

When the operator says "take this DL into Open Design" (or similar):

1. Load `design-language-vF.md`, `tokens.yaml`, and `tokens.css` for the campaign.
2. Assemble a new `DESIGN.md` at `system/skills/open-design/source/.od/design-systems/<campaign-slug>/DESIGN.md` (or the OD project's local override path).
3. For each row in the mapping table, copy or transform the source field into the matching OD section.
4. For sections marked "Skip by default", emit a one-line note rather than fabricating content. Operator can override per campaign.
5. Verify the assembled file:
   - All 9 numbered headings present.
   - `:root {}` block contains the campaign's color, type, spacing tokens.
   - Font Labels block present in Typography.
   - Real hex codes everywhere, no placeholders.
6. Drop a comment at the top of the OD `DESIGN.md` linking back to the source `design-language-vF.md` so future Open Design edits can find the upstream.

## Out of scope (deferred to its own plan)

Runtime pass-through (auto-generate `DESIGN.md` on lock, prepare the first OD prompt at launch, pre-populate the OD design-system picker with the campaign DL, OD source-side changes) is a separate plan. This file is the bridge contract; that plan is the runtime that uses it.
