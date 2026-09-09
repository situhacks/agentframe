#!/usr/bin/env python3
"""Media intake for the operator's own captures: library/assets/media/ (schema: library/assets/README.md).

The deterministic half of library/process/media-intake.md. The tool inventories, hashes,
places, proxies, extracts keyframes, and writes one card per asset; a reviewing agent (any
harness that can view the media) fills the judgment fields on the card. Cards are the record;
manifest.yaml is a derived index.

Usage:
    python system/tools/media_intake.py inventory <folder>
    python system/tools/media_intake.py ingest <folder> --batch <slug> [--context "..."] [--copy] [--no-index]
    python system/tools/media_intake.py register <file> --batch <slug> [--context "..."]
    python system/tools/media_intake.py queue [--include-local] [--json]   # review-queue.md for a reviewing agent
    python system/tools/media_intake.py review [--limit N] [--model SLUG]   # automatic review: Gemini via the Antigravity CLI
    python system/tools/media_intake.py render [--no-index]
    python system/tools/media_intake.py doctor

Layout (root from media.yaml `root:`, default "." = beside the cards):
    cards/{sha12}.md  batches/{slug}.md  manifest.yaml  {YYYY}/{date}-{batch}-{nn}.{ext}  .derived/{sha12}/

Stdlib + PyYAML + Pillow (pillow_heif when present) + ffmpeg/ffprobe. No model is required: the review
fields are filled by a multimodal agent working from review-queue.md (the operator uses Gemini in
Antigravity), or `review` delegates each pending card to Gemini through the Antigravity CLI
(system/tools/agy_call.py, the operator's Google AI Pro login, nothing local). No model runs on this machine.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

try:
    import yaml
except ImportError:  # pragma: no cover
    print("error: pyyaml is required (pip install pyyaml)", file=sys.stderr)
    sys.exit(1)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SHELF = os.path.join(ROOT, "library", "assets", "media")
CARDS = os.path.join(SHELF, "cards")
BATCHES = os.path.join(SHELF, "batches")
CONFIG = os.path.join(SHELF, "media.yaml")
MANIFEST = os.path.join(SHELF, "manifest.yaml")

VIDEO_EXT = {".mp4", ".mov", ".m4v", ".mkv", ".webm", ".avi", ".mts", ".3gp"}
PHOTO_EXT = {".jpg", ".jpeg", ".png", ".heic", ".heif", ".webp", ".tif", ".tiff", ".dng"}
BROWSER_CODECS = {"h264", "vp8", "vp9", "av1"}   # what headless Chromium decodes without help
NEAR_DUP_HAMMING = 6
REVIEW_FIELDS = ("description", "tags", "role", "mood", "setting", "motion", "people", "quality", "restriction")

CARD_KEYS = ["sha256", "path", "kind", "source", "licence", "restriction", "captured_at", "location", "gps",
             "batch", "duration", "orientation", "dimensions", "codec", "proxy", "keyframes", "phash",
             "near_duplicate_of", "review", "reviewed_by", "description", "tags", "role", "mood", "setting",
             "motion", "people", "quality", "used_in", "original_name", "ingested_at"]


# ---------------------------------------------------------------- basics

def die(msg):
    print(f"media_intake: ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def log(msg):
    print(f"[media_intake] {msg}", file=sys.stderr)


def now_iso():
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def load_config():
    if os.path.isfile(CONFIG):
        with open(CONFIG, encoding="utf-8") as fh:
            cfg = yaml.safe_load(fh) or {}
    else:
        cfg = {}
    cfg.setdefault("root", ".")
    return cfg


def ensure_shelf():
    for d in (SHELF, CARDS, BATCHES):
        os.makedirs(d, exist_ok=True)
    if not os.path.isfile(CONFIG):
        with open(CONFIG, "w", encoding="utf-8", newline="\n") as fh:
            fh.write("# root: where the bytes live on this machine. \".\" keeps originals beside the cards;\n"
                     "# an absolute path (external drive, synced folder) keeps gigabytes out of the vault.\n"
                     "root: .\n")


def blob_root():
    r = load_config().get("root") or "."
    return SHELF if r in (".", "") else os.path.abspath(os.path.expandvars(os.path.expanduser(str(r))))


def derived_dir(sha12):
    return os.path.join(SHELF, ".derived", sha12)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def kind_of(path):
    ext = os.path.splitext(path)[1].lower()
    if ext in VIDEO_EXT:
        return "video"
    if ext in PHOTO_EXT:
        return "photo"
    return None


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def tool(name):
    return shutil.which(name) or die(f"{name} not on PATH")


# ---------------------------------------------------------------- probing

def ffprobe(path):
    r = run([tool("ffprobe"), "-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
    if r.returncode != 0:
        return None
    return json.loads(r.stdout or "{}")


def parse_iso6709(s):
    m = re.match(r"([+-]\d+(?:\.\d+)?)([+-]\d+(?:\.\d+)?)", s or "")
    return [float(m.group(1)), float(m.group(2))] if m else None


def video_facts(path):
    info = ffprobe(path) or {}
    v = next((s for s in info.get("streams", []) if s.get("codec_type") == "video"), {})
    fmt = info.get("format", {})
    tags = {**fmt.get("tags", {}), **v.get("tags", {})}
    w, h = int(v.get("width") or 0), int(v.get("height") or 0)
    rot = 0
    try:
        rot = int(float(tags.get("rotate") or 0))
    except ValueError:
        rot = 0
    for sd in v.get("side_data_list", []) or []:
        if "rotation" in sd:
            try:
                rot = int(float(sd["rotation"]))
            except (TypeError, ValueError):
                pass
    if rot % 180:
        w, h = h, w
    created = tags.get("com.apple.quicktime.creationdate") or tags.get("creation_time")
    return {
        "duration": round(float(fmt.get("duration") or 0), 2),
        "codec": v.get("codec_name"),
        "dimensions": f"{w}x{h}" if w and h else None,
        "orientation": "portrait" if h > w else ("square" if h == w and h else "landscape"),
        "captured_at": created,
        "gps": parse_iso6709(tags.get("com.apple.quicktime.location.ISO6709") or tags.get("location")),
    }


def open_image(path):
    from PIL import Image, ImageOps
    try:
        import pillow_heif  # noqa: F401
        pillow_heif.register_heif_opener()
    except ImportError:
        pass
    im = Image.open(path)
    im = ImageOps.exif_transpose(im)
    return im


def photo_facts(path):
    facts = {"duration": None, "codec": os.path.splitext(path)[1].lower().lstrip("."),
             "dimensions": None, "orientation": None, "captured_at": None, "gps": None}
    try:
        im = open_image(path)
    except Exception as e:  # unreadable stays registered, flagged by doctor
        facts["error"] = str(e)[:120]
        return facts
    w, h = im.size
    facts["dimensions"] = f"{w}x{h}"
    facts["orientation"] = "portrait" if h > w else ("square" if h == w else "landscape")
    try:
        exif = im.getexif()
        dto = exif.get(36867) or exif.get(306)
        if dto:
            facts["captured_at"] = dt.datetime.strptime(str(dto), "%Y:%m:%d %H:%M:%S").isoformat()
        gps = exif.get_ifd(34853) if hasattr(exif, "get_ifd") else None
        if gps and 2 in gps and 4 in gps:
            def dms(v):
                return float(v[0]) + float(v[1]) / 60 + float(v[2]) / 3600
            lat = dms(gps[2]) * (-1 if gps.get(1) == "S" else 1)
            lon = dms(gps[4]) * (-1 if gps.get(3) == "W" else 1)
            facts["gps"] = [round(lat, 6), round(lon, 6)]
    except Exception:
        pass
    return facts


def file_mtime_iso(path):
    return dt.datetime.fromtimestamp(os.path.getmtime(path)).astimezone().isoformat(timespec="seconds")


def dhash(image, size=8):
    """Difference hash over a grayscale downscale; robust to resize and mild recompression."""
    from PIL import Image
    im = image.convert("L").resize((size + 1, size), Image.LANCZOS)
    px = list(im.get_flattened_data()) if hasattr(im, "get_flattened_data") else list(im.getdata())
    bits = 0
    for row in range(size):
        for col in range(size):
            left, right = px[row * (size + 1) + col], px[row * (size + 1) + col + 1]
            bits = (bits << 1) | (1 if left > right else 0)
    return f"{bits:016x}"


def hamming(a, b):
    try:
        return bin(int(a, 16) ^ int(b, 16)).count("1")
    except (TypeError, ValueError):
        return 99


# ---------------------------------------------------------------- derived media

def make_keyframes(path, kind, sha12, duration):
    out = derived_dir(sha12)
    os.makedirs(out, exist_ok=True)
    ff = tool("ffmpeg")
    frames = []
    if kind == "photo":
        target = os.path.join(out, "kf-1.jpg")
        try:
            im = open_image(path).convert("RGB")
            im.thumbnail((960, 960))
            im.save(target, "JPEG", quality=85)
            frames.append(target)
        except Exception:
            pass
        return frames
    # scene-aware first, evenly spaced fill
    scene = os.path.join(out, "scene-%d.jpg")
    run([ff, "-hide_banner", "-loglevel", "error", "-y", "-i", path,
         "-vf", "select='gt(scene,0.3)',scale=640:-2", "-vsync", "vfr", "-frames:v", "3", scene])
    picked = sorted(p for p in os.listdir(out) if p.startswith("scene-"))
    for i, name in enumerate(picked[:3], 1):
        os.replace(os.path.join(out, name), os.path.join(out, f"kf-{i}.jpg"))
        frames.append(os.path.join(out, f"kf-{i}.jpg"))
    for leftover in (p for p in os.listdir(out) if p.startswith("scene-")):
        os.remove(os.path.join(out, leftover))
    if len(frames) < 3 and duration:
        for frac in (0.15, 0.5, 0.85):
            if len(frames) >= 3:
                break
            target = os.path.join(out, f"kf-{len(frames) + 1}.jpg")
            r = run([ff, "-hide_banner", "-loglevel", "error", "-y", "-ss", f"{duration * frac:.2f}", "-i", path,
                     "-frames:v", "1", "-vf", "scale=640:-2", target])
            if r.returncode == 0 and os.path.isfile(target):
                frames.append(target)
    return frames


_ENCODER = None


def h264_encoder():
    global _ENCODER
    if _ENCODER is None:
        r = run([tool("ffmpeg"), "-hide_banner", "-encoders"])
        _ENCODER = "h264_amf" if "h264_amf" in (r.stdout or "") else "libx264"
    return _ENCODER


def make_proxy(path, sha12):
    out = os.path.join(derived_dir(sha12), "proxy.mp4")
    os.makedirs(derived_dir(sha12), exist_ok=True)
    enc = h264_encoder()
    quality = ["-rc", "cqp", "-qp_i", "20", "-qp_p", "22"] if enc == "h264_amf" else ["-preset", "veryfast", "-crf", "20"]
    cmd = [tool("ffmpeg"), "-hide_banner", "-loglevel", "error", "-y", "-i", path,
           "-vf", "scale='min(1920,iw)':-2", "-c:v", enc, *quality, "-pix_fmt", "yuv420p",
           "-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart", out]
    r = run(cmd)
    if r.returncode != 0 and enc != "libx264":
        cmd[cmd.index("-c:v") + 1:cmd.index("-pix_fmt")] = ["libx264", "-preset", "veryfast", "-crf", "20"]
        r = run(cmd)
    return out if r.returncode == 0 and os.path.isfile(out) else None


# ---------------------------------------------------------------- cards

def card_path(sha12):
    return os.path.join(CARDS, f"{sha12}.md")


def rel_to_shelf(path):
    return os.path.relpath(path, SHELF).replace("\\", "/")


def read_card(path):
    text = open(path, encoding="utf-8-sig").read()
    m = re.match(r"\A---\r?\n(.*?)\r?\n---\r?\n?(.*)\Z", text, re.S)
    if not m:
        return None, text
    return (yaml.safe_load(m.group(1)) or {}), m.group(2)


def write_card(rec, body=None):
    ordered = {k: rec.get(k) for k in CARD_KEYS if k in rec}
    for k, v in rec.items():
        if k not in ordered:
            ordered[k] = v
    fm = yaml.safe_dump(ordered, sort_keys=False, allow_unicode=True, default_flow_style=None, width=100)
    if body is None:
        body = default_body(rec)
    with open(card_path(rec["sha256"][:12]), "w", encoding="utf-8", newline="\n") as fh:
        fh.write(f"---\n{fm}---\n{body}")


def default_body(rec):
    sha12 = rec["sha256"][:12]
    title = rec.get("original_name") or os.path.basename(rec.get("path", sha12))
    lines = [f"# {rec.get('batch', 'asset')} · {title}", ""]
    if rec.get("description"):
        lines += [rec["description"], ""]
    lines += [f"`{rec.get('kind')}` · {rec.get('dimensions') or '?'}"
              + (f" · {rec.get('duration')}s" if rec.get("duration") else "")
              + (f" · {rec.get('codec')}" if rec.get("codec") else "")
              + (f" · {rec.get('location')}" if rec.get("location") else ""), ""]
    for i, kf in enumerate(rec.get("keyframes") or [], 1):
        lines.append(f"![kf{i}](../{kf})")
    if rec.get("keyframes"):
        lines.append("")
    lines += ["## Review notes", "", "(the reviewing agent's remarks: why it is or is not usable, what to pair it with)", ""]
    return "\n".join(lines)


def all_cards():
    out = []
    if not os.path.isdir(CARDS):
        return out
    for name in sorted(os.listdir(CARDS)):
        if name.endswith(".md"):
            rec, body = read_card(os.path.join(CARDS, name))
            if rec and rec.get("sha256"):
                out.append((rec, body))
    return out


def resolve_path(rec):
    p = rec.get("path") or ""
    return p if os.path.isabs(p) else os.path.join(blob_root(), p)


# ---------------------------------------------------------------- inventory

def walk_media(folder):
    files = []
    for root, _, names in os.walk(folder):
        for n in names:
            p = os.path.join(root, n)
            if kind_of(p):
                files.append(p)
    return sorted(files)


def cmd_inventory(args):
    folder = os.path.abspath(args.folder)
    os.path.isdir(folder) or die(f"not a folder: {folder}")
    files = walk_media(folder)
    counts, total, dates, codecs = {"video": 0, "photo": 0}, 0, [], {}
    for f in files:
        k = kind_of(f)
        counts[k] += 1
        total += os.path.getsize(f)
        facts = video_facts(f) if k == "video" else photo_facts(f)
        d = (facts.get("captured_at") or file_mtime_iso(f))[:10]
        dates.append(d)
        c = facts.get("codec") or "?"
        codecs[c] = codecs.get(c, 0) + 1
    print(f"{len(files)} files: {counts['video']} videos, {counts['photo']} photos, {total / 1e9:.2f} GB")
    if dates:
        print(f"dates: {min(dates)} to {max(dates)}")
    print("codecs: " + ", ".join(f"{k}×{v}" for k, v in sorted(codecs.items())))
    hevc = sum(v for k, v in codecs.items() if k in ("hevc", "prores", "mpeg4", "dnxhd"))
    if hevc:
        print(f"{hevc} video(s) need an H.264 proxy for the browser render engine")
    known = {r["sha256"] for r, _ in all_cards()}
    if known:
        dup = sum(1 for f in files if sha256_of(f) in known)
        if dup:
            print(f"{dup} already on the shelf (exact duplicates will be skipped)")


# ---------------------------------------------------------------- ingest / register

def slugify(s):
    return re.sub(r"[^a-z0-9]+", "-", (s or "").lower()).strip("-")


def build_record(src, kind, sha, batch, context, ingested_name):
    facts = video_facts(src) if kind == "video" else photo_facts(src)
    captured = facts.get("captured_at") or file_mtime_iso(src)
    rec = {
        "sha256": sha, "path": None, "kind": kind, "source": "operator", "licence": "operator",
        "restriction": "none", "captured_at": captured, "location": None, "gps": facts.get("gps"),
        "batch": batch, "duration": facts.get("duration"), "orientation": facts.get("orientation"),
        "dimensions": facts.get("dimensions"), "codec": facts.get("codec"), "proxy": None,
        "keyframes": [], "phash": None, "near_duplicate_of": None,
        "review": "pending", "reviewed_by": None, "description": "", "tags": [], "role": None,
        "mood": [], "setting": None, "motion": None, "people": "none", "quality": None, "used_in": [],
        "original_name": ingested_name, "ingested_at": now_iso(),
    }
    if context:
        rec["batch_context"] = context
    if facts.get("error"):
        rec["unreadable"] = facts["error"]
    return rec


def finish_derived(rec, path):
    sha12 = rec["sha256"][:12]
    frames = make_keyframes(path, rec["kind"], sha12, rec.get("duration"))
    rec["keyframes"] = [rel_to_shelf(f) for f in frames]
    if rec["kind"] == "video" and rec.get("codec") and rec["codec"] not in BROWSER_CODECS:
        proxy = make_proxy(path, sha12)
        rec["proxy"] = rel_to_shelf(proxy) if proxy else None
    if frames:
        try:
            from PIL import Image
            with Image.open(frames[min(1, len(frames) - 1)]) as im:
                rec["phash"] = dhash(im)
        except Exception:
            rec["phash"] = None
    return rec


def flag_near_duplicates(rec, existing):
    if not rec.get("phash"):
        return None
    best = None
    for other, _ in existing:
        if other["sha256"] == rec["sha256"] or other.get("kind") != rec.get("kind") or not other.get("phash"):
            continue
        d = hamming(rec["phash"], other["phash"])
        if d <= NEAR_DUP_HAMMING and (best is None or d < best[1]):
            best = (other["sha256"][:12], d)
    return best[0] if best else None


def place_original(src, rec, batch, counter, copy):
    root = blob_root()
    day = (rec.get("captured_at") or now_iso())[:10]
    year = day[:4]
    ext = os.path.splitext(src)[1].lower()
    folder = os.path.join(root, year)
    os.makedirs(folder, exist_ok=True)
    n = counter.get((day, batch), 0) + 1
    while True:
        target = os.path.join(folder, f"{day}-{batch}-{n:02d}{ext}")
        if not os.path.exists(target):
            break
        n += 1
    counter[(day, batch)] = n
    (shutil.copy2 if copy else shutil.move)(src, target)
    rec["path"] = os.path.relpath(target, root).replace("\\", "/") if root == SHELF or target.startswith(root) else target
    return target


def cmd_ingest(args):
    folder = os.path.abspath(args.folder)
    os.path.isdir(folder) or die(f"not a folder: {folder}")
    batch = slugify(args.batch) or die("--batch must be a slug")
    ensure_shelf()
    existing = all_cards()
    known = {r["sha256"]: r for r, _ in existing}
    files = walk_media(folder)
    files or die(f"no media under {folder}")
    added, dupes, near, flagged, unreadable = [], [], [], [], []
    counter = {}
    for i, src in enumerate(files, 1):
        kind = kind_of(src)
        sha = sha256_of(src)
        if sha in known:
            dupes.append((src, known[sha]["path"]))
            continue
        rec = build_record(src, kind, sha, batch, args.context, os.path.basename(src))
        target = place_original(src, rec, batch, counter, args.copy)
        rec = finish_derived(rec, target)
        nd = flag_near_duplicates(rec, existing)
        if nd:
            rec["near_duplicate_of"] = nd
            near.append((rec["path"], nd))
        if rec.get("unreadable"):
            unreadable.append(rec["path"])
        write_card(rec)
        existing.append((rec, None))
        known[sha] = rec
        added.append(rec)
        log(f"{i}/{len(files)} {os.path.basename(target)} -> cards/{sha[:12]}.md")
    write_batch_card(batch, args.context, added)
    write_manifest()
    print(f"ingested {len(added)} asset(s) into batch '{batch}'"
          + (f"; skipped {len(dupes)} exact duplicate(s)" if dupes else ""))
    for src, existing_path in dupes:
        print(f"  duplicate: {os.path.basename(src)} = {existing_path}")
    for p, nd in near:
        print(f"  near-duplicate: {p} ~ cards/{nd}.md")
    for p in unreadable:
        print(f"  unreadable: {p}")
    proxies = sum(1 for r in added if r.get("proxy"))
    if proxies:
        print(f"  {proxies} H.264 proxy(ies) written under .derived/")
    print(f"  {len(added)} card(s) at review: pending — run the review pass (media-intake.md step 4)")
    if not args.no_index:
        reindex()


def cmd_register(args):
    src = os.path.abspath(args.file)
    os.path.isfile(src) or die(f"not a file: {src}")
    kind = kind_of(src) or die("not a media file")
    batch = slugify(args.batch) or die("--batch must be a slug")
    ensure_shelf()
    sha = sha256_of(src)
    if os.path.isfile(card_path(sha[:12])):
        die(f"already on the shelf: cards/{sha[:12]}.md")
    rec = build_record(src, kind, sha, batch, args.context, os.path.basename(src))
    rec["path"] = src.replace("\\", "/")   # registered in place: absolute, never moved
    rec["registered_in_place"] = True
    rec = finish_derived(rec, src)
    write_card(rec)
    write_manifest()
    print(f"registered in place: {src} -> cards/{sha[:12]}.md (review: pending)")
    if not args.no_index:
        reindex()


# ---------------------------------------------------------------- batch card, manifest, index

def write_batch_card(batch, context, recs):
    path = os.path.join(BATCHES, f"{batch}.md")
    prior = []
    if os.path.isfile(path):
        fm, _ = read_card(path)
        prior = (fm or {}).get("assets") or []
    assets = prior + [r["sha256"][:12] for r in recs]
    fm = {"batch": batch, "context": context or None, "ingested_at": now_iso(), "count": len(assets), "assets": assets}
    body = [f"# Batch — {batch}", ""]
    if context:
        body += [context, ""]
    body += ["| Card | Kind | Captured | Description |", "|---|---|---|---|"]
    by_id = {r["sha256"][:12]: r for r, _ in all_cards()}
    for sid in assets:
        r = by_id.get(sid) or next((x for x in recs if x["sha256"][:12] == sid), {})
        body.append(f"| [{sid}](../cards/{sid}.md) | {r.get('kind', '?')} | {(r.get('captured_at') or '')[:10]} | {r.get('description') or '_pending review_'} |")
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("---\n" + yaml.safe_dump(fm, sort_keys=False, allow_unicode=True) + "---\n" + "\n".join(body) + "\n")


def write_manifest():
    rows = []
    for rec, _ in all_cards():
        rows.append({k: rec.get(k) for k in ("sha256", "path", "kind", "batch", "review", "role", "captured_at", "proxy")})
    with open(MANIFEST, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("# DERIVED INDEX — regenerated by media_intake.py; the cards are the record.\n")
        yaml.safe_dump({"generated_at": now_iso(), "count": len(rows), "assets": rows}, fh, sort_keys=False, allow_unicode=True)


def reindex():
    r = run([sys.executable, os.path.join(ROOT, "system", "af.py"), "index", "update"], cwd=ROOT)
    tail = (r.stdout or r.stderr or "").strip().splitlines()
    log("af index update: " + (tail[-1] if tail else f"exit {r.returncode}"))


def cmd_render(args):
    ensure_shelf()
    batches = {}
    for rec, _ in all_cards():
        batches.setdefault(rec.get("batch"), []).append(rec)
    for b, recs in batches.items():
        if b:
            ctx = next((r.get("batch_context") for r in recs if r.get("batch_context")), None)
            write_batch_card_full(b, ctx, recs)
    write_manifest()
    print(f"rendered {len(batches)} batch card(s) and manifest.yaml")
    if not args.no_index:
        reindex()


def write_batch_card_full(batch, context, recs):
    """Rebuild a batch card from every card that names the batch (render), not just the new ones."""
    path = os.path.join(BATCHES, f"{batch}.md")
    if os.path.isfile(path):
        os.remove(path)
    write_batch_card(batch, context, sorted(recs, key=lambda r: r.get("captured_at") or ""))


# ---------------------------------------------------------------- review queue (hand-off to the reviewing agent)

QUEUE = os.path.join(SHELF, "review-queue.md")

REVIEW_PROMPT = """## Review prompt (for the reviewing agent; the operator runs this in Antigravity with Gemini)

