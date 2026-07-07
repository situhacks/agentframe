# Project Frontmatter Schema

The frontmatter on `workspace/projects/{slug}/project.md` is the canonical state of a project. State-loads read frontmatter first, so it must stay cheap, consistent, and queryable.

**Schema version:** `2026-04-23` (v2).

## Blocks

Each v2 frontmatter block has one job:

| Block | Owns |
|---|---|
| `IDENTITY` | Project identity set at scaffold. |
| `LIFECYCLE` | Project state, active phase, activity timestamps, terminal state. |
| `MANIFEST` | Which post ingredients this project uses by default. |
| `DELIVERABLES` | Per-deliverable tracker; the primary state-discovery surface. |
| `COUNTERS` | Cheap rollups derived from deliverable rows. |

Pointers live inside the relevant block, not in a catch-all section. Do not move deliverable content into `project.md`.

## Required Fields

### Identity

| Field | Type / values | Notes |
|---|---|---|
| `name` | string | Human-readable project name. |
| `slug` | folder-safe slug | Must match the folder name. |
| `schema_version` | ISO date | Frozen at scaffold time. |
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

Cancellation sets `status: cancelled`, `cancelled_at`, `cancelled_reason`, appends a `cancellation` activity event, and offers to move the folder under `workspace/projects/completed/`. Cancelled projects still run system retro; project retro is skipped.

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

### Counters

| Field | Notes |
|---|---|
| `post_count` | Planned post rows. |
| `posts_published` | Delivered post rows across tracker + archive. |
| `system_retro_completed` | Date the harvest/system retro locked, or `null`. |
| `closeout_retro_completed` | Date the campaign/project retro locked, or `null`. |

Counters are derived. Update them in the same turn as the source deliverable rows.

## Schema Drift Check

Every project-frontmatter load runs `python system/af.py doctor <project-slug>` first. It verifies required fields, enums, tracker rows, head pointers, counters, channels, stakeholders, and domain-pack extensions. It surfaces issues and never auto-fixes.

Judgment that stays with the agent: peek locked rows for `back_filled: true`; for `open-flow`, sanity-check `current_phase` against the body plan; report drift with last-activity age and ask before fixing. Approved frontmatter fixes append `frontmatter_manual_edit` to `activity.md`.

## Activity Events

`activity.md` is the material-event log. Each entry is one line prefixed with local `YYYY-MM-DD HH:MM`.

Canonical event shapes:

- `phase_override: skipped {deliverable}; {what happened}. Reason: "{reason}"`
- `post_published: post-{n} -> {url}`
- `cancellation: reason "{one-line cancellation reason}"`
- `frontmatter_manual_edit: corrected {field} from {old} to {new} ({reason}).`
- `plan_revised: {what changed}. Reason: "{reason}"`
- `knowledge_consolidation: dream pass; {what changed}.`

When a flow file says to append an event, use these shapes. Skipping a required retro is a `phase_override`; repeated overrides surface in quarterly self-review.
