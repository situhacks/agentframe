# Assets

Reusable deck/presentation assets shared across projects. Two areas, different weights.

## `logos/`

Flat brand marks. The filename is the inventory — no metadata files, no per-logo notes.

- Naming: `<brand>-<variant>.<ext>`, lowercase kebab. Variants: `white`, `black`, `color`, `icon`, `wordmark`.
  Examples: `deloitte-white.svg`, `cibc-color.png`, `sfu-wordmark.svg`.
- Prefer SVG; recolorable monochrome marks are the most reusable.
- A logo lands here the second time a project needs it, or when a run fetches one from an open source.

Sourcing order for a run that needs a logo: check `logos/` → fetch from an open source and save it
here → ask the operator only as a last resort. Open sources: [Simple Icons](https://simpleicons.org)
(monochrome brand marks, ideal white/black), [gilbarbara/logos](https://github.com/gilbarbara/logos)
(full-color), Wikimedia Commons (official wordmarks).

## `deck-templates/<name>/`

Saved ppt-master template packages — a reusable deck design, not a one-off. Created by ppt-master's
`create-template` / `create-brand` workflows and consumed by handing the Strategist (Step 3) the
explicit directory path.

- Contains `design_spec.md` (`kind: brand | layout | deck` frontmatter) plus any layout roster SVGs.
- Portability: `design_spec.md` is plain markdown — Open Design and the `pptx` skill read it as design
  guidance (palette, typography, voice). The SVG roster replays only through ppt-master.
