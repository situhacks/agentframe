# Photographic Keycap Assets — Design Spec

**Date:** 2026-07-10 · **Mode:** Builder · **Status:** approved, prompts issued

## Purpose

Replace the flat CSS `.keycap` swatches (alpha, modifier, teal, pink) and the flat black `.af-mark`
square with AI-generated photographic keycap textures, matching the GMK Light Dolch identity
already sampled into `styles.css`. Legend text (project initials, "AF") stays live CSS/DOM overlay
on top of the image — never baked into the PNG, per the existing visual spec
(`.claude/plans/2026-07-07-agentframe-local-surface-v2-visual-spec.md`).

## Shared Style-Lock (applies to all 5 variants)

- 1U keycap, GMK/Cherry-profile, explicitly from the **GMK Light Dolch** keyset (named directly in
  each prompt so the model can draw on any training-data familiarity with the real set)
- ABS plastic, not PBT — real GMK caps are ABS
- Slight top-down angle, not strict orthographic — a small tilt (~10-15°) so the sculpted profile
  and front wall read correctly; pure straight-down-the-barrel top-down flattens the cap and looks
  wrong
- No cast shadow onto the background — background is pure flat white for clean cutout
- Rendering/lighting/material finish is left to the model's own judgment — don't over-specify
  highlight/AO placement, it already knows what an ABS keycap looks like
- Consistent framing/crop across all generations so batch outputs drop into the same asset
  pipeline without per-image manual alignment

## 5 Variants — generation strategy

Shape is locked **once**, from the alpha prompt, then reused as an image reference for pure
recolors — this keeps cap geometry identical across all colorways instead of re-rolling shape
variance on every color.

1. **Alpha** — pale cool grey `#E7F1F3` — generate from the text prompt below.
2. **Modifier** — cool grey `~#C5D1D9` — recolor of the chosen alpha image.
3. **Teal accent** — `~#00A896` — recolor of the chosen alpha image.
4. **Pink accent** — `~#e18d9c` (AgentFrame-specific, not canonical GMK Dolch) — recolor of the
   chosen alpha image.
5. **AF mark** — dark graphite/charcoal ABS body with a recessed teal `~#00A896` legend-window
   inset centered on the cap top, where "AF" will overlay in CSS. Different geometry from 1-4
   (has the inset window), so it gets its own standalone prompt, not a recolor.

## Generation Plan

1. Run the alpha prompt in ChatGPT (image-2.0), generate a few times, pick the best shape.
2. Feed that chosen image back in with the recolor prompt, once per remaining color (modifier,
   teal, pink) — three recolor passes, same source image each time, only the color instruction
   changes.
3. Run the AF mark prompt standalone (it has different geometry, not a recolor target).
4. Optionally repeat step 1-2 a second or third time for natural variance across a small batch,
   if more than one variant per color is wanted.

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

### 1. Alpha (base shape — generate this one from text)

```
Studio product photograph of a single 1U mechanical keyboard keycap from the GMK Light Dolch
keycap set, a real Cherry-profile keycap set made of ABS plastic. Shot from a slight, gentle
downward angle (roughly 10-15 degrees off directly overhead, not perfectly flat top-down) so the
sculpted Cherry profile, the top bezel, and a thin sliver of the front wall are all visible. The
keycap is the pale, cool light-grey alpha colorway from that set, color approximately #E7F1F3,
completely blank with no legend, text, or printing on it. Plain flat white (#FFFFFF) seamless
background with no shadow cast onto it, suitable for a clean automated cutout. The keycap is
centered in the frame with generous white margin on all sides. Photorealistic product photography,
no text, no logo, no watermark.
```

### 2. Recolor (reuse for modifier, teal, pink)

Feed the chosen alpha image back into the model along with this prompt, once per target color.
Only the color name/hex changes between the three runs — keep everything else identical so shape
stays locked.

```
Using the attached keycap image as the exact shape, angle, and reference, generate the same
keycap with the identical geometry, camera angle, framing, and background — but recolor the cap
body to [COLOR NAME], color approximately [HEX]. Keep the same plain flat white background with
no cast shadow, same centering and margin, same photorealistic style. Do not change the shape,
angle, or add any text/legend.
```

Fill in per color:
- Modifier: `cool medium-grey` / `#C5D1D9`
- Teal accent: `muted dusty teal` / `#00A896`
- Pink accent: `muted dusty rose-pink` / `#E18D9C`

### 3. AF mark (dark graphite, recessed teal window — standalone, not a recolor)

```
Studio product photograph of a single 1U mechanical keyboard keycap, Cherry profile, ABS plastic,
same keyset family and photography style as the GMK Light Dolch keycap set. Shot from a slight,
gentle downward angle (roughly 10-15 degrees off directly overhead, not perfectly flat top-down)
so the sculpted Cherry profile, the top bezel, and a thin sliver of the front wall are all visible.
The keycap body is a dark matte charcoal/graphite ABS plastic, color approximately #2A2A2A.
Centered on the top face is a recessed square legend window (a dye-sub style inset panel, inset a
millimeter or two below the surrounding cap surface), filled with a muted dusty teal color
approximately #00A896. The teal window is left completely blank with no text or symbol printed on
it. Plain flat white (#FFFFFF) seamless background with no shadow cast onto it, suitable for a
clean automated cutout. The keycap is centered in the frame with generous white margin on all
sides. Photorealistic product photography, no text, no logo, no watermark.
```
