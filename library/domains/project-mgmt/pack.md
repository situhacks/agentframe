---
domain: project-mgmt
prefix: pm
extension_fields: []
verbs: [lock, version, doctor]
flows: [open-flow]
---

# Project-management domain pack

Consulting / PM engagements. `domain: project-mgmt`. No posts, no publish, no assembly record — an engagement starts from a charter and derives everything from it.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | `extension_fields: []` + `prefix: pm` | this domain adds **no** fields to the neutral core (no MANIFEST, no post counters); the folder prefix is `pm-`. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | the neutral `project.md` body — IDENTITY + LIFECYCLE + DELIVERABLES + the two retro counters, nothing marketing. |
| Deliverable templates | [`deliverables/`](deliverables/) | the §2.7 mandatory core: `charter` (the seed source) + the four living governance docs (`raid-log`, `stakeholder-map`, `decision-log`, `workback-schedule`). |
| Verb applicability | `verbs` above | `lock`, `version`, `doctor` apply; **`publish` is not declared** (an engagement delivers, it does not publish a post). |
| Doctor rules | none | no `rules.py` — the generic core checks suffice; a PM-specific check is added only when real PM work earns one. |

## The mandatory core (provisional)

An engagement is seeded by a **charter / SOW** (normally an input you receive — its instance lives in the project's `sources/`, not as a produced deliverable). From it, the four living governance docs are derived at kickoff into `knowledge/`:

- `raid-log` — Risks / Assumptions / Issues / Dependencies, one owner per entry, weekly cadence, living.
- `stakeholder-map` — engagement-level relationship view; references the global people entities.
- `decision-log` — append-only, dated: decision + rationale + owner.
- `workback-schedule` — milestones / WBS planned backward from deadlines; ever-evolving.

No produced content deliverable is mandatory — findings, recommendations, decks, memos are instances of the generic deliverable shape, generated ad-hoc per the work. Default flow: `open-flow`. The kickoff derivation (charter into `sources/`, the four into `knowledge/`) is wired with the knowledge substrate (a later effort); this pack ships the templates.
