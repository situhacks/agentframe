# Project Activity

## Purpose

`workspace/projects/{slug}/activity.md` is the terse material-event trail and the home of unresolved Attention items. It explains what happened when a later task needs rationale; it is not part of the default project-state load.

## When To Load

Load when appending a material event, reading or resolving an Attention item, answering a history/why question, or investigating a drift report that points to activity. Buttons append their own receipts without requiring the agent to load this file first.

## Procedure

### Attention

An optional `## Attention` block sits directly under the activity title. Keep one checkbox per unresolved item:

```md
## Attention

- [ ] 2026-07-15 | due | Finish workshop preread
- [ ] 2026-07-18 | waiting | Client reply on [deck](phase-4-demo/demo-deck-v1.md)
```

`kind` is `due`, `waiting`, `meeting`, `decision`, or `review`. Check the item when it resolves. Governed projects retain full risk/decision/schedule detail in `knowledge/`; Attention is only the shortlist.

### Material events

Use one line per event:

`{YYYY-MM-DD HH:MM} — {event_type}: {short result lead}; {resume-useful consequence, path, state change, or reason}`

Use a snake_case `event_type` and keep the line under about 200 characters. Log locks, deliveries, overrides, plan changes, retros, cancellations, consolidations, and structural decisions. Do not log chat turns or pre-lock iteration; version files own `changes_from_v{N}`.

Buttons write `artifact_drafted`, `artifact_versioned`, lock, publish, and other mechanical receipts. Agents do not duplicate them.

Canonical agent-written shapes:

- `phase_override: {deliverable} skipped; {what happened}. Reason: "{reason}"`
- `cancellation: project cancelled; reason "{one-line reason}"`
- `frontmatter_manual_edit: {field} corrected from {old} to {new}; {reason}.`
- `plan_revised: {short result lead}; {minimum useful consequence}. Reason: "{reason}"`
- `knowledge_consolidation: dream pass completed; {what changed}.`
- `build_started: {repo path}; stub written, build-log created.`
- `build_graduated: {repo path}; context compiled into repo; {one-line what shipped}.`

## Verification Or Logging

`af doctor` checks active-project event-line shape and length as non-blocking drift notes. The event itself is the log; do not mirror it into frontmatter.

## Boundaries

- Current project and deliverable state belongs in `project.md` frontmatter.
- Current deliverable content belongs in the deliverable head.
- Detailed risks, decisions, meetings, and schedules belong in `knowledge/`.
- Activity is not a work journal or a second tracker.
