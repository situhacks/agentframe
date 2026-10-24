# `library/schema/`

The public shapes of the operator's private files. Each schema is the single source an agent generates or checks a private file against; nothing here holds the operator's content. The private files live under `library/context/`, which is gitignored and stays outside the tracked tree.

| Schema | Shapes the private file | Filled by |
|---|---|---|
| [`operator/`](operator/README.md) `positioning.md`, `profile.md`, `design-language.md` | `library/context/operator/{surface}.md` | [`operator-context-setup.md`](../process/operator-context-setup.md) |
| [`operator/voice/`](operator/voice/README.md) | `library/context/operator/voice/`; `af doctor` notes a method file that drifts from its mirror here | [`voice-setup.md`](../process/voice-setup.md), then `voice-harvest` |
| [`operator/career/`](operator/career/README.md) | `library/context/operator/career/`, the career bank | [`career-harvest.md`](../process/career-harvest.md) |
| [`person-profile.md`](person-profile.md) | `library/context/people/{slug}/profile.md` | closeout promotion in [`project-flow.md`](../process/project-flow.md) |
| [`channel-profile.md`](channel-profile.md) | `library/context/channels/{channel}/profile.md` | by hand when a channel first ships |

Change a shape here, never in a copy. A link from a schema into `library/context/` points at a private file and does not resolve in a clean checkout.
