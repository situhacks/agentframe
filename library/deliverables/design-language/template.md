# Template: Design Language

A project's ready visual language. One per project; a language worth reusing graduates to a shared asset (see Reuse Across Projects). Record what, not why, unless historical breakage matters.

## Required Frontmatter

```yaml
status: <drafting | ready>
last_updated: <ISO-8601>
preview: preview/directions-compare.html
tokens: tokens.yaml
storybook: preview/storybook.html    # optional — present only when a storybook exists
storybook_samples: []                # optional — relative paths to real artifacts embedded in the storybook gallery
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

`light_variant` (and `dark_variant` when present) lets document, deck, and page renderers borrow visual signal without parsing the full prose. `category` and `summary` are optional but make Open Design transfer cleaner — see [`transfer-to-open-design.md`](transfer-to-open-design.md).

## Authoring

First check `library/assets/design-languages/` for a shared language that fits. Adopting one beats authoring a fresh identity, because a shared language carries a replayable package that reproduces across contexts; record the adoption in `project.md` notes and skip to the treatment block. Author new only when none fits or the operator wants a distinct identity.

Divergence-first. Offer the starting points rather than assuming one:

1. **Offer the on-ramps:** (a) agent ideates from project context, (b) operator drops inspo references — a live-site URL goes through token extraction (`system/skills/extract-design/`, rules in its `AGENTS.md`), images stay the eyeball path, (c) optional Deep Research style pass, its prompt kept agnostic: visual-trend research any project could use, not this project's narrative baked in.
2. **Propose 3–5 named taste directions**, each a STANDALONE FULL PROMPT, copy-paste ready for any generator. Pick the generation path with the operator per [`image-production.md`](../../process/image-production.md) and record the project-wide preference in the active pack's settings when it declares any.
3. **Render and narrow.** Render on the chosen path (one side-by-side `preview/directions-compare.html` for HTML, no per-direction subfiles), then keep offering variations until the operator picks. Never one-shot the decision.
4. **Ready.** The picked direction becomes `design-language-v{N}.md` with its treatment block, plus `tokens.yaml`/`tokens.css` when surfaces will render as HTML.

The project-level base becomes ready here; per-deliverable evolution versions this deliverable rather than forking it.

**Storybook (optional):** offer one when the operator wants to *visualize* the language or *compare* directions; it costs generation time, so skip it for rudimentary languages. Shape and generation paths live in [`storybook.md`](storybook.md). When one exists, set the `storybook:` field so the preview server pins it atop the Design section.

Single-direction authoring needs an explicit operator call ("skip the directions, pick one") or a text-only defer. Never take that path silently.

Reasoning that must survive readiness lives in `design-language-v{N}.md` itself; sub-session reasoning is throwaway. Readiness mechanics belong to [`library/process/ready-event.md`](../../process/ready-event.md) and the active flow.

## Reuse Across Projects

This deliverable is the project's own language and it stays here. When the same identity will serve a later project, it graduates to a shared asset at `library/assets/design-languages/<name>/` (schema: [`library/assets/README.md`](../../assets/README.md)), and this file remains the record of how the language was set here.

A shared language is a replayable package plus its imagery manifest, never a copy of this prose. Prose hands the next agent a description and lets it re-derive the layouts, which is how a language returns as the right colours in the wrong shapes. Graduate as soon as real artifacts prove it out, not on the second project that needs it. A consuming project names it in `project.md` notes; [`library/process/deck-production.md`](../../process/deck-production.md) owns the load for deck work.

## Artifact Shape

1. **Visual Theme & Atmosphere** — one short paragraph on mood, register, and use-cases, under a `# <Title>` line with an optional `> Category:` and one-line `summary` right after, which keeps the artifact OD-transferable.
2. **Palette** — token table (`token`, `hex`, `role`) with one-line accent rules directly under it.
3. **Type System** — face table (`face`, `role`, `weights`, `key sizes`) with one-line type rules, then a Font Labels block for Open Design's parser:

   ```
   Display: <CSS family stack>
   Body: <CSS family stack>
   Mono: <CSS family stack>
   ```
4. **Layout & Composition** — canvas size (e.g. `1080x1080`), safe margin, corner radius, grid hints, and where the hero or cover visual sits.
5. **Voice & Brand** — one paragraph: tone of voice, this project's brand register, how visual moves and copy tone reinforce each other.
6. **Emphasis Devices** — each as name, job, never-combine rule.
7. **Motif / Imagery** — only when earned; omit otherwise.
8. **Treatment block** — the paste-once prompt block downstream image work consumes: the full visual treatment (palette, light, materials, mood, composition grammar) as generator-ready prose. Each `image-prompts-v{N}.md` copies it verbatim and adds per-image deltas.
9. **Anti-patterns** — project-specific banned moves. System-wide bans live in `library/context/operator/design-language.md`.

Sections without earned content can stay short ("none for this project") rather than padded. No Components or Motion specs by default; a project that earns them puts them in the artifact body or in `transfer-to-open-design.md` as overrides.

## Companion Artifacts

- `tokens.yaml` — machine-readable token export for render pipelines.
- `tokens.css` — CSS variables for browser preview and render. All tokens sit inside `:root {}` (plus `[data-theme="dark"]` when a dark variant exists), which keeps the file drop-in-compatible with Open Design's parser.
- When a project renders slides or pages as HTML, this deliverable is the renderer's source: render per Layout & Composition plus `tokens.css`, then screenshot to PNG for delivery.
- `preview/directions-compare.html` — side-by-side render of the proposed directions; the ready direction's preview lives in this same file with non-picked columns dimmed or removed.
- `preview/storybook.html` — optional browsable catalog, shape in [`storybook.md`](storybook.md). Present only when generated.
- [`transfer-to-open-design.md`](transfer-to-open-design.md) — field-by-field mapping into Open Design's 9-section design-system schema. Read on demand, only when the operator wants the project language inside Open Design.
