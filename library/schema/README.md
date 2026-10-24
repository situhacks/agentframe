# `library/schema/`

The public shapes of the operator's private files. Each schema is the single source an agent generates or checks a private file against; nothing here holds the operator's content. The private files live under `workspace/life/`, and the chat registry under `system/sessions/`; both are gitignored and stay outside the tracked tree.

| Schema | Shapes the private file | Filled by |
|---|---|---|
| [`operator/`](operator/README.md) `positioning.md`, `profile.md`, `design-language.md` | `workspace/life/{surface}.md` | [`operator-context-setup.md`](../process/operator-context-setup.md) |
| [`operator/voice/`](operator/voice/README.md) | `workspace/life/voice/`; `af doctor` notes a method file that drifts from its mirror here | [`voice-setup.md`](../process/voice-setup.md), then `voice-harvest` |
| [`operator/career/`](operator/career/README.md) | `workspace/life/career/`, the career bank | [`career-harvest.md`](../process/career-harvest.md) |
| [`person-profile.md`](person-profile.md) | `workspace/life/people/{slug}.md` | closeout promotion in [`project-flow.md`](../process/project-flow.md) |
| [`channel-profile.md`](channel-profile.md) | `workspace/life/channels/{channel}/profile.md` | by hand when a channel first ships |
| [`life.md`](life.md) | `workspace/life/life.md`, injected into every main session | main and the consolidator, by hand; `af doctor` holds the cap |
| [`open-loops.md`](open-loops.md) | `workspace/life/open.md` and the month log in `workspace/life/log/` | `af open` |
| [`sessions-registry.md`](sessions-registry.md) | `system/sessions/registry.md` | `af session` and `af hook` |
| [`handoff.md`](handoff.md) | `system/sessions/<name>/brief.md` and `handoff.md` | main writes the brief; the chat writes its handoff |

Change a shape here, never in a copy. A link from a schema into `workspace/life/` points at a private file and does not resolve in a clean checkout.
