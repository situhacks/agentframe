# Template: Design Language

A campaign's locked visual language. One per campaign. Record what, not why unless the rule has historical breakage behind it.

## Required Frontmatter

```yaml
status: <drafting | locked>
last_updated: <ISO-8601>
preview: preview/directions-compare.html
tokens: tokens.yaml
category: <short OD-style category, optional — e.g. "Editorial & Print", "AI & LLM">
summary: <one-line picker-style summary>
light_variant:
  accent_hue: "<hex>"
  cover_font_family: "<optional CSS family>"
  cover_motif_svg: |
    <optional inline SVG>
dark_variant:           # optional — populate when a dark variant is in scope
  accent_hue: "<hex>"
  background: "<hex>"
```

`light_variant` (and `dark_variant` when present) lets document and post renderers borrow campaign visual signal without parsing full DL prose. `category` and `summary` are optional but make Open Design transfer cleaner — see [`transfer-to-open-design.md`](transfer-to-open-design.md).

## Authoring

Divergence-first. When this deliverable opens, offer the starting points — don't assume one:

1. **Offer the on-ramps:** (a) agent ideates directions from campaign context, (b) operator drops inspo references — when a reference is a live-site URL, run the token extraction on it (`system/skills/extract-design/`, rules in its `AGENTS.md`) and distill; images stay the eyeball path, (c) optional Gemini Deep Research style pass — keep the DR prompt agnostic: visual-trend research any campaign could use, not this campaign's narrative baked in.
2. **Propose 3–5 named taste directions**, each as a STANDALONE FULL PROMPT — copy-paste ready for any generator. Pick the generation path with the operator per [`image-production.md`](../../process/image-production.md); record the campaign-wide preference in `campaign.md` `post_manifest`.
3. **Render and narrow.** Render the directions on the chosen path (side-by-side `preview/directions-compare.html` for the HTML path — one file, no per-direction subfiles), then keep offering variations until the operator picks. Never one-shot the lock.
4. **Lock.** The picked direction becomes `design-language-v{N}.md` with its treatment block, plus `tokens.yaml`/`tokens.css` when slides will render as HTML.

The campaign-level base locks here; per-post evolution of the language is allowed when the campaign calls for it — version this deliverable, don't fork it.

Single-direction authoring is allowed only when the operator explicitly says "skip the directions, pick one" or chooses text-only defer. Do not invent the single-direction path silently.

No `decisions.md` companion. Reasoning that survives lock lives in `design-language-v{N}.md` itself; sub-session reasoning is throwaway.

## Artifact Shape

1. **Visual Theme & Atmosphere**
   - One short paragraph naming the mood, register, and use-cases.
   - One `# <Title>` line plus an optional `> Category:` line and one-line `summary` immediately after — keeps the artifact OD-transferable.
2. **Palette**
   - Token table: `token`, `hex`, `role`.
   - One-line accent rules directly under the table.
3. **Type System**
   - Face table: `face`, `role`, `weights`, `key sizes`.
   - One-line type rules directly under the table.
   - Font Labels block (parser-friendly for Open Design transfer):

     ```
     Display: <CSS family stack>
     Body: <CSS family stack>
     Mono: <CSS family stack>
     ```
4. **Layout & Composition**
   - Canvas size (e.g. `1080x1080`, `1200x1200`), safe margin, corner radius, grid hints.
   - Where the campaign's hero-image or carousel cover lives in the canvas.
5. **Voice & Brand**
   - One paragraph: tone of voice, brand register specific to this campaign, how visual moves and copy tone reinforce each other.
6. **Emphasis Devices**
   - Each device listed as: name, job, never-combine rule.
7. **Motif / Imagery**
   - Include only when earned; omit otherwise.
8. **Treatment block**
   - The paste-once prompt block downstream image work consumes: the full visual treatment (palette, light, materials, mood, composition grammar) as generator-ready prose. Stored once here; each post's `image-prompts-v{N}.md` copies it verbatim and adds per-slide deltas.
9. **Anti-patterns**
   - Campaign-specific anti-patterns (banned moves). System-wide bans live in `library/context/operator/design-language.md`.

Sections without earned content can stay short ("none for this campaign") rather than padded.

## Companion Artifacts

- `tokens.yaml` — machine-readable token export for render pipelines.
- `tokens.css` — CSS variables for browser preview and render. All tokens go inside a `:root {}` block (and `[data-theme="dark"]` block when a dark variant exists). Keeps the file drop-in-compatible with Open Design's parser.
- When a campaign renders carousel slides as HTML, this deliverable is the renderer's source: slides render per the Layout & Composition section and `tokens.css`, then screenshot to PNG for publishing.
- `preview/directions-compare.html` — side-by-side render of the proposed directions during authoring; the locked direction's preview lives in this same file with non-picked columns dimmed or removed.
- [`transfer-to-open-design.md`](transfer-to-open-design.md) — sibling child resource documenting the field-by-field mapping from this template into Open Design's 9-section design-system schema. Read on demand only when the operator wants to use the campaign DL inside Open Design.

## Not In This Template

- No lock procedure. Lock-event mechanics live in [`library/process/lock-event.md`](../../process/lock-event.md) and the active campaign flow.
- No review, humanizer, or publish/export sections.
- No `decisions.md` companion and no `hero_mock` field.
- No Components or Motion specs by default. Marketing campaigns rarely need full app-style component libraries; if a campaign earns those, they live in this template's artifact body or in `transfer-to-open-design.md` as overrides.
