# Assets

Reusable assets shared across every surface. Four shelves, different weights: `logos/`, `design-languages/`, the operator's own `media/`, and third-party `audio/`. Instances on the last three are gitignored; the schemas here travel.

## `logos/`

Flat brand marks. The filename is the inventory - no metadata files, no per-logo notes.

- Naming: `<brand>-<variant>.<ext>`, lowercase kebab. Variants: `white`, `black`, `color`, `icon`, `wordmark`.
  Examples: `acme-white.svg`, `northwind-color.png`, `contoso-wordmark.svg`.
- Prefer SVG; recolorable monochrome marks are the most reusable.
- A logo lands here whenever a run fetches a credible official/open-source asset and reuse is likely, or the second time a project needs it.

Sourcing order for a run that needs a logo: check `logos/` -> find the official brand / design / press kit -> official website, app, package, or repo assets -> open-source logo collections -> ask the operator only as a last resort. Open sources: [Simple Icons](https://simpleicons.org) (monochrome brand marks, ideal white/black), [gilbarbara/logos](https://github.com/gilbarbara/logos) (full-color), Wikimedia Commons (official wordmarks).

Never hand-draw, trace, approximate, or recreate a company logo as custom SVG. If no credible source exists, use a text label or ask the operator; do not ship a scruffy imitation.

## `design-languages/<name>/`

A named visual identity that reproduces in a **fresh context**, on a project that did not create it. A prose description of a look is not one of these: the replayable half is a ppt-master template package whose SVG prototype roster the vendor pipeline replays natively, so a new deck inherits the actual archetypes and only its content and imagery change.

`<name>` is lowercase kebab and stable once projects reference it, naming the identity rather than the client or its author: `editorial-dark-keynote`, `product-led-mono`.

### Required components

```text
library/assets/design-languages/<name>/
├── README.md            # provenance, reuse notes, what a new project swaps
├── package/             # ppt-master workspace root — the replayable identity
│   ├── templates/       #   design_spec.md (kind: deck) + the SVG prototype roster
│   ├── images/          #   bitmaps the roster references as ../images/<name>
│   └── icons/imported/  #   vectors the roster references as data-icon="imported/<name>"
├── imagery/
│   ├── manifest.yaml    # one record per curated asset (schema below)
│   └── <assets>         # the curated pool projects draw from
├── tokens.css           # optional — only when HTML surfaces render this language
└── video/               # optional — only when the language dresses video (schema below)
```

`package/` is a vendor-shaped workspace root and AgentFrame never hand-edits inside it. Everything a re-run of `create-template` could overwrite stays in there; everything AgentFrame owns stays outside it. `exports/` is review evidence and is never promoted.

Ownership split, so palette and type have exactly one source of truth:

| Question | Owner |
|---|---|
| Palette, typography, canvas, page roster, image-placement rules, density | `package/templates/design_spec.md` (vendor schema) |
| The clone-able archetypes themselves | `package/templates/*.svg` |
| Asset licence, provenance, theme, role, usage restriction | `imagery/manifest.yaml` |
| Where this identity came from, which projects ran it, what to swap | `README.md` |

`README.md` never restates palette or type values. When it disagrees with `design_spec.md`, the spec wins.

### Reference-grade — a language without a package

A language may land here package-less when it arrives from another AgentFrame instance or predates the current vendor contract. This is a documented waypoint, not a second asset class, and never the target state.

It qualifies only when the archetypes exist as real ppt-master prototypes with clone-able structure and coordinates, and a roster index mapping each one to its archetype. A structured roster declares its Master and Layout keys with PowerPoint picker names and omits `data-pptx-page-role`; that marker belongs to flat free-design pages, so its presence on a deck prototype is a legacy tell rather than a credential. Prose plus a palette does not qualify and does not belong in this directory at all.

Such a language still carries `README.md` and `imagery/manifest.yaml`. Its README declares reference-grade status in its own first section, names what blocks promotion, and cites the `BB-*` row tracking it. Consumers clone exemplars by hand; [`deck-production.md`](../process/deck-production.md) routes around the preflight on that declaration. Promote by running Capture below with the roster as the reference — the missing `design_spec.md` is authored by the vendor's Template_Designer, never by hand.

### `video/` — the frame-scale half of a language (optional)

A language that dresses video carries a second replayable half, because a deck package cannot tell a render engine how a caption or a punch-in should look. Present only when a channel names the language.

```text
video/
├── frame.md            # HyperFrames design spec: normative frontmatter tokens + composition prose
├── caption-skin.html   # the caption treatment
├── captions.yaml       # identity: <embedded-captions identity | ass-burn>, rail/embed policy, font, colours
├── audio.yaml          # sfx palette and beds, each pointing at a library/assets/audio/ record
└── lut.cube            # optional colour grade
```

`frame.md` follows the vendored HyperFrames design-spec contract (`system/skills/hyperframes/source/skills/hyperframes-creative/references/design-spec.md`): frontmatter is the normative layer, prose is context. Author it from the operator's references, never by adopting a frame preset wholesale, and state the anti-tells explicitly: radii, density, type, what never appears. `af doctor` notes a `video/` folder missing `frame.md` or `captions.yaml`.

### Capture — how one gets created

A design language is earned, not declared: capture it after an identity has proved out on a real deck, using that deck's own SVGs as the reference. Both halves matter, and the roster is the half that makes a fresh context reproduce anything.

1. Initialize a throwaway ppt-master capture project (never inside `system/`, never the deck project itself, whose `templates/` is already occupied).
2. Run the vendor's Create Template route, which dispatches Create Deck for a branded structural system, with the proven deck's SVGs as the reference and output scope `project`.
3. Validate the workspace from its root: `python system/skills/ppt-master/scripts/svg_quality_checker.py "<root>/templates" --template-mode`.
4. Promote `templates/`, `images/`, and `icons/` into `design-languages/<name>/package/`. Leave `exports/` behind.
5. Write `README.md` and `imagery/manifest.yaml`.
6. Re-run the validator against the promoted `package/templates/` so the copy is proven, not assumed.

Choose Create Deck when the identity carries brand and structure together, which is the normal case here. Create Brand (identity only, no roster) cannot reproduce archetypes and is not sufficient on its own.

### Consumption

[`library/process/deck-production.md`](../process/deck-production.md) owns the handoff. It resolves the named language, preflights the package, and supplies `package/` as an exact workspace root so the vendor's Stage 1 opens in template mode with the package preselected and installs it before authoring. Passing palette and typography alone reproduces the colours and loses the archetypes, which is the failure this asset class exists to fix.

### `imagery/manifest.yaml`

One record per curated asset. The human-readable index is optional and generated from this, never the reverse.

```yaml
assets:
  - path: glow/orb-dark-01.png       # relative to imagery/
    source: pexels                    # pexels | pixabay | getty | adobe-stock | operator | derived
    source_ref: "pexels.com/photo/123456"
    licence: pexels                   # pexels | pixabay | cc0 | stock-licensed | unknown
    attribution_required: false
    restriction: none                 # none | project-scoped | reference-only
    role: master                      # master | derived | reference-only
    derived_from: null                # path of the master when role is derived
    theme: [abstract, dark-ground, glow]
    slots: [hero, divider]            # archetype slots this asset suits
    dimensions: "2400x1350"
```

`af doctor` reports conformance drift across this shelf as notes: a missing `README.md` or `imagery/manifest.yaml`, a `package/` without `design_spec.md` or an SVG roster, a promoted `exports/`, a package-less language whose README does not declare reference-grade with its `BB-*` row, a manifest record naming a file that is not on disk, and an `unknown` licence marked `restriction: none`. It does not run the vendor SVG checker — [`deck-production.md`](../process/deck-production.md) preflights that at consumption, where a failure can still block the run.

`restriction` is the field that keeps a deck shippable. `reference-only` never reaches a rendered slide. `project-scoped` requires confirming the client or project is covered before external delivery. Treat an asset that arrived without licence metadata as `licence: unknown` with `restriction: project-scoped`; never upgrade it to `none` by assumption. An asset with no manifest record is not available for selection.

## `media/` — the operator's own captures

The personal DAM. Every recording, clip and photo the operator shot that another surface could plausibly use lives here once, is described once, and is found with `af search`. Studio posts, project posts and essays all draw from this shelf; none of them owns it. Intake procedure: [`library/process/media-intake.md`](../process/media-intake.md).

```text
library/assets/media/
├── media.yaml                    # root: <absolute blob-store path | "."> and defaults
├── cards/{sha12}.md              # ONE CARD PER ASSET — the record (frontmatter) plus review notes (body)
├── batches/{YYYY-MM-DD}-{slug}.md # one card per intake batch: place, dates, event, what came in
├── manifest.yaml                 # derived index (path, sha, kind, review) regenerated by the tool; never hand-edited
├── {YYYY}/{YYYY-MM-DD}-{batch}-{nn}.{ext}   # originals, only when root is "."
└── .derived/{sha12}/             # proxy.mp4 for HEVC sources, kf-1..3.jpg keyframes; regenerable
```

**The card is the record.** Its frontmatter is the structured truth for that asset, human-readable and agent-editable, the way a photo manager's sidecar file is. `manifest.yaml` is a derived index the tool regenerates. Cards sit under `library/`, a corpus root, so `af index update` indexes them and `af search "<scene>"` returns the card; the card names the file. Media files themselves are never indexed.

`root:` in `media.yaml` names where the bytes live on this machine. Defaults to `.` (beside the cards); point it at an external drive or a synced folder to keep gigabytes out of the vault. Card `path` values are relative to it, except register-in-place records, which are absolute.

### Card frontmatter

```yaml
sha256: <hex>                 # identity; the card filename is its first 12 characters
path: 2026/2026-09-06-toronto-move-03.mov   # relative to root, or absolute when registered in place
kind: video                   # video | photo
source: operator
licence: operator
restriction: none             # none | private | reference-only  (private never ships in a post)
captured_at: 2026-09-06T18:42:00-04:00
location: "Toronto, ON"
batch: 2026-09-06-toronto-move
duration: 14.2                # seconds, video only
orientation: portrait         # portrait | landscape | square
dimensions: "1080x1920"
codec: hevc                   # from ffprobe / EXIF
proxy: .derived/3f9a1c2b7d4e/proxy.mp4   # present when codec is not browser-decodable
keyframes: [.derived/3f9a1c2b7d4e/kf-1.jpg, .derived/3f9a1c2b7d4e/kf-2.jpg, .derived/3f9a1c2b7d4e/kf-3.jpg]
phash: <hex>                  # perceptual hash for near-duplicate detection
review: pending               # pending | done
reviewed_by: null             # gemini | claude | qwen3-vl | operator
description: ""               # one concrete sentence, filled at review
tags: []                      # five to eight: subject, place, action
role: null                    # b-roll | hero | reference | personal | talking-head
mood: []                      # a few words: calm, energetic, moody, bright, cozy, urban, nature
setting: null                 # indoor | outdoor, plus time of day when it matters
motion: null                  # static | pan | handheld | timelapse   (video only)
people: none                  # none | self | others | self+others
quality: null                 # hero | b-roll | reference | reject
used_in: []                   # post or project slugs, appended by the edit route
```

The tool fills everything above `review`; the reviewing agent fills the rest. `people: others` and `restriction: private` gate use in a post behind an operator check. An asset with no card is not available for selection.

## `audio/` — third-party sound

Sound effects and music beds, licensed. Same card-per-asset shape as `media/` with the imagery manifest's licence fields load-bearing: `source`, `source_ref`, `licence`, `attribution_required`, `restriction`. A track with `licence: unknown` is `restriction: reference-only` and never reaches a render. The design language's `video/audio.yaml` points at records here; the edit route never resolves sound from anywhere else.
