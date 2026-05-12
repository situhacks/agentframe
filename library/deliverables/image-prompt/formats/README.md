# Image-Prompt Formats

Reusable composition blueprints for image generation. Each `{name}.md` here is a partial prompt skeleton — the structural pieces that do not change per campaign — that per-post `image-prompt-vF.md` records hydrate with campaign-specific subject, mood, and palette tokens.

This directory ships **empty by design**. Formats are not pre-fabricated; they're extracted from real campaign artifacts when a generated image's composition turns out to be reusable.

## When to add a format

Add one when:

- A composition pattern (subject placement, background treatment, negative-space convention) clearly transfers across campaigns
- The campaign-specific bits (subject, palette, mood) are cleanly separable from the structural bits (aspect, framing, type-in-image policy)
- You'd plausibly call this format by name on a future post ("use the billboard format")

Don't add one for:

- A one-off shot tied to a specific subject/person/product
- Compositions where palette or subject IS the structure
- Anything you'd struggle to write a clear "when to use" line for

## Format-file shape

Every format follows this shape. Keep it tight — formats are read by the agent at prompt-assembly time, so verbosity is cost.

```markdown
# Format: {Name}

## Aspect / dimensions
e.g. 3:1 horizontal, 1920x640. Or "square 1:1" for cover slides.

## Composition
Subject placement, framing rules, focal points, negative-space conventions.
1-3 sentences. Be specific enough that the agent can hydrate without
inventing — vague composition rules produce vague prompts.

## Background treatment
Solid color block / texture / gradient / scene. Reference palette by token
name (`bg_primary`, `accent_primary`) so the format works across campaigns
that have different `tokens.yaml` values.

## Type / text in image
Almost always: "none — HTML overlay handles copy." Call out the rare
exception (e.g. a giant numeral baked into the image as a graphic element).

## Standard negative prompt additions
Format-specific exclusions. e.g. for a billboard:
- "no suburban roadside billboard framing, no sky background"
- "no real-world billboard mockup with depth/shadow — flat composition only"

## When to use
1-3 bullets describing the slot this format fills. Concrete enough that
the agent can pattern-match a per-post slot to this format.

## Per-campaign slot-fillers
What the per-post `image-prompt-vF.md` must supply when referencing this format:
- Subject (required)
- Mood, 1-2 adjectives (required)
- Palette tokens by name (required)
- Variants count (optional, defaults to 3)
- Any format-specific extras

## Provenance
First saved from campaign `{slug}` post-{n}, generated {date}. Worked because:
{one line on what made the composition succeed — this is the durable insight,
not just attribution}.
```

## How the agent uses formats

1. `library/process/image-production.md` selects the image path and record convention.
2. Per-post `image-prompt-vF.md` declares `format: {name}` plus campaign-specific slot-fillers.
3. Agent reads the format file + the per-post params + `tokens.yaml` + `design-language-vF.md`.
4. Agent assembles the full prompt, surfaces it to the user for verification (with cost estimate).
5. Agent calls `python -m system.server.lib.image_generate --out-dir {post_visuals_dir} --variants 3`.
6. After generation, if the composition reveals reusable structure not yet captured, agent recommends saving as a new format.

If no existing format fits, agent writes the prompt freehand from the design language and the deliverable template's hard constraints. That's a normal mode, not an exception — formats are a productivity layer, not a gate.

## Naming conventions

- Lowercase, kebab-case: `billboard.md`, `ig-cover.md`, `podcast-card.md`, `linkedin-banner.md`, `hero-still.md`
- Name describes the slot, not the visual style (visual style varies per campaign's design language; the slot is what's stable)
- Avoid generic names like `image.md` or `default.md` — every format needs a clear "when to use"
