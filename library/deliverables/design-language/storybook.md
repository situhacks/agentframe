# Companion: Design Storybook

An **optional** single-file HTML catalog of a project's design language — a browsable visual reference rendered from the ready language. Offer it when the operator wants to *visualize* the language or *compare* candidate directions; skip it for rudimentary languages that don't earn the generation cost.

One storybook per design language. It is a companion to `design-language-v{N}.md`, not a replacement — the `.md` is the source of truth; the storybook is the visual view of it.

**Not versioned.** The storybook is a *rendered view*, like `tokens.css` — a single `storybook.html` that is regenerated when the language changes, always reflecting the current (highest) `design-language-v{N}.md`. The version history already lives in the language's `v{N}` chain; do not create `storybook-v{N}.html`. Static filename, dynamically current content. It is the Design group's default/current file in the preview server.

## When It Exists

- The operator asks to see the design language rendered, not just read it.
- During direction selection, to compare candidates richly (an upgrade of `preview/directions-compare.html`).
- As the pinned reference for a ready language (surfaces at the top of the preview server's Design section).

A language with no storybook is normal and fully valid. Absence is never an error.

## File Shape (hard rules)

- **One self-contained `.html` file** at `preview/storybook.html`, beside the design language.
- **Links `tokens.css` relatively** — `<link rel="stylesheet" href="assets/tokens.css">`. Never inline token values. This keeps token edits reflecting live under the preview server and lets the file open standalone via `file://`.
- **Google Fonts import allowed** (matches existing preview HTML).
- Renders correctly both standalone (`file://`) and served.
- No build step, no external JS dependencies beyond fonts.

## Canonical Sections (in order)

Sections with no earned content collapse to a short "none for this project" note rather than padding — except a storybook always has Cover, Palette, and Type.

1. **Cover / Identity** — title, `Category:` line, one-line summary, mood paragraph. The mood paragraph comes from the design-language `.md` body.
2. **Palette** — swatch grid. Each swatch: color block, token name, hex, role. Accent/usage rules listed under the grid.
3. **Type** — a live specimen per `type` role: the actual font rendered, family + weights labeled, at the role's key sizes.
4. **Emphasis Devices** — *rendered* examples of each device (real highlighter swipes, brackets, annotations — not descriptions of them). Each carries its name and its never-combine rule.
5. **Composition** — canvas dimensions, a safe-zone diagram drawn as CSS boxes, composition rules.
6. **Sample Gallery** — embedded real artifacts via relative `<iframe>`/`<img>` (slides, carousel frames, hero images) when they exist. Sources come from the design-language frontmatter `storybook_samples:` list. No list → section omitted cleanly.
7. **Anti-patterns** — project-specific banned moves.

## Generation Paths

Both paths satisfy this same standard, so they are interchangeable:

- **Script (default, low-cost):** `python -m system.server.lib.storybook <design-language-folder>`. Deterministic render from `tokens.yaml` + heading-extracted prose from the `.md`. Fails loudly on schema drift (named error, no partial file) — see the script's validation.
- **Agent-authored (fallback):** when the script can't run (schema the script doesn't cover, or the script is archived), author the HTML directly against this standard. Same file shape, same sections.

## Tokens Schema It Reads

The script and any parser read the canonical `tokens.yaml` schema documented in `system/server/lib/tokens_to_css.py`. `type` roles are project-defined keys (e.g. `display/body/mono` or `primary/annotation/mono`), iterated generically — there is no fixed role enum. Project-specific token sections the standard doesn't name (e.g. `highlighter`, `nano-banana`) are passed over gracefully, not errored.

## Not In This Companion

- No separate readiness procedure—the storybook follows its language's lifecycle.
- No component library or motion specs (design languages rarely need them; out unless a project earns it, in which case it lives in the language `.md`).
- No provenance/changelog prose — the `v{N}` chain is the changelog.
