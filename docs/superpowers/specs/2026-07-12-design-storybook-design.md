# Design Storybook — Design Spec

**Date:** 2026-07-12
**Mode:** Builder
**Status:** Draft for operator review

## Problem

A project's design language currently lives as `design-language-v{N}.md` (human-readable), `tokens.yaml` (machine-readable), and `tokens.css` (auto-derived). None of these is a *browsable visual catalog*. The operator wants an optional, richly-rendered HTML "storybook" — a live catalog of the design language (identity, palette swatches with usage rules, type specimens, emphasis devices, composition rules, sample artifacts, anti-patterns) that:

1. Helps **compare candidate directions** during design-language selection (an upgrade of the current `directions-compare.html` pattern).
2. Serves as the **pinned, top-of-preview-server reference** for the locked language once chosen.

It is **optional** — only generated when the operator wants to visualize or compare, since it costs generation time. Simple projects with a rudimentary language don't need one.

The storybook is a **standalone HTML file** (opens via `file://` or through the preview server) that lives beside the design language and consumes `tokens.css` relatively, so token edits reflect live.

## What tokens.yaml is (resolved)

`tokens.yaml` is **part of the design language, not a separate file to maintain**. One language, three representations:

- `design-language-v{N}.md` — human-readable WHAT (+ surviving WHY inline; no `decisions.md`).
- `tokens.yaml` — machine-readable export of the same tokens.
- `tokens.css` — auto-derived from `tokens.yaml` via `tokens_to_css.py`.

The storybook consuming `tokens.css` *is* consuming the design language.

## Schema reality (load-bearing finding)

`system/server/lib/tokens_to_css.py` documents a schema (`typography.{role}`, `spacing.scale`, `layout.*`, `grid.*`) that **matches zero real `tokens.yaml` files**. Both real projects converged independently on a different shape:

| Section | POV (`agent-architecture-pov`) | Completed (`enterprise-ai-adoption`) |
|---|---|---|
| Palette | `palette.{key}` (flat hex) | `palette.{key}` (flat hex) + `palette_roles` |
| Type | `type.faces.{primary,annotation,mono}` + `type.scale` | `type.{display,body,mono}` |
| Canvas/layout | `composition.canvas.*` | `canvas.*` (flat) |
| Emphasis | `emphasis.*` | `emphasis.*` |
| Anti-patterns | (in `.md` body) | `anti_patterns` (in yaml) |

**Decision:** The canonical `tokens.yaml` schema is what the projects actually use, not what the stale script documents. This spec defines one canonical schema (below), conforms POV to it, and updates `tokens_to_css.py` to match. The completed project is left as-is (it is closed; not a render target), but its shape is honored as evidence for the canonical decision.

### Canonical tokens.yaml schema (v1)

```yaml
meta:
  campaign: <slug>
  version: <N>              # integer, matches design-language-v{N}.md
  category: <optional OD-style category>
  summary: <one-line picker summary>
  reference: <optional real-world reference note>

palette:
  <token>: "<hex>"          # flat token -> hex map

palette_roles:              # optional; semantic names -> palette token keys
  background: <token>
  foreground: <token>
  primary_accent: <token>
  # ...

type:
  <role>:                   # roles are project-defined (display/body/mono, or primary/annotation/mono)
    family: "<CSS family stack>"
    weights: [<int>, ...]
    role: <optional prose role>
    sizes:                  # optional named sizes
      <name>: <px int or "NNpx">
    line_height: <number>   # optional

canvas:
  width: <int>
  height: <int>
  aspect: "<W:H>"
  safe_margin: <int|px>     # optional
  grid_columns: <int>       # optional
  grid_gutter: <int>        # optional

emphasis:                   # optional; project-specific, free-shape
  # ...

anti_patterns:              # optional list
  - <string>
```

Roles under `type` are **project-defined keys**, not a fixed enum — the script iterates whatever keys exist. This is the drift-tolerant seam: structured fields (palette, type families, canvas) are read generically; the script does not hard-code role names.

## Approach

Build **both** the template-layer standard AND the deterministic generator script (operator decision). If the script proves fragile in practice, it gets archived via a `BB-*` row and the agent-authored path (satisfying the same standard) takes over with zero standard changes.

### Component 1 — Storybook standard (`storybook.md` companion spec)

New file: `library/deliverables/design-language/storybook.md`. Defines the canonical single-file HTML shape. Both the script and any hand-authored storybook must satisfy it, so the fallback is seamless.

**Canonical section order (7):**

1. **Cover / Identity** — title, `> Category:` line, one-line summary, mood paragraph.
2. **Palette** — swatch grid: each swatch shows the color block, token name, hex, and role. Accent/usage rules listed under the grid.
3. **Type** — live specimens per `type` role (the actual font rendered at key sizes), family + weights labeled.
4. **Emphasis devices** — *rendered* examples (real highlighter swipes, brackets, annotations), each with its name and never-combine rule.
5. **Composition** — canvas dims, a safe-zone diagram drawn as CSS boxes, composition rules.
6. **Sample gallery** — embedded real artifacts (relative `<iframe>`/`<img>` to slides, carousel frames, hero images) when they exist; omitted cleanly when none.
7. **Anti-patterns** — project-specific banned moves.

**Hard rules:**

- Single self-contained `.html` file at `preview/storybook.html` beside the design language.
- Links `tokens.css` **relatively** (`<link rel="stylesheet" href="assets/tokens.css">`) — never inlines tokens, so edits reflect live under the server and the file still opens standalone.
- Google Fonts import allowed (matches existing preview HTML).
- Sample gallery sources come from an optional frontmatter list; absent list -> section omitted, not errored.

**Template.md hooks (two small edits to `library/deliverables/design-language/template.md`):**

