# AgentFrame — Project-Management Open Flow

Open flow for **long-horizon project-management engagements** that need a governance
spine. It is [`open-flow.md`](open-flow.md) plus one addition: a governance kickoff that
seeds the project's `knowledge/` substrate. Everything else — build-as-you-go phases,
versioned deliverables, the runway loop, the learning close — is inherited from open
flow unchanged.

Opt-in, never default. A one-off project-management deliverable (a single deck, a memo,
a slide redesign) uses plain `open-flow`; reach for this flow only when the engagement is
long enough that risks, decisions, stakeholders, and milestones need a durable home.

## Purpose

Run a sustained engagement where governance has to persist across many work sessions.
The flow front-loads a charter + living governance docs so the engagement stays
reviewable from disk, then rolls forward exactly like open flow.

## Phase Sequence

Phases are project-defined, same as open flow: the agent proposes a plan scaled to the
engagement, the operator narrows it, and `current_phase` starts at `active` (named
phases are added to the plan as the engagement takes shape). The governance kickoff is a
prelude to the first runway item, not a phase of its own.

## Deliverables By Phase

Two layers, kept separate:

- **Governance substrate (kickoff, lives in `knowledge/`, not the deliverable tracker).**
  Seeded once at kickoff, then maintained as living docs per
  [`knowledge-base.md`](../knowledge-base.md):
  - **Charter** — an *input*, not a produced deliverable. Ingest the SOW to `sources/`
    and register it in `sources/INDEX.md`. If no SOW exists, co-author a lightweight
    charter into `project.md` body. Template:
    [`charter`](../../domains/project-mgmt/deliverables/charter/template.md).
  - **`knowledge/raid-log.md`, `knowledge/decision-log.md`,
    `knowledge/stakeholder-map.md`, `knowledge/workback-schedule.md`** — derived from the
    charter, seeded structured (may start near-empty), then kept current by the ingest
    workflow. Templates under
    [`domains/project-mgmt/deliverables/`](../../domains/project-mgmt/deliverables/).
- **Produced deliverables (build-as-you-go, tracked + versioned).** Findings,
  recommendations, decks, memos — instances of the generic deliverable shape or any pack
  template, versioned and locked exactly as in open flow
  ([`deliverable-versioning.md`](../deliverable-versioning.md)).

## Tracker Updates

- Scaffold via `python system/af.py new-project <slug> --domain project-mgmt --flow project-mgmt-open-flow`; `current_phase` starts `active`.
- The four governance docs are **knowledge substrate, not tracker rows** — they live in
  `knowledge/` and do not appear in `project.md` `deliverables`. Only produced
  deliverables get tracker rows, moving `not_started → drafting → locked → delivered` in
  the same turn their files change.
- Schema and drift checks: [`project-frontmatter.md`](../project-frontmatter.md). State
  transitions are button-owned (`python system/af.py`); lock triggers per
  [`lock-event.md`](../lock-event.md).

## Overrides And Skips

- **Governance can be deferred.** If an engagement turns out lighter than expected, a
  governance doc may stay empty or be skipped — record the call in `activity.md`. The
  honest downgrade is switching the project to plain `open-flow`; log the `flow` change
  as a `plan_revised` event.
- **Charter without a SOW.** Co-author it into `project.md` body rather than blocking on
  a missing input.
- **Plan recalibration** carries over from open flow: phases can be added, merged, or
  dropped; log `plan_revised` to `activity.md`.

## Completion Criteria

Same two-step learning close as open flow — harvest retro (`system-retro`) → performance
capture + project retro → completion. `LIFECYCLE.status: complete` only after the project
retro locks or the operator records a closeout override in `activity.md`. At closeout,
run [`project-consolidate`](../../../system/skills/project-consolidate/SKILL.md) to
archive stale governance entries from `knowledge/`.

## Load Notes

- Inherits the loop, file conventions, and closeout from [`open-flow.md`](open-flow.md);
  read it for anything this file does not override.
- The `knowledge/` substrate schema and the sources→knowledge ingest workflow live in
  [`knowledge-base.md`](../knowledge-base.md); this flow only triggers the kickoff
  derivation, it does not restate the schema.
