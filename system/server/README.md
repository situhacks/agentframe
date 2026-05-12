# AgentFrame Marketing Preview Server

Localhost preview server for visual deliverables. Renders mood-board direction compares, the design-language one-pager, and per-post carousel slides in your browser. LiveReload pushes refreshes as the agent edits files in Cursor.

Spec: [`docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/preview-server.md`](../../docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/preview-server.md). Visual flow it serves: [`runtime/in-app-generation.md`](../../docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/in-app-generation.md).

## 1. Install

From the repo root:

```
pip install -r system/server/requirements.txt
```

This installs `livereload`, `watchdog`, `PyYAML`, and `google-genai` (the last is for the Nano Banana image helper, not the server itself).

## 2. Run

```
python system/server/run.py
```

Defaults to `http://localhost:8080`. The whole project root is served, so any HTML file resolves naturally:

```
http://localhost:8080/workspace/campaigns/marketingos/phase-3-planning/design-language/preview/design-language.html
```

Useful flags:

- `--port 8081` if 8080 is taken (see Troubleshooting).
- `--host 0.0.0.0` to expose on the LAN (rare; default `localhost` is correct for solo work).
- `--campaign marketingos` opens the browser tab on that campaign's design-language preview.
- `--no-open` prevents the browser open even when `--campaign` is set.

Stop with `Ctrl+C`.

## 3. What gets watched

Watch globs come from [`system/server/config.yaml`](config.yaml). Defaults:

```
workspace/campaigns/*/phase-3-planning/design-language/**
workspace/campaigns/*/phase-4-production/posts/**/visuals/**
workspace/campaigns/*/phase-4-production/posts/**/video/**
workspace/campaigns/*/phase-4-production/posts/**/edit/**
```

