# AgentFrame — Ready-Event Procedure

## Purpose

Own the generic quality and export gate that marks a deliverable good enough to use or share. `ready` is the ordinary completion state: small corrections may still land in place, while meaningful feedback starts a new version. `published` is separate and immutable.

## When To Load

Load when the operator signals that a deliverable is good enough to use/share or when an agent is about to run `af ready`.

## Procedure

1. Read the deliverable template and verify its readiness criteria.
2. Run any explicitly declared pre-ready quality gates, including a Humanizer pass when the template requires it.
3. For exportable deliverables, land approved finals inside the deliverable folder and record every path in `exports[]`. `af ready` refuses missing or dangling exports unless the operator explicitly approves the rare override.
4. Run `python system/af.py ready <project-slug> <deliverable-slug-or-path>`. The button updates artifact and tracker state together, runs any domain readiness hook, and records the event.
5. Work the printed checklist and surface unresolved follow-ups. If later feedback changes the deliverable materially, run `af version`; small corrections may update the ready head in place.

## Verification Or Logging

Run `python system/af.py doctor <project-slug>` after the transition. The artifact and tracker must both read `ready`, the pointer must name the numeric head, and required exports must resolve.

## Boundaries

- `ready` does not mean immutable and does not require a lock/unlock ceremony.
- `publish` is the only transition to immutable `published` state.
- Templates own type-specific readiness criteria; domain hooks own domain-specific assembly or verification.
