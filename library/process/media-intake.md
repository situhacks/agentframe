# Media Intake

## Purpose

How the operator's own recordings, footage and photos enter the shared media shelf at `library/assets/media/` so that any surface (studio posts, project posts, essays) can find them with `af search` and use them without copying. Owns the intake order, the review contract any multimodal agent fills, and the placement rule. The record schema lives in [`library/assets/README.md`](../assets/README.md).

## When To Load

- The operator hands over a folder, a drive, or a drop of captures and asks for them to be organized, indexed, or made usable.
- A post or project recording turns out to be reusable and should be registered without moving it.
- A rescan or review pass over the shelf is requested.

Placement rule before anything else: **he shot it and another surface could plausibly use it → the media shelf. Someone else supplied it for one project → that project's `sources/`.** A post's own talking-head recording stays with the post unless it earns a register-in-place record.

## Procedure

The tool owns the deterministic steps; the reviewing agent owns the judgment fields. The agent may be any harness that can view the media (Gemini takes video natively; Claude and local models work from the keyframes the tool extracts).

### 1. Receive

Take the folder path and one line of batch context from the operator: place, dates, event, whether other people appear. That line seeds the batch card and the `location`, `batch` and `restriction` fields.

### 2. Inventory and report

```
python system/tools/media_intake.py inventory <folder>
```

Counts by kind, total size, date range, codecs. Report it before touching anything; a wrong folder is cheapest to catch here.

### 3. Ingest

```
python system/tools/media_intake.py ingest <folder> --batch "<slug>" --context "<one line>"
```

The tool: hashes every file and skips exact duplicates; flags near-duplicate videos and burst photos; moves originals to `{root}/{YYYY}/{YYYY-MM-DD}-{batch}-{nn}.{ext}` without transcoding; writes an H.264 proxy for HEVC sources; extracts three scene-aware keyframes per video; reads EXIF and `ffprobe` facts; writes one card per asset with `review: pending` and the batch card; then runs `af index update`. Duplicates are listed, never silently dropped.

### 4. Review (judgment)

Open each pending card, view the media (or its keyframes), and fill the review fields on the card's frontmatter: `description` (one sentence, concrete), `tags` (five to eight: subject, place, action), `role` (`b-roll | hero | reference | personal | talking-head`), `mood` (a few words: calm, energetic, moody, bright, cozy, urban, nature), `setting` (indoor or outdoor, time of day), `motion` (static, pan, handheld, timelapse) for video, `people` (`none | self | others | self+others`), `quality` (`hero | b-roll | reference | reject`), and `restriction` (`none | private | reference-only`). Set `review: done` and `reviewed_by`. Anything with `people: others` or `restriction: private` is flagged for the operator before it can ship in a post.

A local fallback fills `description` and `tags` from keyframes when no reviewing agent is available:

```
python system/tools/media_intake.py describe --pending --model qwen3-vl:8b
```

Then rebuild the derived index so search sees the review:

```
python system/tools/media_intake.py render && python system/af.py index update
```

### 5. Report

What landed, what was flagged (duplicates, unreadable files, people present, private), and five sample queries that return the right asset. The operator sees it worked.

### Register in place

For a reusable file that must not move (a post recording, a project source): `python system/tools/media_intake.py register <file> --batch "<slug>"` writes a card whose `path` is absolute and leaves the file where it is.

## Verification Or Logging

- Step 5's five queries hit through `af search`.
- `python system/af.py doctor` notes: a card whose file is missing, an HEVC record with no proxy, a card still `review: pending` after seven days, a `people: others` asset used in a post without a consent note.
- No audit row; intake is content work.

## Boundaries

- Does not decide which asset a post uses; the edit route does that through `af search`.
- Does not manage third-party media (stock, sfx, music), which follow the `library/assets/audio/` and imagery records with their licence fields.
- Does not transcode originals or delete anything; proxies and keyframes live under `.derived/` and are regenerable.
- Does not sync the shelf to Immich or any external DAM; the cards are exportable if that day comes.
