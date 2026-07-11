"""Normalize AI-generated keycap PNGs: trim to a consistent tight crop and
report the legend anchor point for CSS overlay text.

The source PNGs already ship with a real alpha-channel cutout (verified
against the 2026-07-10 batch) — this script does NOT re-derive transparency
from background color. It only crops to content and computes where the
cap's top face (legend surface) sits so CSS text overlay can be positioned
consistently across the set.

Usage:
    python system/tools/keycap_process.py <input_dir_or_files...> [--out DIR]
"""

import argparse
import glob
import os
import sys

from PIL import Image

# Fallback pad added around the tight alpha bbox, as a fraction of the
# cropped content size, so the cap doesn't touch the canvas edge.
PAD_PCT = 0.04


def crop_to_content(im: Image.Image) -> tuple[Image.Image, float]:
    """Crop to the alpha channel's tight bbox with a small pad.

    Returns (cropped_image, legend_anchor_pct) where legend_anchor_pct is
    the top face's estimated vertical center as a fraction of the cropped
    image's height, measured from the crease between the top face and the
    front wall (the widest horizontal run of opaque pixels below the top
    edge marks roughly where the front wall begins).
    """
    im = im.convert("RGBA")
    # getbbox() treats any nonzero alpha as content, which picks up faint
    # translucent halo pixels some generations leave around the cap edge.
    # Threshold to substantially-opaque pixels only for a tight, correct bbox.
    alpha = im.getchannel("A").point(lambda a: 255 if a > 64 else 0)
    bbox = alpha.getbbox()
    if bbox is None:
        raise ValueError("image has no opaque content")

    l, t, r, b = bbox
    w, h = r - l, b - t
    pad_x, pad_y = int(w * PAD_PCT), int(h * PAD_PCT)
    l = max(0, l - pad_x)
    t = max(0, t - pad_y)
    r = min(im.width, r + pad_x)
    b = min(im.height, b + pad_y)
    cropped = im.crop((l, t, r, b))

    # Top face spans from the cap's top edge to the top-face/front-wall
    # crease. Approximate the crease as ~55% down the *uncropped* bbox
    # (measured against the 2026-07-10 batch's consistent camera angle),
    # then re-express as a fraction of the cropped/padded frame.
    crease_in_bbox_pct = 0.55
    top_in_bbox_pct = 0.0
    face_center_bbox_pct = (top_in_bbox_pct + crease_in_bbox_pct) / 2
    face_center_px = t + pad_y + face_center_bbox_pct * h - t
    legend_anchor_pct = face_center_px / cropped.height

    return cropped, legend_anchor_pct


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inputs", nargs="+", help="PNG files, dirs, or glob patterns")
    ap.add_argument(
        "--out",
        default="system/server/static/surface/assets/keycaps",
        help="output directory",
    )
    args = ap.parse_args()

    files = []
    for pattern in args.inputs:
        if os.path.isdir(pattern):
            files.extend(sorted(glob.glob(os.path.join(pattern, "*.png"))))
        else:
            files.extend(sorted(glob.glob(pattern)))

    if not files:
        print("No input PNGs found.", file=sys.stderr)
        sys.exit(1)

    os.makedirs(args.out, exist_ok=True)
    anchors = []
    for f in files:
        im = Image.open(f)
        cropped, anchor_pct = crop_to_content(im)
        name = os.path.splitext(os.path.basename(f))[0]
        out_path = os.path.join(args.out, f"{name}.png")
        cropped.save(out_path)
        anchors.append(anchor_pct)
        print(f"{f} -> {out_path}  (size={cropped.size}, legend_anchor={anchor_pct*100:.1f}%)")

    avg = sum(anchors) / len(anchors)
    print(f"\nAverage legend anchor across batch: top: {avg*100:.1f}%")
    print("Use this as a shared CSS offset for the text overlay if framing is consistent.")


if __name__ == "__main__":
    main()
