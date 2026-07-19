# Project State Index

## Purpose

`workspace/projects/{slug}/project.md` frontmatter is the compact index a fresh agent reads before opening project detail. It answers four questions: what project is this, which domain and flow route it, what state is current, and which file is the current head of each deliverable.

The index is not a history log or a substitute for deliverable content. A small amount of duplication is deliberate: tracker `status` and `file` let an agent reconstruct the working set without scanning folders, and `af doctor` keeps that cache synchronized with each head file.

**Schema version:** `2026-07-19` (v4). Projects migrate forward; `af doctor` rejects older live shapes.

## When To Load

Load for project state, continuity, dependency, routing, and next-action decisions, and before changing project frontmatter. Do not load prior versions or full activity history unless the task asks what changed, why a decision was made, or how an artifact evolved.

## Procedure

1. Read `project.md` frontmatter and run `python system/af.py doctor <project-slug>`.
2. Use `domain`, `flow`, `current_phase`, and the deliverable tracker to route the task.
3. Read the `project.md` body when onboarding into the project or when its thesis, plan, or open project-level notes affect the decision.
4. Follow only the relevant `deliverables.{slug}.file` pointer. That head is canonical content; lower-numbered versions are history.
5. Read [`project-activity.md`](project-activity.md) only for event rationale, unresolved Attention items, or an activity write.

## Schema

### Required core

| Field | Job |
|---|---|
| `name` | Human-readable project identity used by agents and surfaces. |
| `slug` | Stable CLI/folder identity; must match the folder. |
| `schema_version` | Deterministic migration and doctor contract. |
| `created_at` | Stable creation date used for age and ordering. |
| `domain` | Resolves the domain pack and deliverable templates. |
| `status` | Project lifecycle: `active`, `complete`, or `cancelled`. |
| `current_phase` | Current position in the selected flow; `open-flow` may use a project-defined phase id from the body plan. |
| `flow` | Resolves the phase map under [`flows/`](flows/README.md). |
| `last_activity` | Cheap project-freshness signal; buttons update it with state/content work. |
| `deliverables` | Current working-set map; `{}` is valid before the first deliverable exists. |

`status: complete` requires `completed_at`. `status: cancelled` requires `cancelled_at`. Active projects carry neither terminal timestamp. Cancellation reason belongs in the `cancellation` event, not a second frontmatter copy.

### Optional only when used

| Field | Add when |
|---|---|
| `channels` | The project names global channel profiles that future work should load. |
| `stakeholders` | The project names global people profiles with project overlays. |
| `last_consolidated` | The first [`project-consolidate`](../../system/skills/project-consolidate/SKILL.md) pass runs. |
| `shipped_at` | A domain needs the project's first ship date as a summary/index value. |
| `completed_at` / `cancelled_at` | The matching terminal transition occurs. |
| `quarterly_goals_advanced` | The project actively references current positioning goals. |
| `build_repo` / `build_graduated_at` | [`technical-build.md`](technical-build.md) owns an external build. |
| `automations` | `af automation init` creates the first project-attached automation pointer. |

Domain packs may add current routing or state fields through `pack.md`. Marketing adds `post_manifest` when a real manifest moment occurs; it is absent for marketing work with no post manifest.

Do not seed optional fields with empty lists or `null`. Absence means the state has not occurred or the capability is not in use.

### Deliverable tracker rows

```yaml
deliverables:
  {deliverable-slug}:
    status: {not_started | drafting | locked | delivered | deferred}
    file: {path-from-project-root}
    last_updated: {ISO date}
    job: {short current role}
    review: {not_required | pending | complete | waived}
    expected_feedback_by: {ISO date}
```

`status` and `file` are required. A `not_started` row may point at the planned numeric-v1 path before the file exists. Once work begins, `last_updated` is required and the pointed head file must exist with the same status. `job` is optional and stays short: it explains the deliverable's current role, not its revision history. Review fields appear only when external review applies.

Working directives, open questions, deferral reasons, publish data, exports, and version history live in the head deliverable or its prior versions. Do not add those to the tracker.

Delivered rows older than 30 days may move to `knowledge/_archive/deliverables-archive.md` through `project-consolidate`. The archive preserves row shape so domain code can derive all-time facts without frontmatter counters.

## Verification Or Logging

`af doctor` validates schema version, required fields, lifecycle timestamps, domain/flow resolution, tracker rows, numeric head pointers, artifact status, optional channel/stakeholder pointers, and domain extensions. It reports drift and never auto-fixes.

An approved manual correction appends `frontmatter_manual_edit` using [`project-activity.md`](project-activity.md). Button-owned transitions write their own state and activity receipts.

## Boundaries

- `project.md` body owns thesis, plan, directory, and open project-level notes.
- Deliverable heads own current work; prior versions own their evolution trail.
- `activity.md` owns material events and unresolved Attention items.
- `knowledge/` owns distilled operational detail.
- Frontmatter does not store historical tombstones or counters derivable from deliverable rows.
