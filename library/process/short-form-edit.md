# Short-Form Edit

## Purpose

Footage-first production for a talking-head short: a recording comes in, a rendered 9:16 file goes out. Owns the order of operations, the edit-decision grammar, the caption path, and the creator-study procedure that feeds the hook bank. The render engine is the vendored HyperFrames; the transcript comes from the vendored local Parakeet tool.

Spec-first video (a campaign plans a video from a brief) stays with [`video-production.md`](video-production.md).

## When To Load

- A recording exists and needs to become a post (studio step 3 onward in `library/domains/creator/production.md`).
- A project post wants a quick cut of raw footage rather than a composed video.
- A creator study or hook extraction is requested.

Read first: the post's `post.md` and `edit.md`, the channel profile, and the `video/` component of the design language the profile names. Do not load `hyperframes-creative` house style; the design language is the only design input.

## Procedure

### 1. Ingest

Copy the recording to `video/source.mp4` if `af studio new --from` did not. Record `ffprobe` facts (duration, codec, dimensions, fps) in `post.md` notes. An HEVC source gets an H.264 proxy first; headless Chromium does not decode HEVC.

### 2. Transcribe

```
python system/tools/transcribe.py video/source.mp4 --out video/transcript.json
```

Output is `{text, words:[{text,start,end}]}`, the shape `transcript-cut.mjs` and the caption tools consume. Read the text once, whole.

### 3. Cut

Mark in `edit.md` what goes: retakes (he restarts a sentence), fillers, dead air over 0.8s, tangents that do not serve the hook. Compile and review before encoding:

```
node system/skills/hyperframes/source/skills/media-use/scripts/transcript-cut.mjs \
  --input video/source.mp4 --transcript video/transcript.json \
  --remove "12.4-15.0" --remove-fillers "um,uh" --cut-silence 0.8 --plan
```

Encode to `video/cut.mp4` once the kept segments read right. Re-transcribe the cut only if timings drift; otherwise offset the word times by the removed ranges.

### 4. Decide (the `edit.md` grammar)

Fill each section of `edit.md`:

- **Hook.** The first three seconds decide distribution. If the strongest line is not first, move it first. Under twelve spoken words; identity call, contrarian, open loop, confession, or question. Check `hooks.md` for what has retained before.
- **Punch-ins.** On emphasis words: scale 1.0 → 1.10–1.15 over 150–250 ms, hold, ease back on the next breath. Three to six per minute. Never on the hook's first frame unless the hook is the punch.
- **B-roll slots.** Wherever he names something visual, one to two seconds, then back to him. Query the shelf: `python system/af.py search "<what he names>"`, open the card, take `path` (or `proxy`) and pick a sub-window. Full-frame for places and objects; picture-in-picture when his face carries the beat. Append the post slug to the card's `used_in`.
- **Text cards.** The claim and any number, once each, in the design language's card style. Not a transcript.
- **SFX and bed.** From `library/assets/audio/` records only (licence known). A soft whoosh on punch-ins, a pop on cards, a riser into the CTA, a bed at −24 LUFS or lower with ducking under speech. Sparingly; silence is a choice.
- **Captions.** Verbatim, word-timed, in the identity the design language names. Drop fillers that survived the cut.
- **End card and CTA.** One line, one card, matching the charter's current ask.
- **Platform caption and tags.** Keyword-first first line (search is discovery), then the caption, then three to five tags from the channel profile's conventions.

### 5. Compose

One HyperFrames project per post under `video/`. Base track: `cut.mp4` as a `<video>` element; punch-ins are GSAP scale and translate keyframes on it. Cutaways: clips with `data-media-start` sub-windows, full-frame or picture-in-picture. Text cards: the `talking-head-recut` skill's card pattern, styled from `frame.md`. Captions per the caption path below. Audio: SFX and bed with the ducking keyframes `audio-duck.mjs` emits. Load `system/skills/hyperframes/SKILL.md` for the engine contract; read `frame.md` from the design language as brand truth.

### 6. Render and QC

```
npx hyperframes check
npx hyperframes render --out renders/final.mp4      # 1080x1920
ffmpeg -i renders/final.mp4 -af loudnorm=I=-16:TP=-1.5:LRA=11 -c:v copy renders/final-ln.mp4 \
  && mv -f renders/final-ln.mp4 renders/final.mp4
```

ffmpeg refuses to read and write the same file, so loudness normalization goes to a second name and replaces the render only on success; `-c:v copy` keeps the picture untouched.

Watch the first three seconds and the end card. Confirm captions sit inside the platform's safe zones, duration and size fit the channel profile, and nothing carries a watermark. Then `python system/af.py studio stage <slug> cut`. The operator watches before anything is scheduled.

### Caption path

In order of preference:

1. `embedded-captions` in the identity the design language names, `anchor` rail by default. Its prepare step mattes the subject; if that is too slow on this machine, stop and use 2.
2. A word-timed ASS track burned by ffmpeg in the identity's font, size and colours. No matting, no engine.

Record which path the post used in `edit.md` so the next post starts there.

### Creator study (feeds `hooks.md`)

For each creator on `research/whitelist.md`, pull a small recent set, never bulk:

```
yt-dlp --impersonate chrome --skip-download --flat-playlist --playlist-end 8 \
  --print "%(id)s|%(view_count)s|%(duration)s|%(title)s" "https://www.tiktok.com/@{handle}"
yt-dlp --impersonate chrome --write-subs --sub-langs "eng-US" --skip-download -o "research/{date}-{handle}/%(id)s" "<video url>"
```

Where a video ships no captions, download audio and transcribe with the local tool. To have the video *watched* (hook delivery, cuts, on-screen text, pacing), delegate perception to Gemini and keep the judgment here:

```
python system/tools/agy_call.py --file research/{date}-{handle}/{id}.mp4 --schema '{...}' \
  --prompt "Describe the first 3 seconds, list every cut with timestamps, quote on-screen text, name the caption style."
```

For each video record: the first-twelve-word hook and its pattern, structure and beat timing, length, caption style, and views relative to the creator's median in the set. Write the breakdown to `research/{date}-{handle}/study.md` and append hook rows to `hooks.md` with the URL. Requires `yt-dlp` 2026.08 or later with `curl_cffi`; if a read fails twice, record the gap rather than substituting a source.

## Verification Or Logging

- `edit.md` names the hook line, the caption path, every b-roll asset path, and the QC checks run.
- `renders/final.mp4` exists before `cut`; the operator has watched it before `scheduled`.
- `hooks.md` rows carry a source URL or `own` and, after capture, a result.
- No entry in `activity.md` per post; the board and the receipt are the record.

## Boundaries

- Does not own state transitions (`af studio`), the intake of reusable captures (`media-intake.md`), publishing mechanics (the channel's browser recipe), or performance capture (`composio-notes.md`).
- Does not maintain a parallel HyperFrames guide; the vendored skills own composition, animation and rendering detail.
- Does not automate anything on the platform.
