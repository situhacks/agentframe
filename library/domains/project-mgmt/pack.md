---
domain: project-mgmt
prefix: pm
extension_fields: []
verbs: [lock, version, doctor]
flows: [open-flow, project-mgmt-open-flow]
---

# Project-management domain pack

Consulting / PM engagements. `domain: project-mgmt`. No posts, no publish, no assembly record. A long-horizon engagement derives a governance spine from a charter via the `project-mgmt-open-flow` flow; a one-off PM deliverable (a single deck, memo, or redesign) uses plain `open-flow` and derives nothing.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | `extension_fields: []` + `prefix: pm` | this domain adds **no** fields to the neutral core (no MANIFEST, no post counters); the folder prefix is `pm-`. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | the neutral `project.md` body — IDENTITY + LIFECYCLE + DELIVERABLES + the two retro counters, nothing marketing. |
| Deliverable templates | [`deliverables/`](deliverables/) | the governance template set: `charter` (the seed source) + the four living governance docs (`raid-log`, `stakeholder-map`, `decision-log`, `workback-schedule`). Available, not auto-spawned — instantiation is flow-driven (see below). |
| Verb applicability | `verbs` above | `lock`, `version`, `doctor` apply; **`publish` is not declared** (an engagement delivers, it does not publish a post). |
| Doctor rules | none | no `rules.py` — the generic core checks suffice; a PM-specific check is added only when real PM work earns one. |

## The governance template set

A long-horizon engagement is seeded by a **charter / SOW** (normally an input you receive — its instance lives in the project's `sources/`, not as a produced deliverable). From it, four living governance docs are maintained in `knowledge/`:

- `raid-log` — Risks / Assumptions / Issues / Dependencies, one owner per entry, weekly cadence, living.
- `stakeholder-map` — engagement-level relationship view; references the global people entities.
- `decision-log` — append-only, dated: decision + rationale + owner.
- `workback-schedule` — milestones / WBS planned backward from deadlines; ever-evolving.

These are **available templates, not an auto-spawned core.** Instantiation depends on the project's `flow`:

- **`project-mgmt-open-flow`** (long-horizon, opt-in) derives the charter + four docs at kickoff. See [`project-mgmt-open-flow.md`](../../process/flows/project-mgmt-open-flow.md).
- **`open-flow`** (the default, including PM one-offs) derives none at kickoff; a governance doc is created on demand only when the ingest workflow has real evidence for it (a decision, a risk). See [`knowledge-base.md`](../../process/knowledge-base.md).

No produced content deliverable is mandatory — findings, recommendations, decks, memos are instances of the generic deliverable shape, generated ad-hoc per the work.
