# Workspace Dashboard Process

Lazy-load this file only when the operator explicitly asks for the dashboard, historical calendar, a browser preview, or a look at a deliverable/version/export without opening desktop apps.

## Previewable Types

Markdown, HTML, images (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.svg`), PDF, video (`.mp4`, `.mov`, `.webm`), and Office files (`.pptx`, `.docx` — converted server-side to PDF by the installed Microsoft Office desktop app via [`native-office-render.md`](native-office-render.md)).

Office preview requires PowerPoint/Word on the machine and has no LibreOffice fallback; a missing Office install surfaces the renderer's instructions in the viewer instead of a degraded render.

## Start Or Open — One Command, No Ceremony

```
python system/server/run.py --daemon --view dashboard
```

Idempotent start-or-open: reuses an already-running healthy surface (lock file + health check), otherwise starts one detached on a free port, then opens the requested view and prints its URL. Run it yourself when the environment allows instead of telling the operator to run something — the operator should never need a second manual step after asking for a preview.

Choose the destination from the request:

- Dashboard: `--view dashboard`
- Historical calendar: `--view calendar`
- Artifact: `--view preview --project {slug} --file {path-from-project-root}`

If the operator asks only for the preview server, default to Dashboard. Add `--no-open` when only a clickable URL is wanted.

## Stop

When the operator explicitly asks to close or stop the preview server, run:

```
python system/server/run.py --stop
```

Do not stop it automatically; the operator may still be using the open browser view.

## Surface Map

- **Dashboard tab** — Attention items (from each active project's `activity.md` `## Attention` block; convention in [`project-activity.md`](project-activity.md)), active-project table, recent activity. The in-app percentage is a content-density control: it scales table type, padding, keycaps, and section headers together while preserving section capacity, so lower percentages expose more rows. Browser zoom remains the outer accessibility/page-scale control. Deterministic file reads only; no LLM.
- **Calendar tab** — retrospective, career-narrative view of work over time, derived from active and completed project state. Four views: **Day/Week/Month** (FullCalendar, loaded from CDN) place project spans as ribbons and logged activity as synthesized work blocks; **Timeline** is the presentation-ready swimlane (creation-to-close/today duration bars) slimmed for many-project scale. Ribbons use a **ghost/solid** treatment — a faint track for the whole span, solid segments only on days with logged work — toggleable to a solid fill via `ghost inactive days`. Deliverables collapse to hover-to-preview dots; toggle `expand labels` to show them as chips. Filter projects at left with the `all projects` / `active only` / `completed only` / `none` scope dropdown or individual checkboxes; manual checkbox combinations read as `custom`. Month weeks size to their visible ribbon lanes and the month scrolls, so busy weeks remain isolated at enlarged browser zoom. The legend has its own remembered hide/show state. `export PNG` lazily loads pinned `html-to-image` and captures the complete current calendar surface at its current width, expanding inner scroll containers and capping output near 24 megapixels for PowerPoint-friendly handoff. Hover markers for detail; click a deliverable to open it in Preview. View, focus date, toggle state, filter collapse, and legend visibility persist in the URL hash (`?view=`, `?date=`, `?ghost=`, `?expand=`, `?filters=`, `?legend=`).
- **Preview tab** — active/completed/all project filters over a media-first artifact index built from `project.md` tracker rows (+ archived rows), manifest media, exports, and untracked previewable files. The rail defaults to active projects and `media`; `text` shows `.md`/`.txt`; `all` restores grouped deliverables with current file, previous versions, manifest media, and exports behind the expand control. Ordinary rail clicks reuse one preview tab so browsing many versions does not accumulate live iframes; **Open to Side** and **Open in New Tab** create durable comparison panels. Viewer toolbars adapt to each split's width: the path and secondary actions collapse before the bar can overflow, with the same actions retained in the context menu. When a project's design language names a `storybook:` file that exists, a pinned **Design** entry sits at the top of the rail — `media` opens the storybook HTML, `all` expands the whole design-language folder. Absent a storybook, no Design pin appears.
- Legacy file-tree hub remains at `http://localhost:8080/hub`.

## Hide Noise

- `sources/`, `knowledge/`, `references/`, `archive/`, `backup/`, `history/`, `raw/`, `stills/`, `node_modules/` never appear in the untracked list.
- Drop a zero-byte `.preview-hide` file in any folder to hide it and its descendants (the Preview rail context menu can write it after confirmation).
- `exclude_globs:` in `system/server/config.yaml` additionally filters the legacy hub.
