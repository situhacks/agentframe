# Design Language — canonical shape

> **Canonical shape for `library/context/operator/design-language.md`.** The setup process ([`operator-context-setup.md`](../../process/operator-context-setup.md)) generates the operator's filled-in file from this shape; the bracketed prompts are the generation values. The operator's file is gitignored (private).

The system default for HOW any rendered deliverable looks (.docx, .pptx, future formats). Sibling to `voice.md` (verbal identity) and `positioning.md` (strategic identity).

This file defines the **safe baseline** — the look that applies when a campaign doesn't carry its own visual identity. Per-campaign visual identity is layered on top via `workspace/projects/{slug}/phase-3-planning/design-language/`.

Loaded by the agent on every export turn. Not loaded for prose work.

---

## Color Tokens

| Token | Value | Where it shows up |
|---|---|---|
| `bg` | `[#FFFFFF]` | Page background, cover background |
| `surface` | `[#F7F7F5]` | Table zebra striping, callout box fills |
| `text` | `[#1A1A1A]` | All body copy, all headings |
| `muted` | `[#6B6B6B]` | Captions, footers, metadata labels |
| `divider` | `[#D6D6D2]` | Hairlines, table row separators |
| `accent_primary` | `[#1F3A5F]` | H1 underline, table header fills, callout left rules |

Adjust to fit your visual identity. Keep token names stable so export prompts and templates can map values consistently.

---

## Typography

| Slot | Family | Fallback | Weight |
|---|---|---|---|
| Headings | `[Aptos]` | `[Calibri]` | 700 |
| Body | `[Aptos]` | `[Calibri]` | 400 |
| Mono | `[Consolas]` | `[Menlo, monospace]` | 400 |

Same contract — the slot names are read by exports config; change the families to match your identity.

---

## Composition with Per-Campaign Design Language

A campaign can override the system default by shipping its own DL at `workspace/projects/{slug}/phase-3-planning/design-language/design-language-v{N}.md`. The render pipeline composes:

1. **System default** (this file) — safe baseline.
2. **Per-campaign DL** — overrides specific tokens (often `accent_primary`, sometimes typography for cover).
3. **Per-deliverable template** — the .docx or .pptx skeleton in `workspace/projects/{slug}/exports/templates/{type}.{ext}`.

The agent reads all three on every export turn. If they conflict, per-campaign overrides system; per-deliverable overrides per-campaign.

---

## What's NOT in this file

- Per-deliverable layout decisions (slide masters, page templates) — those live in `workspace/projects/{slug}/exports/templates/`.
- Cover render parameters (Chrome headless flags, dimensions) — keep those in your export workflow docs/scripts.
- Voice (verbal tone) — `voice.md`.
- Strategic positioning — `positioning.md`.