- Authoring note: storybook is optional; offer it when the operator wants to visualize or compare directions; costs generation time.
- Frontmatter field: `storybook: preview/storybook.html` (present only when one exists) and optional `storybook_samples: [<relative paths>]`.

### Component 2 — Generator script (`system/server/lib/storybook.py`)

Sibling of `tokens_to_css.py`, same invocation ergonomics.

```
python -m system.server.lib.storybook path/to/design-language/  \
    [--out path/to/preview/storybook.html]
```

**Inputs (from the design-language folder):**

- `tokens.yaml` — canonical schema above; drives palette, type specimens, canvas/composition, emphasis, anti-patterns (when in yaml).
- `design-language-v{N}.md` — extracts **mood paragraph** and **anti-patterns** (when not in yaml) by heading. Picks the highest `v{N}`.
- Frontmatter `storybook_samples:` — sample gallery sources.

**Output:** `preview/storybook.html` per the standard. Default out path derived like `tokens_to_css.py` (beside the yaml).

**Schema validation up front (the anti-drift guard):**

- Validate `tokens.yaml` has the required top-level keys (`palette`, `type`, `canvas`) and that `type` roles carry `family`.
- Validate the `v{N}` markdown has the expected headings the script extracts from (mood, anti-patterns).
- On any failure: **exit non-zero with a named error** naming the missing key/heading and instructing the agent to fix the schema or hand-author the storybook per `storybook.md`. Never emit a half-rendered file.

**Determinism boundary (stated honestly):** everything from `tokens.yaml` is fully deterministic. The two prose sections (mood, anti-patterns) are heading-extracted from the markdown — this is the drift-sensitive seam, guarded by the up-front heading validation above.

### Component 3 — `tokens_to_css.py` update

Update its schema mapping and module docstring to the canonical schema (`type.{role}.family` -> `--font-{role}`, `type.{role}.weights`, `canvas.*` -> `--canvas-*`, etc.), replacing the stale `typography`/`spacing`/`layout`/`grid` mapping. Regenerate POV's `tokens.css` from the conformed `tokens.yaml`. Add/adjust unit tests under `system/server/tests/` to lock the canonical mapping.

### Component 4 — Preview server surfacing

Minimal change to the Preview tab (not the dashboard, not a new route, no server-side md->HTML transform):

- When the active project's design-language frontmatter names a `storybook:` file that **exists**, pin a **Design** entry at the **top** of the Preview rail.
- `media` filter: Design entry shows only the storybook HTML.
- `all` filter: Design group expands to the whole `design-language/` folder (`.md`, `tokens.yaml`, `tokens.css`, `storybook.html`, `preview/` subfiles).
- Clicking opens the storybook in the existing HTML viewer. No new viewer.

Implementation seam: `lib/surface/artifacts.py` gains a design-language detection that emits a pinned group; `static/surface/preview.js` renders it at rail top. Scoped so a project **without** a storybook shows no Design pin (backward compatible).

### Component 5 — POV migration (trial + schema conformance)

Bring `workspace/projects/agent-architecture-pov/phase-3-planning/design-language/` to current standard:

1. Rename `design-language-vF.md` -> `design-language-v1.md`.
2. Conform frontmatter to current `template.md` (drop `hero_mock`, `decisions`, `current_version`, `version_history`; add `summary`, `light_variant` with `accent_hue: "#87c7c0"` = `accent-teal`, the system/right-thing accent; keep `status`, `tokens`, `preview`). Add `storybook: preview/storybook.html`.
3. Fold any still-true reasoning from `decisions.md` into the `v1` body; **delete `decisions.md`**.
4. Strip changelog/tombstone/provenance prose from the body (per current template: "record what, not why unless the rule has historical breakage behind it"). The v1->v2->v3 chain is the changelog; material events go to `activity.md`.
5. Conform `tokens.yaml` to the canonical schema (fold `type.faces` -> `type.{primary,annotation,mono}` role map; `composition.canvas` -> `canvas`; keep `emphasis`, `highlighter`, `nano-banana` as project extensions the script ignores gracefully).
6. Regenerate `tokens.css` via updated `tokens_to_css.py`.
7. Run `storybook.py` to produce the first real `preview/storybook.html`.
8. Verify under the preview server: Design pin at rail top, storybook renders palette/type/emphasis/composition correctly.

**Note:** these are project-file touches, in-bounds as a schema migration (Builder owns schema/migration jobs per the operating index). The implementation plan flags them so the commit is clearly a migration, with a `MIGRATION:` line.

## Out of scope

- Global/operator-wide design system (per-project only for now).
- Server-side deterministic md->HTML transformation as a live route (rejected: drift risk; script is offline/agent-invoked).
- Migrating the completed `enterprise-ai-adoption` project (closed; not a render target).
- Component libraries / motion specs in the storybook (design languages rarely need them; out unless a project earns it).
- Dashboard or Calendar changes.

## Success criteria

1. `storybook.md` standard exists and defines the 7-section single-file shape with hard rules.
2. `storybook.py` generates a valid storybook from a conformant folder and **fails loudly** (named error, non-zero exit, no output file) on schema drift.
3. `tokens_to_css.py` reads the canonical schema; tests lock it.
4. Preview server pins a Design entry at rail top when a storybook exists; `media` shows only the HTML, `all` shows the folder; projects without a storybook are unaffected.
5. POV design language is migrated to current schema, `decisions.md` deleted, tombstone prose stripped, and a real `storybook.html` renders correctly under the server.
6. System change logged in `system/audit/agentframe.db`; commit carries a `MIGRATION:` line.

## Open questions

None blocking. Canonical `type` role naming (`display/body/mono` vs project-defined) is resolved as **project-defined keys, iterated generically** — no enum.
