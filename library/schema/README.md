# `library/schema/`

The public shapes of the operator's private files. Each schema is the single source an agent generates or checks a private file against; nothing here holds the operator's content. The private files live under `workspace/life/`, the chat registry under `system/sessions/`, and the backlog in `system/`; all are gitignored and stay outside the tracked tree.

| Schema | Shapes the private file | Filled by |
|---|---|---|
| [`operator/`](operator/README.md) `positioning.md`, `profile.md`, `design-language.md` | `workspace/life/{surface}.md` | [`operator-context-setup.md`](../process/operator-context-setup.md) |
| [`operator/voice/`](operator/voice/README.md) | `workspace/life/voice/`; `af doctor` notes a method file that drifts from its mirror here | [`voice-setup.md`](../process/voice-setup.md), then `voice-harvest` |
| [`operator/career/`](operator/career/README.md) | `workspace/life/career/`, the career bank | [`career-harvest.md`](../process/career-harvest.md) |
| [`project.md`](project.md) | `workspace/projects/<slug>/`: `project.md`, `activity.md`, `sources/`, `knowledge/`, versioned deliverables; `af doctor` reports a folder with no `project.md` | `af new-project`, then `af draft` and `af version` |
| [`person-profile.md`](person-profile.md) | `workspace/life/people/{slug}.md` for anyone he knows and `people/groups/{slug}.md` for a friend group | `af person add`, by whichever chat hears the name; the `filer`; closeout promotion in [`project-flow.md`](../process/project-flow.md) |
| [`place.md`](place.md) | `workspace/life/places/{slug}.md`: a venue, shop, service, or area, with his visits and verdicts | `af place` verbs, by whichever chat searches, books, or hears a reaction; the `researcher` adds candidates; the `filer` adds visits from bookings |
| [`tags.md`](tags.md) | `workspace/life/tags.yaml`, the vocabulary behind `tags:` on place and person pages, with `tags-log.md` and the rendered `tags.md` | `af tag` only: `match` before writing, `propose` for a new one, `merge`/`rename` to retire a form; the `consolidator` runs `af tag lint` weekly; `af doctor` flags drift |
| [`channel-profile.md`](channel-profile.md) | `workspace/life/channels/{channel}/profile.md` | by hand when a channel first ships |
| [`life.md`](life.md) | `workspace/life/life.md`, injected into every main session | by hand; `af doctor` holds the cap |
| [`open-loops.md`](open-loops.md), the one reminder mechanism and the loop rule every chat follows | `workspace/life/open.md` and the month log in `workspace/life/log/` | `af open add`, `close`, `redate`, `due`; `--every` makes a loop recur; `system/tools/push-due-loops.ps1` sends due loops to his phone |
| [`bills.md`](bills.md), the card statement ledger and its loop timing | `workspace/life/systems/finances/bills.md` | `system/tools/bills-watch.ps1` each morning from his card emails; he edits only the Cards list |
| [`loop-watch.md`](loop-watch.md), what the morning check of his due loops found in his email | `workspace/life/systems/loop-watch-report.md` | `system/tools/loop-watch.ps1` each morning; main reviews it, closes the loops itself, then runs `af open watch-reviewed` |
| [`loop-sweep.md`](loop-sweep.md), the morning sweep that closes loops on email proof and reports the rest | `workspace/life/systems/loop-sweep-report.md`, the overdue handoff, the private `discord/boards/open-private.txt`, and the generated `discord/boards/open.md` | `system/tools/loop-sweep.ps1` each morning after the loop watch, report-only unless registered `-Live`; main acts on the report |
| [`needs-list.md`](needs-list.md), what main needs the operator to decide | Marked lines in `workspace/life/open.md`, `workspace/life/needs/<slug>.md`, and the generated `workspace/life/needs.md` | `af needs`; main writes the detail file |
| [`source-record.md`](source-record.md) | A filed source in `workspace/life/sources/` or a project's `sources/`, and its `INDEX.md` row | the `filer` role, through `/file` |
| [`knowledge-page.md`](knowledge-page.md) | `workspace/life/knowledge/<slug>.md` and a project's compiled knowledge pages | the `filer` writes; the `consolidator` recompiles weekly |
| [`sessions-registry.md`](sessions-registry.md) | `system/sessions/registry.md` | `af session` and `af hook` |
| [`handoff.md`](handoff.md) | `system/sessions/<name>/brief.md` and `handoff.md` | the chat that starts it writes the brief; the chat writes its handoff |
| [`backlog.md`](backlog.md) | `system/backlog.md` (open rows) and `system/backlog-completed.md` (done rows) | any chat appends; `/upgrade` moves a row on merge |

Change a shape here, never in a copy. A link from a schema into `workspace/life/` points at a private file and does not resolve in a clean checkout.
