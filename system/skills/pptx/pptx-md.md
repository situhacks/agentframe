# PPT-MD Format Specification

PPT-MD ("PowerPoint-flavored Markdown") is the intermediate format the agent and user iterate on in chat *before* rendering a `.pptx`. It bridges the gap between prose markdown (great for `.docx`, awkward for slides) and the structured slide-by-slide reality of PowerPoint.

**The flow**: prose `draft.md` -> agent translates to PPT-MD -> user iterates with agent in chat -> agent renders `.pptx` from final PPT-MD using `system/skills/pptx/` + `system/skills/export-assets/templates/{type}.pptx`.

PPT-MD is human-readable, agent-editable, and deterministically renderable. Don't render directly from prose markdown - pre-translation lets the user steer slide selection and content trimming before bytes hit disk.

---

## File Structure

A PPT-MD file is markdown with YAML frontmatter and slide blocks delimited by `---` separators (the same delimiter as YAML frontmatter - context disambiguates).

```markdown
---
template: business-brief.pptx
deliverable_type: business-brief
campaign: secondbrain
title: "SecondBrain — Business Brief"
author: "Brandon Situ"
date: "2026-04-18"
brand:
  accent: "#E45A2E"
---

::: slide layout=cover
title: SecondBrain
subtitle: Business Brief
footer: Brandon Situ · April 2026
:::

::: slide layout=section_divider
title: Opportunity
:::

::: slide layout=content
title: Why now
body: |
  Personal AI infrastructure is moving from "novelty" to "table stakes" for
  knowledge workers. Three signals in the last 30 days:

  - Anthropic ships Claude Memory beta
  - Cursor launches workspace-scoped agents
  - Notion AI hits 10M paid seats
notes: |
  Speaker note: open with the Anthropic + Cursor signals, lead with Notion's
  scale as the proof that this is mainstream.
:::

::: slide layout=two_column
title: Audience direction
left_title: Builder-curious professionals
left_body: |
  - Building side projects with AI tools
  - Treating personal AI infra as table stakes
  - Read Stratechery, listen to Lenny
right_title: Consultant-builders
right_body: |
  - Enterprise leaders shipping internal AI tools
  - Want to understand "personal AI infrastructure" in practice
  - Often DM after seeing build-in-public posts
:::

::: slide layout=callout_quote
quote: "I'm not building a note-taker. I'm building a passive dataset of myself."
attribution: Brandon Situ
:::

::: slide layout=closing
title: Decision needed
body: |
  Approve campaign brief by April 22 to publish Post 1 the week of April 28.
cta: Reply by Tuesday with your steer.
:::
```

---

## Frontmatter Fields

All fields required unless marked optional.

| Field | Type | Notes |
|---|---|---|
| `template` | string | Filename (not full path) of the master template inside `system/skills/export-assets/templates/` or campaign override folder. |
| `deliverable_type` | string | One of: `business-brief`, `campaign-brief`. Used for output-path resolution. |
| `campaign` | string | Campaign slug. Determines export destination. |
| `title` | string | Cover slide title. |
| `subtitle` | string (optional) | Cover slide subtitle. Defaults to deliverable type if absent. |
| `author` | string | Author name for cover + footer. Inherits from `system/skills/export-assets/config.yaml#default_author`. |
| `date` | ISO date string | Document date. |
| `brand` | object (optional) | Per-deck color overrides. Keys: `accent`, `text`, `bg`. Falls back to config.yaml palette. |

---

## Slide Block Syntax

Slide blocks use a fenced syntax: `::: slide layout=<name>` opening, `:::` closing.

```
::: slide layout=<layout_name>
<key>: <value>
<key>: |
  multi-line value
:::
```

- `layout` is required and must match one of the names in `pptx_layout_catalog` from `system/skills/export-assets/config.yaml`.
- All other fields are layout-specific (see below).
- Use YAML's `|` block-scalar syntax for multi-line content (preserves newlines).
- Bullet points are written as markdown bullets inside `body`/`left_body`/`right_body` block scalars.

---

## Layout Catalog

The agent populates these from the master template's named slide layouts. Adding or removing layouts requires updating BOTH this spec AND `system/skills/export-assets/config.yaml#pptx_layout_catalog` AND the master `.pptx` template.

### `cover`
Title slide. Always slide 1.

| Field | Required | Notes |
|---|---|---|
| `title` | yes | Big headline. |
| `subtitle` | no | Smaller subtitle below title. |
| `footer` | no | Author / date footer line. Defaults to `{author} · {date}` from frontmatter. |

### `section_divider`
Full-bleed section break. Use to separate major sections (Opportunity, Audience, etc.).

| Field | Required | Notes |
|---|---|---|
| `title` | yes | Section name. |

### `content`
Standard title + body slide. The workhorse.

| Field | Required | Notes |
|---|---|---|
| `title` | yes | Slide title. |
| `body` | yes | Body content. Markdown bullets, paragraphs, or short prose. Keep under ~6 lines for readability. |
| `notes` | no | Speaker notes (rendered into `.pptx` notes pane, not visible on slide). |

### `two_column`
Title + two-column body. For comparisons or paired ideas.

| Field | Required | Notes |
|---|---|---|
| `title` | yes | Slide title. |
| `left_title` | yes | Column 1 heading. |
| `left_body` | yes | Column 1 content. |
| `right_title` | yes | Column 2 heading. |
| `right_body` | yes | Column 2 content. |
| `notes` | no | Speaker notes. |

### `callout_quote`
Large pull-quote slide. Use sparingly - high-impact moments only.

| Field | Required | Notes |
|---|---|---|
| `quote` | yes | The quote text (no surrounding quote marks; template handles styling). |
| `attribution` | no | Speaker / source. |

### `closing`
Final slide. Decision needed, CTA, or next steps.

| Field | Required | Notes |
|---|---|---|
| `title` | yes | E.g. "Decision needed", "Next steps". |
| `body` | no | Context paragraph. |
| `cta` | yes | The single action requested of the audience. |

---

## Iteration Pattern (in chat)

1. User asks to export `.pptx`. Agent reads `draft.md`.
2. Agent drafts PPT-MD in chat (paste into a code block first - don't write to disk yet).
3. User reacts: "drop slide 4," "split slide 6 into two," "callout the Anthropic quote."
4. Agent revises PPT-MD, re-pastes.
5. Once user approves, agent writes to `workspace/campaigns/{slug}/phase-2-strategy/{type}/exports/{type}-v{N}.pptx-md` (the source of truth, kept alongside the rendered `.pptx`).
6. Agent renders `.pptx` from PPT-MD using `system/skills/pptx/` + master template.

---

## Why this exists

PowerPoint is structurally NOT prose. Auto-generating a deck from prose markdown gives you the equivalent of a 30-page Word doc spread across 8 cramped slides. PPT-MD forces explicit slide selection and per-slide content trimming - a deliberate conversation between agent and user.

The format is intentionally small. Six layouts, no animations, no embedded media, no transition effects. Polish the master template (Phase C.5) rather than expanding the format.

## Out of scope (POC)

- Animations / transitions
- Embedded images per slide (covered by post-production image-prompt flow, not exports)
- Charts / data viz (would need a separate `chart` layout - defer until needed)
- Custom per-slide colors (use brand palette from frontmatter; per-slide override would invite chaos)
- Multi-deck merging
