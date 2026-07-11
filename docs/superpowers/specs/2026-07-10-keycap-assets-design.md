# Photographic Keycap Assets — Design Spec

**Date:** 2026-07-10 · **Mode:** Builder · **Status:** approved, prompts issued

## Purpose

Replace the flat CSS `.keycap` swatches (alpha, modifier, teal, pink) and the flat black `.af-mark`
square with AI-generated photographic keycap textures, matching the GMK Light Dolch identity
already sampled into `styles.css`. Legend text (project initials, "AF") stays live CSS/DOM overlay
on top of the image — never baked into the PNG, per the existing visual spec
(`.claude/plans/2026-07-07-agentframe-local-surface-v2-visual-spec.md`).

## Shared Style-Lock (applies to all 5 variants)

- Top-down orthographic angle (camera directly above the cap, not a 3/4 hero angle)
- No cast shadow onto the background — background is pure flat white for clean cutout
- Lighting/ambient occlusion lives on the cap surface only (bevel highlights, wall-taper shading)
- Sculpted-keycap material realism: visible stem well silhouette, wall taper, top bezel from
  directly above
- Consistent framing/crop across all generations so batch outputs drop into the same asset
  pipeline without per-image manual alignment

## 5 Variants

1. **Alpha** — pale cool grey `#E7F1F3`, Dolch light plastic (PBT/ABS satin finish)
2. **Modifier** — cool grey `~#C5D1D9`, Dolch light plastic
3. **Teal accent** — `~#00A896`, Dolch light plastic
4. **Pink accent** — `~#e18d9c` — AgentFrame-specific addition, not canonical GMK Dolch, styled to
   match the set's finish/material
5. **AF mark** — dark graphite/charcoal matte plastic body (different material family from 1-4),
   with a recessed teal `~#00A896` legend-window inset centered on the cap top, where "AF" will
   overlay in CSS. Same top-down/no-shadow/white-bg rules as 1-4; only the material and the
   recessed-window geometry differ.

## Generation Plan

Each of the 5 prompts below is run **3×** per generator call (natural per-generation variance —
no deliberate lighting/texture axis requested) → **15 raw PNGs on white background**.

Target generators: Nano Banana (Gemini image gen) or GPT image-2.0 family. Both follow literal
natural-language constraints reliably, which this task depends on (plain white bg, no shadow,
top-down).

## Tooling (follow-up, not part of this prompt handoff)

A local Python/Pillow script will threshold near-white pixels to transparent alpha with feathered
edges, batch-processing a folder of raw PNGs into clean transparent output. Reusable for future
re-generation batches.

## Output Location

`system/server/static/surface/assets/keycaps/{alpha,modifier,teal,pink,af-mark}-{1,2,3}.png`

Wiring into `.keycap` / `.af-mark` CSS (random-per-load or deterministic-per-project pick among
the 3 variants) is a later follow-up once favorites are picked from the batch.

## Non-Goals

- No change to the deterministic project→color assignment logic
- No legend/text baked into any generated image
- No new cap geometry beyond what's specified here
- No CSS/JS wiring in this pass

---

## Prompts

Each prompt is fully self-contained — paste as-is, no external context needed. Run each 3
times per color/variant.

### 1. Alpha (pale cool grey)

```
Studio product photograph of a single mechanical keyboard keycap, shot from directly above
(top-down orthographic view, camera perfectly perpendicular to the cap so no side walls are
visible except a thin sliver of bevel at the very edge). The keycap is a Cherry-profile sculpted
cap in a pale, cool light-grey satin PBT plastic, color approximately #E7F1F3, completely blank
with no legend, text, or printing on it. Soft, even studio lighting from slightly above creates a
subtle highlight across the top bezel and gentle ambient occlusion in the corners and stem-well
area, but casts no visible shadow onto the background. The background is pure flat white
(#FFFFFF), seamless, with zero shadow or gradient falling on it, suitable for a clean automated
cutout. The keycap is centered in the frame with generous white margin on all sides. Photorealistic,
sharp focus, macro product photography, no text, no logo, no watermark.
```

