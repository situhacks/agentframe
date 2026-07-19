# Project Frontmatter Schema

`workspace/projects/{slug}/project.md` frontmatter is canonical project state. State-loads read it first, so keep it cheap and queryable.

**Schema version:** `2026-07-15` (v3). `schema_version` names the current format, not the project's creation date. Projects migrate forward with schema changes; `af doctor` rejects older shapes.

## Blocks

Each frontmatter block has one job:

| Block | Owns |
|---|---|
| `IDENTITY` | Project identity set at scaffold. |
| `LIFECYCLE` | Project state, active phase, activity timestamps, terminal state. |
| `MANIFEST` | Which post ingredients this project uses by default. |
| `DELIVERABLES` | Per-deliverable tracker; the primary state-discovery surface. |
| `AUTOMATIONS` | Optional pointers to standing project-attached automation contracts; absent until first use. |
| `COUNTERS` | Cheap rollups derived from deliverable rows. |

Pointers live inside the relevant block, not in a catch-all section. Do not move deliverable content into `project.md`.

## Required Fields

### Identity

| Field | Type / values | Notes |
|---|---|---|
| `name` | string | Human-readable project name. |
| `slug` | folder-safe slug | Must match the folder name. |
| `schema_version` | ISO date | Must equal the current schema version above. |
| `created_at` | ISO date | Scaffold date. |
| `supersedes` | string or `null` | Prior project this replaces, if any. |
| `domain` | `marketing`, `project-mgmt` | Active domain pack. |
| `parent` | project slug or `null` | Optional parent project. |
| `channels` | list of channel slugs | Must resolve under `library/context/channels/`. |
| `stakeholders` | list of person slugs | Must resolve under `library/context/people/`; project overlays live in `knowledge/people/`. |

Domain packs may require extra fields through `pack.md` `extension_fields`; `af doctor` validates them.

### Lifecycle

| Field | Type / values | Notes |
|---|---|---|
| `status` | `active`, `complete`, `cancelled` | Folder location is a side effect, not a status value. |
| `current_phase` | selected-flow phase id | `open-flow` may use project-defined phase ids from the body plan. |
| `flow` | `marketing-solo-flow`, `marketing-standard-flow`, `open-flow`, `project-mgmt-open-flow` | Flow selector; definitions live in `library/process/flows/`. |
| `last_activity` | ISO datetime | Touched whenever a deliverable changes state or content. |
| `last_consolidated` | ISO date or `null` | Stamped by [`project-consolidate`](../../system/skills/project-consolidate/SKILL.md); `af doctor` nudges when stale or logs bloat. |
| `shipped_at` | ISO date or `null` | First publish date, sourced from the delivered post. |
| `completed_at` | ISO date or `null` | Set when closeout completes. |
| `cancelled_at` | ISO date or `null` | Mutually exclusive with `completed_at`. |
| `cancelled_reason` | string or `null` | One-line reason. |
| `quarterly_goals_advanced` | list | References goals in `library/context/operator/positioning.md`. |
| `build_repo` | absolute path or `null` | Set when a phase turns technical and code lives in a separate external repo. Its presence routes to [`technical-build.md`](technical-build.md). Optional; absent on non-technical projects. |
| `build_graduated_at` | ISO date or `null` | Stamped when the external repo graduates to self-sufficiency and AgentFrame stops orchestrating it. `null` while the build is active. Optional. |

Cancellation sets `status: cancelled`, `cancelled_at`, `cancelled_reason`, appends a `cancellation` activity event, and offers to move the folder under `workspace/projects/completed/`. Cancelled projects still run system retro; project retro is skipped.

`build_repo` / `build_graduated_at` are optional and only appear on projects with a technical-build phase; `af doctor` tolerates their absence. Lifecycle and mechanics live in [`technical-build.md`](technical-build.md).

### Manifest

`post_manifest` is set when a project reaches a manifest moment: campaign architecture lock in structured marketing flows, or the open-flow plan revision that puts posts in scope.

```yaml
post_manifest:
  ingredients: [slide-copy, body-copy, image-prompts]
  notes: "prompts only - operator renders in Gemini"
```

A post can override the default with `ingredients: [...]` on its tracker row.

### Deliverables

Each row is one deliverable:

```yaml
deliverables:
  {deliverable-slug}:
    status: {enum}
    file: {path-from-project-root}
    last_updated: {ISO date}
    review: {enum}
    expected_feedback_by: {ISO date or null}
    job: {short-string}
    framing_note: {short-string}
```

`status` is required. `file` is required and may point at a folder only while `status: not_started`. `last_updated` is required once work begins.

`job` is the row's stable role description: written at row creation and changed only when the role or status changes. Working state, directives, and open questions live in the head file.

| `status` | Meaning |
|---|---|
| `not_started` | Tracker-only placeholder; no deliverable file exists yet. |
| `drafting` | Work exists and is not locked; includes in-flight external review. |
| `locked` | Substantive edits require an explicit unlock/version event. |
| `delivered` | Post is published; publish data lives in `post-FINAL.md`. |
| `deferred` | Intentionally skipped or postponed; reason lives in the deliverable frontmatter. |

