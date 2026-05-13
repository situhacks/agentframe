---
status: locked
last_updated: 2026-05-11T18:45:00-07:00
preview: preview/permission-boundary.html
tokens: tokens.yaml
light_variant:
  accent_hue: "#B64A2F"
  cover_font_family: "Space Grotesk"
  cover_motif_svg: |
    <svg width="240" height="160" viewBox="0 0 240 160" fill="none">
      <rect x="18" y="18" width="204" height="124" rx="18" stroke="white" stroke-opacity="0.55" stroke-width="3"/>
      <circle cx="58" cy="62" r="8" fill="white" fill-opacity="0.8"/>
      <circle cx="182" cy="62" r="8" fill="white" fill-opacity="0.8"/>
      <circle cx="120" cy="106" r="8" fill="white" fill-opacity="0.8"/>
      <path d="M66 62H174M120 70V98" stroke="white" stroke-opacity="0.45" stroke-width="2"/>
    </svg>
---

# Design Language: AI Automation POV

## One-Line Synth

Restrained editorial system with a warm permission-boundary accent: enough visual signal for a hero image, never louder than the POV.

## Palette

| token | hex | role |
|---|---|---|
| `bg` | `#F7F5EF` | Warm off-white canvas; avoids sterile SaaS white. |
| `surface` | `#FFFFFF` | Cards, inner panels, and document-safe surfaces. |
| `text` | `#171717` | Primary copy and diagram labels. |
| `muted` | `#68645E` | Metadata, secondary labels, and low-emphasis notes. |
| `divider` | `#D8D2C7` | Thin rules, node connectors, and audit-line marks. |
| `accent` | `#B64A2F` | Boundary outline, small status marks, and one strong visual moment. |
| `accent_soft` | `#E8B8A8` | Soft highlight behind boundary details. |
| `boundary` | `#2F3A45` | Tool surface boxes and darker structural lines. |

Accent rule: one warm boundary accent per composition. Do not use the accent as a full background.

## Type System

| face | role | weights | key sizes |
|---|---|---|---|
| Space Grotesk | Hero headline and large diagram labels | 500, 700 | 54-72px hero; 22-28px labels |
| Aptos / Inter fallback | Body, captions, metadata | 400, 600 | 18-24px body; 14-16px metadata |

Type rule: headline can be blunt and oversized; supporting labels stay small and functional.

## Emphasis Devices

| device | job | never-combine rule |
|---|---|---|
| Permission boundary box | Shows what the agent is allowed to touch. | Do not pair with heavy shadows or gradient fills. |
| Small permission nodes | Mark read, write, approve, or stop states. | Do not turn into a complex flowchart. |
| Audit-line marks | Suggest traceability without becoming a compliance diagram. | Do not combine with icon sets. |
| Off-centre caption block | Carries the plain-language tension of the post. | Do not add more than one caption block per visual. |

## Motif / Imagery

Primary motif: a thin rectangular boundary around a small cluster of tools, with one node sitting outside the boundary to imply "ask first" or "do not touch."

Hero image direction:

- Warm off-white background.
- Large blunt headline: "The useful agent knows its boundary."
- Simple diagram below or beside the headline: `Agent` inside a boundary, connected only to `Read`, `Draft`, and `Ask`; `Write` sits outside the boundary.
- Small footer line: "AI automation is an operating model problem before it is a model problem."

## Banned

- No robot icons.
- No glowing neural networks.
- No cyber blue/purple gradients.
- No dashboards full of tiny fake data.
- No "autonomy explosion" imagery.
- No generic SaaS product-card stack.