### 2. Modifier (cool grey)

```
Studio product photograph of a single mechanical keyboard keycap, shot from directly above
(top-down orthographic view, camera perfectly perpendicular to the cap so no side walls are
visible except a thin sliver of bevel at the very edge). The keycap is a Cherry-profile sculpted
cap in a cool medium-grey satin PBT plastic, color approximately #C5D1D9, completely blank with
no legend, text, or printing on it. Soft, even studio lighting from slightly above creates a
subtle highlight across the top bezel and gentle ambient occlusion in the corners and stem-well
area, but casts no visible shadow onto the background. The background is pure flat white
(#FFFFFF), seamless, with zero shadow or gradient falling on it, suitable for a clean automated
cutout. The keycap is centered in the frame with generous white margin on all sides. Photorealistic,
sharp focus, macro product photography, no text, no logo, no watermark.
```

### 3. Teal accent

```
Studio product photograph of a single mechanical keyboard keycap, shot from directly above
(top-down orthographic view, camera perfectly perpendicular to the cap so no side walls are
visible except a thin sliver of bevel at the very edge). The keycap is a Cherry-profile sculpted
cap in a muted dusty teal satin PBT plastic, color approximately #00A896, completely blank with
no legend, text, or printing on it. Soft, even studio lighting from slightly above creates a
subtle highlight across the top bezel and gentle ambient occlusion in the corners and stem-well
area, but casts no visible shadow onto the background. The background is pure flat white
(#FFFFFF), seamless, with zero shadow or gradient falling on it, suitable for a clean automated
cutout. The keycap is centered in the frame with generous white margin on all sides. Photorealistic,
sharp focus, macro product photography, no text, no logo, no watermark.
```

### 4. Pink accent

```
Studio product photograph of a single mechanical keyboard keycap, shot from directly above
(top-down orthographic view, camera perfectly perpendicular to the cap so no side walls are
visible except a thin sliver of bevel at the very edge). The keycap is a Cherry-profile sculpted
cap in a muted dusty rose-pink satin PBT plastic, color approximately #E18D9C, completely blank
with no legend, text, or printing on it, matching the same satin plastic finish as a light grey
and teal keycap set from the same keyboard keyset family. Soft, even studio lighting from slightly
above creates a subtle highlight across the top bezel and gentle ambient occlusion in the corners
and stem-well area, but casts no visible shadow onto the background. The background is pure flat
white (#FFFFFF), seamless, with zero shadow or gradient falling on it, suitable for a clean
automated cutout. The keycap is centered in the frame with generous white margin on all sides.
Photorealistic, sharp focus, macro product photography, no text, no logo, no watermark.
```

### 5. AF mark (dark graphite, recessed teal window)

```
Studio product photograph of a single premium mechanical keyboard keycap, shot from directly above
(top-down orthographic view, camera perfectly perpendicular to the cap so no side walls are
visible except a thin sliver of bevel at the very edge, same camera angle and framing as a plain
top-down keycap product shot). The keycap is a Cherry-profile sculpted cap in a dark matte
charcoal/graphite plastic body, color approximately #2A2A2A. Centered on the top face is a
recessed square legend window (a dye-sub style inset panel, inset a millimeter or two below the
surrounding cap surface) filled with a muted dusty teal color approximately #00A896, with a subtle
glossy highlight along its top edge. The teal window is left completely blank with no text or
symbol printed on it. Soft, even studio lighting from slightly above creates a subtle highlight
across the top bezel and gentle ambient occlusion in the corners and stem-well area, but casts no
visible shadow onto the background. The background is pure flat white (#FFFFFF), seamless, with
zero shadow or gradient falling on it, suitable for a clean automated cutout. The keycap is
centered in the frame with generous white margin on all sides. Photorealistic, sharp focus, macro
product photography, no text, no logo, no watermark.
```