`review` is orthogonal to `status` and is required only when a template declares external review:

| `review` | Meaning |
|---|---|
| `not_required` | No external review was expected; no override log. |
| `pending` | Review is expected or in flight; use `expected_feedback_by` when known. |
| `complete` | Reviewer feedback applied or explicitly returned clean. |
| `waived` | Expected review was skipped; log a `phase_override`. |

Do not add `status: in_review`; use `status: drafting` + `review: pending`.

**Row archiving.** The tracker is the working set. [`project-consolidate`](../../system/skills/project-consolidate/SKILL.md) may move delivered rows older than 30 days to `knowledge/_archive/deliverables-archive.md`, whose frontmatter is a top-level `deliverables:` map holding the rows verbatim. `af doctor` and `af publish` count tracker + archive. `locked` and `deferred` rows never archive.

### Automations

The optional `automations` mapping appears only after `af automation init`. Each row points to one living operational bundle:

```yaml
automations:
  email-intake:
    status: active
    file: automations/email-intake/automation.md
    deployment_id: client-email-intake-work
    last_updated: 2026-07-12T15:10:00-07:00
    job: "Route approved client emails into project intake"
```

`status` is `proposed | ready | active | paused | retired`. Project state records desired lifecycle; deployment heartbeat and run outcomes remain observed runtime state. Create and transition rows only through `python system/af.py automation`. Existing projects require no backfill.

### Counters

| Field | Notes |
|---|---|
| `post_count` | Planned post rows. |
| `posts_published` | Delivered post rows across tracker + archive. |
| `system_retro_completed` | Date the harvest/system retro locked, or `null`. |
| `closeout_retro_completed` | Date the campaign/project retro locked, or `null`. |

Counters are derived. Update them in the same turn as the source deliverable rows.

## Schema Drift Check

Every project-frontmatter load runs `python system/af.py doctor <project-slug>` first. It verifies the current schema version, required fields, enums, tracker rows, numeric head pointers, artifact frontmatter/status, counters, channels, stakeholders, and domain-pack extensions. Completed projects may not retain `not_started` or `drafting` rows. It surfaces issues and never auto-fixes.

Judgment that stays with the agent: peek locked rows for `back_filled: true`; for `open-flow`, sanity-check `current_phase` against the body plan; report drift with last-activity age and ask before fixing. Approved frontmatter fixes append `frontmatter_manual_edit` to `activity.md`.

## Activity Events

`activity.md` is the material-event audit trail — locks, deliveries, overrides, plan changes, retros, cancellations, and structural decisions — plus terse button-generated work pulses. It is a tracker, not a work journal.

Line shape (`af doctor` lints shape, never vocabulary): `{YYYY-MM-DD HH:MM} — {event_type}: {short result lead}; {resume-useful consequence, path, state change, or reason}`. `event_type` is a snake_case token; keep a line under ~200 characters. One line means one material event, not one chat turn. Split unrelated state changes into separate lines; do not split just to polish prose.

Self-check before appending: name the event type first. If the moment is not a lock, delivery, override, plan change, retro, cancellation, or structural decision, it gets no line.

Never log pre-lock iteration prose — draft feedback, prompt/copy/render churn. That trail lives in each version file's `changes_from_v{N}` per [deliverable-versioning](deliverable-versioning.md); the `af lock` activity line is the loop's one roll-up. The one mechanical exception: `af draft` and `af version` themselves append a single `artifact_drafted` / `artifact_versioned` pulse line so the calendar can derive worked days and work blocks. Buttons write pulses; agents never add pulse lines by hand.

### Attention Block

`activity.md` may open with an `## Attention` block directly under the title — the dashboard-facing shortlist the Workspace Dashboard reads. One checkbox bullet per open item:

```md
## Attention

- [ ] 2026-07-15 | due | Finish workshop preread
- [ ] 2026-07-18 | waiting | Client reply on [deck](phase-4-demo/demo-deck-v1.md)
```

`kind` is one of `due`, `waiting`, `meeting`, `decision`, `review`. The dashboard shows unchecked items only; check an item off when it resolves and log the resolution as a normal activity line. Waiting-on / next-action state that must survive a session gap belongs here as one bullet, not as an activity line. Governed projects keep the full record in `knowledge/raid-log.md` / `decision-log.md` / `workback-schedule.md` — Attention is only the shortlist.

Canonical event shapes:

- `phase_override: {deliverable} skipped; {what happened}. Reason: "{reason}"`
- `post_published: post-{n} shipped; {url}`
- `cancellation: project cancelled; reason "{one-line cancellation reason}"`
- `frontmatter_manual_edit: {field} corrected from {old} to {new}; {reason}.`
- `plan_revised: {short result lead}; {minimum useful consequence}. Reason: "{reason}"`
- `knowledge_consolidation: dream pass completed; {what changed}.`
- `build_started: {repo path}; stub written, build-log created.`
- `build_graduated: {repo path}; context compiled into repo; {one-line what shipped}.`

When a flow file says to append an event, use these shapes. Skipping a required retro is a `phase_override`; repeated overrides surface in quarterly self-review.
