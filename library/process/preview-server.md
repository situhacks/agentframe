# Workspace Dashboard Process

Lazy-load this file when a turn writes a previewable artifact under `workspace/projects/`, or when the operator asks for the dashboard, historical calendar, a preview, or a look at any deliverable/version/export without opening desktop apps.

## Previewable Types

Markdown, HTML, images (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.svg`), PDF, video (`.mp4`, `.mov`, `.webm`), and Office files (`.pptx`, `.docx` — converted server-side; requires LibreOffice installed).

## Start Or Open — One Command, No Ceremony

```
python system/server/run.py --daemon
```

Idempotent start-or-open: reuses an already-running healthy surface (lock file + health check), otherwise starts one detached on a free port, then opens the browser and prints the URL. Run it yourself when the environment allows instead of telling the operator to run something — the operator should never need a second manual step after asking for a preview.

## Offer Trigger

After the first previewable artifact write in a turn, run the start-or-open command (or, if you cannot execute commands, give the URLs) once for that turn:

1. Deep link with the artifact loaded: `http://localhost:8080/#/preview?project={slug}&file={path-from-project-root}`
2. Dashboard: `http://localhost:8080/`
3. Historical calendar: `http://localhost:8080/#/calendar`

Do not repeat the offer after every file edit in the same turn. Add `--no-open` to suppress the browser tab when only the URL is wanted.

## Surface Map

- **Dashboard tab** — Attention items (from each active project's `activity.md` `## Attention` block; convention in [`project-frontmatter.md`](project-frontmatter.md)), active-project table, recent activity. Deterministic file reads only; no LLM.
- **Calendar tab** — retrospective, career-narrative view of work over time, derived from active and completed project state. Four views: **Day/Week/Month** (FullCalendar, loaded from CDN) place project spans as ribbons and logged activity as synthesized work blocks; **Timeline** is the presentation-ready swimlane (creation-to-close/today duration bars) slimmed for many-project scale. Ribbons use a **ghost/solid** treatment — a faint track for the whole span, solid segments only on days with logged work — toggleable to a solid fill via `ghost inactive days`. Deliverables collapse to hover-to-preview dots; toggle `expand labels` to show them as chips. Filter projects at left (active-first, with `all · active · completed · none` shortcuts); hover markers for detail; click a deliverable to open it in Preview; `print / PDF` for a coach or leadership handoff. View, focus date, and toggle state persist in the URL hash (`?view=`, `?date=`, `?ghost=`, `?expand=`).
- **Preview tab** — active/completed/all project filters over a media-first artifact index built from `project.md` tracker rows (+ archived rows), manifest media, exports, and untracked previewable files. The rail defaults to active projects and `media`; `text` shows `.md`/`.txt`; `all` restores grouped deliverables with current file, previous versions, manifest media, and exports behind the expand control. Ordinary rail clicks reuse one preview tab so browsing many versions does not accumulate live iframes; **Open to Side** and **Open in New Tab** create durable comparison panels. When a project's design language names a `storybook:` file that exists, a pinned **Design** entry sits at the top of the rail — `media` opens the storybook HTML, `all` expands the whole design-language folder. Absent a storybook, no Design pin appears.
- Legacy file-tree hub remains at `http://localhost:8080/hub`.

## Hide Noise

- `sources/`, `knowledge/`, `references/`, `archive/`, `backup/`, `history/`, `raw/`, `stills/`, `node_modules/` never appear in the untracked list.
- Drop a zero-byte `.preview-hide` file in any folder to hide it and its descendants (the Preview rail context menu can write it after confirmation).
- `exclude_globs:` in `system/server/config.yaml` additionally filters the legacy hub.