You are reviewing assets on my personal media shelf for use as b-roll in short-form talking-head videos.
Work through the rows below. For each row: open the media (the proxy when listed, else the path) and watch
or look at it; then edit the card's YAML frontmatter and fill exactly these fields:

- description: one concrete sentence (what is visible, where, light, movement)
- tags: 5-8 lowercase tags (subject, place, action, objects)
- role: b-roll | hero | reference | personal | talking-head
- mood: 1-3 of calm, energetic, moody, bright, cozy, urban, nature, gritty, warm, cold
- setting: indoor or outdoor, plus time of day when visible
- motion (video only): static | pan | handheld | timelapse
- people: none | self | others | self+others   (self = me on camera)
- quality: hero | b-roll | reference | reject
- restriction: none | private | reference-only   (private = I would not publish it)

Then set `review: done` and `reviewed_by: gemini`. Change no other field and keep the YAML valid. Add one or
two lines under "## Review notes" in the card body when something matters: a strong two-second moment and
its timestamp, a person who needs consent, the reason for a reject. The card's `batch_context` names the
place and event; use it. When the queue is done run
`python system/tools/media_intake.py render && python system/af.py index update`, then report how many
cards you reviewed, which you rejected or marked private, and every card with `people: others`.
"""


def cmd_queue(args):
    ensure_shelf()
    states = {"pending"} | ({"local"} if args.include_local else set())
    rows = [(r, b) for r, b in all_cards() if r.get("review") in states]
    lines = [f"# Review queue - {len(rows)} card(s)", "", REVIEW_PROMPT,
             "| Card | Media (open this) | Kind | Dur | Keyframes | Batch context |", "|---|---|---|---|---|---|"]
    for rec, _ in rows:
        sid = rec["sha256"][:12]
        media = os.path.join(SHELF, rec["proxy"]) if rec.get("proxy") else resolve_path(rec)
        kfs = ", ".join(os.path.join(SHELF, k) for k in rec.get("keyframes") or [])
        ctx = (rec.get("batch_context") or rec.get("batch") or "").replace("|", "/")
        lines.append(f"| {card_path(sid)} | {media} | {rec.get('kind')} | {rec.get('duration') or ''} | {kfs} | {ctx} |")
    with open(QUEUE, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    if args.json:
        print(json.dumps([{"card": card_path(r["sha256"][:12]),
                           "media": (os.path.join(SHELF, r["proxy"]) if r.get("proxy") else resolve_path(r)),
                           "keyframes": [os.path.join(SHELF, k) for k in r.get("keyframes") or []],
                           "kind": r.get("kind"), "duration": r.get("duration"),
                           "batch_context": r.get("batch_context")} for r, _ in rows], indent=1))
    else:
        print(f"wrote {QUEUE} with {len(rows)} card(s) to review; hand it to the reviewing agent")


# ---------------------------------------------------------------- review (automatic: Gemini via the Antigravity CLI)

REVIEW_SCHEMA = {
    "type": "object",
    "properties": {
        "description": {"type": "string"},
        "tags": {"type": "array", "items": {"type": "string"}},
        "role": {"type": "string", "enum": ["b-roll", "hero", "reference", "personal", "talking-head"]},
        "mood": {"type": "array", "items": {"type": "string"}},
        "setting": {"type": "string"},
        "motion": {"type": "string", "enum": ["static", "pan", "handheld", "timelapse", "none"]},
        "people": {"type": "string", "enum": ["none", "self", "others", "self+others"]},
        "quality": {"type": "string", "enum": ["hero", "b-roll", "reference", "reject"]},
        "restriction": {"type": "string", "enum": ["none", "private", "reference-only"]},
        "best_moment": {"type": "string"},
        "note": {"type": "string"},
    },
    "required": ["description", "tags", "role", "mood", "setting", "people", "quality", "restriction"],
}

REVIEW_TASK = """You are cataloguing ONE asset from my personal footage library for use as b-roll in short-form
talking-head videos. Batch context: {context}
Watch or look at the file, then answer:
- description: one concrete sentence (what is visible, where, light, movement)
- tags: 5-8 lowercase tags (subject, place, action, objects)
- role: b-roll | hero | reference | personal | talking-head
- mood: 1-3 of calm, energetic, moody, bright, cozy, urban, nature, gritty, warm, cold
- setting: indoor or outdoor, plus time of day when visible
- motion: static | pan | handheld | timelapse (photos: none)
- people: none | self | others | self+others  (self = the single presenter on camera)
- quality: hero | b-roll | reference | reject
- restriction: none | private | reference-only  (private = not for publishing)
- best_moment: for video, the strongest two seconds as MM:SS-MM:SS; else empty
- note: one line if something matters (a person who needs consent, why reject), else empty
"""


def _agy():
    import importlib.util
    path = os.path.join(ROOT, "system", "tools", "agy_call.py")
    spec = importlib.util.spec_from_file_location("agy_call", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def cmd_review(args):
    agy = _agy()
    cards = [(r, b) for r, b in all_cards() if r.get("review") == "pending"]
    if args.limit:
        cards = cards[: args.limit]
    cards or die("nothing pending")
    done, failed = 0, []
    for rec, body in cards:
        sid = rec["sha256"][:12]
        media = os.path.join(SHELF, rec["proxy"]) if rec.get("proxy") else resolve_path(rec)
        if not os.path.isfile(media):
            failed.append((sid, "file missing"))
            continue
        env = agy.run(REVIEW_TASK.format(context=rec.get("batch_context") or rec.get("batch") or "none given"),
                      files=[media], schema=json.dumps(REVIEW_SCHEMA), model=args.model,
                      timeout=args.timeout)
        out = env.get("structured_output") if env.get("status") == "SUCCESS" else None
        if not isinstance(out, dict):
            failed.append((sid, f"{env.get('status')}: {env.get('stderr_tail', '')[:120]}"))
            continue
        for k in REVIEW_FIELDS:
            if out.get(k) not in (None, "", []):
                rec[k] = out[k]
        if rec.get("motion") == "none":
            rec["motion"] = None
        note_lines = []
        if out.get("best_moment"):
            note_lines.append(f"- best moment: {out['best_moment']}")
        if out.get("note"):
            note_lines.append(f"- {out['note']}")
        rec["review"] = "done"
        rec["reviewed_by"] = f"agy:{env.get('model')}"
        new_body = body if (body and "## Review notes" in body) else default_body(rec)
        if note_lines:
            new_body = new_body.rstrip("\n") + "\n" + "\n".join(note_lines) + "\n"
        write_card(rec, new_body)
        done += 1
        usage = env.get("usage") or {}
        log(f"{sid}: {rec.get('description', '')[:70]} ({usage.get('total_tokens', '?')} tok)")
    cmd_render(argparse.Namespace(no_index=True))
    print(f"reviewed {done} card(s) with {args.model} via the Antigravity CLI"
          + (f"; {len(failed)} failed" if failed else ""))
    for sid, why in failed:
        print(f"  failed: cards/{sid}.md — {why}")
    if not args.no_index:
        reindex()


# ---------------------------------------------------------------- doctor

def notes(root=None):
    """Shelf drift as advisory notes; af doctor may surface these."""
    out = []
    if not os.path.isdir(CARDS):
        return out
    cfg_root = blob_root()
    if cfg_root != SHELF and not os.path.isdir(cfg_root):
        out.append(f"library/assets/media/media.yaml: root {cfg_root} is not mounted")
    week_ago = (dt.datetime.now().astimezone() - dt.timedelta(days=7)).isoformat()
    for rec, _ in all_cards():
        sid = rec["sha256"][:12]
        p = resolve_path(rec)
        if not os.path.isfile(p):
            out.append(f"media cards/{sid}.md: file missing at {rec.get('path')}")
        if rec.get("kind") == "video" and rec.get("codec") and rec["codec"] not in BROWSER_CODECS and not rec.get("proxy"):
            out.append(f"media cards/{sid}.md: {rec['codec']} source with no H.264 proxy — the render engine cannot play it")
        if rec.get("review") == "pending" and (rec.get("ingested_at") or "") < week_ago:
            out.append(f"media cards/{sid}.md: review pending for over a week")
        if rec.get("people") == "others" and rec.get("used_in") and not rec.get("consent"):
            out.append(f"media cards/{sid}.md: people: others used in {rec['used_in']} with no consent note")
    return out


def cmd_doctor(args):
    for n in notes():
        print(f"note — {n}")
    print("media shelf checked")


# ---------------------------------------------------------------- main

def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8")
        except Exception:
            pass
    ap = argparse.ArgumentParser(description="Media intake for library/assets/media/")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("inventory"); s.add_argument("folder"); s.set_defaults(fn=cmd_inventory)
    s = sub.add_parser("ingest"); s.add_argument("folder"); s.add_argument("--batch", required=True)
    s.add_argument("--context"); s.add_argument("--copy", action="store_true", help="copy instead of move")
    s.add_argument("--no-index", action="store_true"); s.set_defaults(fn=cmd_ingest)
    s = sub.add_parser("register"); s.add_argument("file"); s.add_argument("--batch", required=True)
    s.add_argument("--context"); s.add_argument("--no-index", action="store_true"); s.set_defaults(fn=cmd_register)
    s = sub.add_parser("queue", help="write review-queue.md for the reviewing agent")
    s.add_argument("--include-local", action="store_true"); s.add_argument("--json", action="store_true")
    s.set_defaults(fn=cmd_queue)
    s = sub.add_parser("review", help="automatic review of pending cards: Gemini via the Antigravity CLI")
    s.add_argument("--limit", type=int); s.add_argument("--model", default="gemini-3.8-flash-medium")
    s.add_argument("--timeout", default="5m"); s.add_argument("--no-index", action="store_true"); s.set_defaults(fn=cmd_review)
    s = sub.add_parser("render"); s.add_argument("--no-index", action="store_true"); s.set_defaults(fn=cmd_render)
    s = sub.add_parser("doctor"); s.set_defaults(fn=cmd_doctor)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