Adjust `config.yaml` and restart if you need to widen the scope (e.g. while iterating on a brand new campaign whose folders don't match the glob shape yet).

## 4. tokens.css regeneration

Every time `tokens.yaml` changes, regenerate the CSS variables:

```
python -m system.server.lib.tokens_to_css workspace/campaigns/marketingos/phase-3-planning/design-language/tokens.yaml
```

Output goes to `./preview/assets/tokens.css` next to the yaml by default. Override with `--out PATH` if needed. The agent runs this automatically after editing tokens.yaml; the standalone CLI is for ad-hoc fix-ups.

## 5. Image generation (Nano Banana)

HTML is the primary visual medium for AgentFrame Marketing — slide HTML + tokens.css covers most of what carousel posts need. This helper exists for the cases where a real image is the right tool: mockups, textured backdrops, hero stills, or anywhere a flat colour block would feel cheap.

```
python -m system.server.lib.image_generate \
    --prompt-file path/to/prompt.txt \
    --negative-prompt "no text, no stock photo lighting" \
    --out-dir workspace/campaigns/marketingos/phase-4-production/posts/post-2/visuals \
    --aspect 4:5 --variants 3
```

Default model is `gemini-3.1-flash-image-preview` (Nano Banana 2). Use `--model pro` for `gemini-3-pro-image-preview` (Nano Banana Pro).

Auth uses `GEMINI_API_KEY` from root `.env` (or OS env override).

Nano Banana surface notes:

- One image per API call. The helper loops `--variants` times.
- Aspect ratio and negative prompt are not config fields on `generate_content`; the helper injects both into the prompt body before calling. Keep `--aspect` set to one of `1:1`, `4:5`, `3:4`, `9:16`, `16:9`, `4:3`.

Output:

- `image-variant-{a,b,c}.png` in `--out-dir`
- `_image_meta.json` sidecar with original prompt, composed prompt, negative prompt, model, aspect, refusal text (if any), file paths, and archive path when prior variants were moved
- Existing variants are moved to `visuals/history/` with a UTC timestamp before being overwritten

The agent is responsible for writing the human-readable `image-prompt.md` and `alt-text.md` alongside the variants.

## 6. Troubleshooting

| Symptom | Fix |
|--------|-----|
| `OSError: [WinError 10048]` or "address already in use" on start | Another process is on 8080. Stop it, or run `python system/server/run.py --port 8081` and update your browser tab. |
| Browser doesn't auto-refresh after a file save | Hard refresh (Ctrl+Shift+R). If LiveReload still doesn't fire, confirm your file matches a glob in `config.yaml` and that watchdog installed cleanly (`pip show watchdog`). |
| `watchdog` install fails on Windows | LiveReload will still serve files; you just lose auto-refresh. Restart Python after re-installing with `pip install --upgrade watchdog`. |
| Standalone HTML opened via `file://` doesn't refresh | Auto-injection only works for files served over the server. Use the helper to inject the script tag into the HTML at author time: `python -c "from system.server.lib.livereload_inject import inject_file; inject_file('path/to.html')"`. |
| Image call fails with "model not found" or "404" | Pass `--model nb2` or `--model pro`, or a current raw model id if Google rotates naming. |
| Image call fails with auth error | Confirm `GEMINI_API_KEY` is set in root `.env` or exported in your shell. |
| Image call returns text but no image | Nano Banana sometimes refuses with a text response instead of generating. The CLI logs how many calls refused and the model's text is captured under `refusals` in `_image_meta.json`. Reword the prompt to be more concrete (subject, surface, lighting) and retry. |
| `--aspect 4:5` produced something more square than expected | Nano Banana takes aspect intent as language, not a hard constraint. If shape matters, restate it in the prompt body (e.g. "a tall vertical poster") in addition to the flag. |
| Page CSS looks wrong | Check the link order: `preview-chrome.css` BEFORE the campaign's `tokens.css`. The chrome owns the page wrapper; tokens own the campaign visuals. |

## 7. Demo set

A self-contained demo lives at `system/server/static/demo/`. It exercises the three preview patterns (direction-compare, design-language one-pager, post-N stack) using stub data, so you can verify the server end-to-end before any real campaign reaches the visual phase.

```
http://localhost:8080/system/server/static/demo/index.html
```

Delete the `demo/` folder once you have a real campaign at sub-artifact 1+.

## 8. File map

```
system/server/
  run.py                         # entry point (CLI)
  config.yaml                    # port, host, watch globs
  requirements.txt
  README.md                      # this file
  lib/
    __init__.py
    server.py                    # build + serve helpers
    watcher.py                   # watch-glob loader
    livereload_inject.py         # script-tag inject for file:// fallbacks
    tokens_to_css.py             # tokens.yaml -> tokens.css
    image_generate.py            # Nano Banana variants + sidecar meta
    hub.py                       # / dashboard: scan workspace + render shell
  static/
    preview-chrome.css           # neutral page wrapper for all preview HTML
```

## 9. Hub (`/`)

`http://localhost:8080/` renders an auto-generated dashboard. Three columns: left sidebar renders each campaign as a generic folder tree; middle is a panzoom canvas hosting whichever artifact you click; right rail shows the post's `copy.md` text when a post group is active (collapses to zero width otherwise). Use it as a second-monitor viewer while you iterate in Cursor.

### Artifact kinds

Five kinds appear in the sidebar. All five are pan/zoomable in the same canvas with the same controls.

- **single** — one HTML file (e.g. `direction-compare`, `carousel-slide-2`). Renders as one iframe in the canvas. URL hash is the file's server path.
- **image** (`[img]` prefix) — a PNG / JPG / JPEG / WebP / GIF / SVG file. Renders as a native `<img>` at intrinsic size (`naturalWidth`/`naturalHeight`). No conversion, no dependencies.
- **pdf** (`[pdf]` prefix) — a PDF file. Rendered via [PDF.js v3.11](https://github.com/mozilla/pdf.js) into a horizontal canvas strip — one `<canvas>` per page at `scale: 2` (~retina-sharp). Iframing the PDF would hand control to the browser's PDF viewer and break Fit + pan, so we render it ourselves. PDF.js is loaded lazily from jsDelivr the first time you click a PDF entry; HTML/image-only sessions never download it.
- **video** (`[video]` prefix) — an MP4 / MOV / WebM file. Renders as a native `<video controls>` element so rendered video outputs can be reviewed beside the campaign artifacts.
- **group** (\u25a3 prefix) — a post directory containing `visuals/carousel-slide-*.html`. Renders as N slides side-by-side (each at native 1080x1350) in the canvas, plus the directory's `copy.md` text in the right rail. URL hash is `group:<dir-path>`. Group detection is HTML-only for now; image/PDF "carousels" are out of scope.

### Sidebar grouping

- Campaign root is open by default.
- Every nested folder is collapsed by default.
- Entries render at the leaf folder level using the same `data-artifact` payload contract as before.
- The only special-case grouping remains `post-*/visuals/carousel-slide-*.html` -> one `group` entry per post directory.

### Discovery rules

Zero curation, runs on every page load. The same extension list applies in all three sources:

```
# Singles, images, PDFs, and videos (anywhere under phase-*)
workspace/campaigns/{slug}/phase-*/**/*.{html,png,jpg,jpeg,webp,gif,svg,pdf,mp4,mov,webm}             -> Active
workspace/campaigns/completed/{slug}/phase-*/**/*.{html,png,jpg,jpeg,webp,gif,svg,pdf,mp4,mov,webm}   -> Completed
system/server/static/demo/**/*.{html,png,jpg,jpeg,webp,gif,svg,pdf,mp4,mov,webm}                       -> Demo

# Groups (post-N convention, applied uniformly to all three)
**/post-*/visuals/carousel-slide-*.html                  -> one group entry per post-* dir
**/post-*/copy.md                                        -> populates the right rail (optional)
```

A new campaign with files in those phase folders shows up automatically on the next refresh. New posts following the `post-{n}/visuals/` convention surface as group entries with no extra config. Image, PDF, and video labels keep their extension visible in the sidebar (`post-1-final.png`, `carousel-export.pdf`, `final.mp4`); HTML labels strip it.

#### Filtering noise

Default discovery now applies two filter layers before entries are shown in the sidebar:

- `exclude_globs` from `system/server/config.yaml` (for repeatable intermediate paths like `history/`, `raw/`, or `video/compositions/`)
- `.preview-hide` marker file in any folder (hides that folder and all descendants)

This keeps the hub focused on final-facing previews while preserving flexible campaign-specific control.

### Controls

- Click a sidebar entry — loads it in the canvas; sidebar stays put; URL hash updates (so back/forward and bookmarks work).
- Drag inside the canvas — pans the artwork.
- **Ctrl+wheel** — zooms (focal point under the cursor). Plain wheel falls through to native scroll.
- Toolbar buttons — `25 / 50 / 100 / Fit`. Fit is the default after every selection so the whole artifact is visible first. For a group or PDF, "Fit" frames the entire side-by-side strip.
- "Open in new tab" — pops the current artifact into a standalone tab. For groups, opens the first slide.
- "Show intermediates" — appears when filtered artifacts exist. Opens `/?intermediates=1` to temporarily include excluded and `.preview-hide` paths; use "Hide intermediates" to return to filtered view.

### Refresh behavior

- Edit a file shown in the canvas (e.g. `tokens.yaml` -> regenerate `tokens.css`) — the iframe content reloads via LiveReload; sidebar untouched.
- Add a *new* file or a new `post-{n}/` directory — refresh the hub page (Ctrl+R) to pick it up. The sidebar is rendered server-side at request time, not pushed live.

### Architectural rules

- Every iframe the hub mounts gets `scrolling="no"`. Inner scrollbars cannot appear regardless of slide author behavior; if content overflows its 1080x1350 frame, that's a slide-authoring bug to fix in the slide HTML, not a hub concern.
- Every static direct child of `#canvas-content` (iframe, img, canvas) gets `pointer-events: none` so the canvas div captures every drag/zoom event cleanly. Video artifacts are the exception: `<video controls>` keeps pointer events so the operator can play and scrub.
- Panzoom uses the library's default transform origin. The centering math in `applyZoom()` is written for that default; if the origin changes, update the centering math in the same patch.

**Libraries**: [@panzoom/panzoom 4.6.2](https://github.com/timmywil/panzoom) loaded from unpkg (~3.7 KB gzipped); [PDF.js 3.11.174](https://github.com/mozilla/pdf.js) loaded lazily from jsDelivr (UMD legacy build + worker file).
