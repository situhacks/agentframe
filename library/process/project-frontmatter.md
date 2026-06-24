# Campaign Frontmatter Schema (canonical)

The frontmatter on `workspace/projects/{slug}/project.md` is the canonical state of a campaign. The agent reads frontmatter only (no full-body load) when answering state questions like "what's going on?" / "where am I?" — so it has to be cheap, consistent, and queryable.

**Schema version: 2026-04-23** (v2).

## Top-level shape

A v2 `project.md` frontmatter has four blocks. Each block has one job:

```yaml
---
# IDENTITY        — who/what this campaign is. Set once at scaffold; rarely changes.
# LIFECYCLE       — what state this campaign is in. Touched on phase transitions / ship / complete.
# MANIFEST        — which ingredient deliverables this campaign's posts assemble from. Set when campaign-architecture locks.
# DELIVERABLES    — per-deliverable tracker. The PRIMARY state-discovery surface for state-load reads.
# COUNTERS        — rollup counts derived from deliverables[]. Cheap state-summary.
---
```

Pointers (success criteria source, etc.) live inside the relevant blocks rather than as a top-level catch-all.

## Required fields by block

### IDENTITY (set at scaffold)

| Field | Type | Allowed values | Default | Notes |
|---|---|---|---|---|
| `name` | string | free text | — | Human-readable campaign name. Title-cased. Distinct from the slug folder name. |
| `slug` | string | folder-safe lowercase | — | The folder name. Required because frontmatter dumps are read without their paths. |
| `schema_version` | ISO date | e.g. `2026-04-23` | current schema version | Frozen at scaffold time. |
| `created_at` | ISO 8601 date | e.g. `2026-04-19` | scaffold date | When the campaign folder was created. |
| `supersedes` | string or `null` | optional, free text | `null` | If this campaign replaces a prior one (deleted or archived), name the prior. |

### LIFECYCLE (touched on phase transitions, ship, complete)

| Field | Type | Allowed values | Default | Notes |
|---|---|---|---|---|
| `status` | enum | `active`, `complete`, `cancelled` | `active` | Lifecycle state. The marketing PROCESS dictates campaign completion (post-campaign retros are the last process steps); the folder location is a side-effect of the status transition, not its own status value. `active` covers any in-progress phase. `complete` set by Campaign Retro lock when the campaign finished naturally. `cancelled` set when the operator (or external reviewer) decides to kill the campaign mid-flight. Both terminal. |
| `current_phase` | enum | flow-defined phase ids; for `open-flow`, the campaign-defined ids declared in the `project.md` plan section | first phase in selected flow | Where the campaign is right now. Updated when the agent finishes a phase's last deliverable or the user explicitly transitions. End-of-phase transition rules live in the selected `flow` file. |
| `flow` | enum | flow ids in [`flows/README.md`](flows/README.md) | `open-flow` | Canonical flow selector for this campaign instance. Valid values: `marketing-solo-flow`, `marketing-standard-flow`, `open-flow`. (See `library/process/flows/` for definitions). |
| `last_activity` | ISO 8601 datetime | e.g. `2026-04-23T03:00:00+00:00` | scaffold time | Touched whenever any deliverable in this campaign is edited / locked / delivered. Used to compute stale-campaign nudges (>7d). |
| `shipped_at` | ISO 8601 date or `null` | — | `null` | When the first post in the campaign published. (Sourced from the post's `post-FINAL.md` frontmatter `published.posted_at` — see [`post-final/template.md`](../deliverables/post-final/template.md) "Publish / Export Mechanics".) |
| `completed_at` | ISO 8601 date or `null` | — | `null` | When the campaign retro ran (the formal close — `LIFECYCLE.status` transitions `active → complete` in the same turn). |
| `cancelled_at` | ISO 8601 date or `null` | — | `null` | When `LIFECYCLE.status` was set to `cancelled`. Mutually exclusive with `completed_at`. |
| `cancelled_reason` | string or `null` | free text, single line | `null` | One-line reason for cancellation. |
| `quarterly_goals_advanced` | array of strings | references operator's quarterly goals | `[]` | Which quarterly goals this campaign serves. References goals declared in `library/context/operator/positioning.md` → "Current Quarter Goals". |

#### Cancellation (operator or external-review kill)

When the operator or reviewer kills the campaign:

1. Ask for a one-line cancellation reason.
2. Set `LIFECYCLE.status: cancelled`, `cancelled_at: {today}`, and `cancelled_reason: "{one-line}"`.
3. Append a `cancellation` event in `workspace/projects/{slug}/activity.md`.
4. Offer to move the folder to `workspace/projects/completed/{slug}/`.
5. Cancelled campaigns still run system retro; campaign retro is skipped (no delivered performance to score).

### MANIFEST (set when campaign-architecture locks)

Each post in the campaign is assembled from ingredient deliverables, each with its own version trail and lock, accumulating into the post's `post-FINAL.md` (see [`library/deliverables/post-final/template.md`](../deliverables/post-final/template.md)). The manifest names which ingredients this campaign's posts use, plus campaign-wide generation preferences:

```yaml
post_manifest:
  ingredients: [slide-copy, body-copy, image-prompts]   # per-post ingredient deliverables for this series
  notes: "prompts only — operator renders in Gemini"     # optional: campaign-wide generation/tooling preference
```

A single post can deviate (e.g. one video post in a carousel series): add `ingredients: [...]` to that post's tracker row. The manifest is the default, not a cage.

### DELIVERABLES (per-deliverable tracker — the primary state-discovery surface)

The `deliverables` block is a YAML map keyed by deliverable slug. Each row is one deliverable. The state-load reads this block first to answer "what exists, what state is it in, where do I look for the canonical content."

```yaml
deliverables:
  {deliverable-slug}:
    status: {enum}                    # required
    file: {path-from-campaign-root}   # required (can be a folder path if status: not_started)
    last_updated: {ISO 8601 date}     # required when status != not_started
    review: {enum}                    # required for deliverables whose template declares Review path: external
    expected_feedback_by: {ISO date or null}   # optional — only when review: pending
    job: {short-string}               # optional — useful for posts so the index reads at a glance
    framing_note: {free text}         # optional — short note when something material shifted (e.g. an angle reframe mid-campaign)
```

**Per-deliverable `status` enum** (use the same enum across all deliverable types):

| Value | Meaning | When to use |
|---|---|---|
| `not_started` | The deliverable has not yet been opened. The `file` may point at a folder rather than a file. **This value lives only in the campaign-tracker mirror** — per-deliverable file frontmatter never carries `not_started` (the file doesn't exist yet to carry frontmatter). | Default for new deliverables that the selected campaign flow says are expected at this phase. |
| `drafting` | Active work in progress. The `-v{N}.md` file exists with content. **`drafting` includes the in-flight-with-reviewer case** — the orthogonal `review` field carries the external-coordination state; the deliverable itself is still drafting until reviewer feedback is applied (or waived) and the operator locks. | Any state between `not_started` and `locked` / `delivered`. |
| `locked` | The deliverable is locked — no more substantive edits without an explicit "unlock" event. Downstream work depends on this state. Reached either directly from `drafting` (no reviewer) or via `drafting + review: complete` (reviewer feedback applied). | After lock-event skill fires for the deliverable. |
| `delivered` | Used for post rows once the post has actually been published. The delivered-state record lives in the post's `post-FINAL.md` frontmatter (`published.{platform,url,posted_at}` + `shipped_media[]`); performance metrics live in `phase-5-launch-and-learn/performance-data.csv`. There is no separate `published.md` file. | After publish coordination updates the post's `post-FINAL.md` frontmatter. |
| `deferred` | The deliverable was intentionally skipped or postponed. The reason lives in the deliverable's own `-v{N}.md` frontmatter (NOT here — this row just notes the state). | When the selected campaign flow expected the deliverable but operator + agent agreed to defer or skip with back-fill obligation. |

**Per-deliverable `review` enum** (only for deliverables whose template declares `Review path: external`):

| Value | Meaning |
|---|---|
| `not_required` | No review was ever expected (solo operator OR template doesn't have an external review path). **No override log.** This is the normal path, not exception-handling. |
| `pending` | Review was expected — covers BOTH "preparing to send" AND "in flight with reviewer." Stays `pending` until the reviewer finishes and feedback is applied (or waived). One value covers the whole window because the operationally-distinguishing event from the agent's perspective is "feedback received and applied" — anything before that is the same state. When `pending`, the deliverable row can also carry `expected_feedback_by` so PM reasons from the campaign's actual expectation rather than from a generic elapsed-time rule. |
| `complete` | Review cycle is done — reviewer feedback applied (or "looks good, no changes" returned). Deliverable is now eligible for `status: locked`. |
| `waived` | Review WAS expected (template + context said yes) but the operator chose to skip. **This still earns a `phase_override` log** because something expected was intentionally skipped. |

The retro templates use the `not_required` vs `waived` distinction: `not_required` is informational only; `waived` triggers criterion-honesty footnotes in the campaign retro. Review path defaults live in the selected campaign flow, with shared schema here.

**Orthogonality.** `status` and `review` are two independent axes — `status` is the operator's working state (am I drafting, locked, delivered?), `review` is the external-coordination state (is a reviewer involved, and where in the loop are we?). A deliverable at `status: drafting` + `review: pending` is a perfectly valid combination (working version exists, sent for review, waiting for feedback). Do not add a separate `status: in_review`; that would conflate the two axes.

### COUNTERS (rollup — derived from deliverables[])

| Field | Type | Default | Notes |
|---|---|---|---|
| `post_count` | integer | derived (count of `post-*` deliverables) | Total planned posts. Update when the arc adds or drops a post. |
| `posts_published` | integer | derived (count of `post-*` deliverables with `status: delivered`) | Cheap rollup so a state-load can answer "how many delivered?" without walking deliverables[]. |
| `system_retro_completed` | ISO 8601 date or `null` | `null` | Filled when `phase-5-launch-and-learn/system-retro-v{N}.md` lands at `status: locked`. |
| `closeout_retro_completed` | ISO 8601 date or `null` | `null` | Filled when `phase-5-launch-and-learn/campaign-retro-v{N}.md` lands at `status: locked`. |



## Example block (v2)

```yaml
---
# IDENTITY
name: "Agent Architecture POV"
slug: agent-architecture-pov
schema_version: 2026-04-23
created_at: 2026-04-19
supersedes: "workspace/projects/marketingos/ (deleted 2026-04-19)"

# LIFECYCLE
status: active
current_phase: 4-production
flow: marketing-standard-flow
last_activity: 2026-04-23T03:00:00+00:00
shipped_at: 2026-04-20
completed_at: null
cancelled_at: null
cancelled_reason: null
quarterly_goals_advanced: ["Q2-distribution", "Q2-narrative-consistency"]

# MANIFEST
post_manifest:
  ingredients: [slide-copy, body-copy, image-prompts]
  notes: "prompts only — operator renders in Gemini"

# DELIVERABLES (the primary state-discovery surface)
deliverables:
  business-brief:
    status: locked
    file: phase-2-strategy/business-brief/draft-v2.md
    last_updated: 2026-04-19
    review: not_required
  campaign-brief:
    status: locked
    file: phase-2-strategy/campaign-brief/draft-v1.md
    last_updated: 2026-04-19
    review: not_required
  campaign-architecture:
    status: locked
    file: phase-3-planning/campaign-architecture/draft-v3.md
    last_updated: 2026-04-20
    review: not_required
  design-language:
    status: locked
    file: phase-3-planning/design-language/design-language-v1.md
    last_updated: 2026-04-19
  post-1:
    status: delivered
    file: phase-4-production/posts/post-1/post-FINAL.md
    last_updated: 2026-04-20
    job: attention
  post-2:
    status: delivered
    file: phase-4-production/posts/post-2/post-FINAL.md
    last_updated: 2026-04-20
    job: thought-leadership-soft-CTA
    framing_note: "shifted 2026-04-20 from week-six diagnosis to self-evolving framing"
  post-3:
    status: drafting
    file: phase-4-production/posts/post-3-middle-ground/post-FINAL.md
    last_updated: 2026-04-21
    job: thought-leadership-middle-ground

# COUNTERS
post_count: 5
posts_published: 2
system_retro_completed: null
closeout_retro_completed: null
---
```

## Schema-drift check (the always-on guarantee)

**Every project-frontmatter load runs this check first** — not an opt-in lookup. Run `python system/af.py doctor <campaign-slug>` — it verifies required fields, enums, tracker-row validity, head pointers, and counters as code, and surfaces issues without ever auto-fixing.

Judgment that stays with the agent: peek locked rows for `back_filled: true` and surface inline; for `open-flow`, sanity-check `current_phase` against the plan section's declared phases; report drift with last-activity age and ask before fixing. Drift fixes are user-approved frontmatter edits logged to the campaign's `activity.md` as `frontmatter_manual_edit`.

## Activity event line shapes

`workspace/projects/{slug}/activity.md` is the canonical material-event log. Each entry is a single line.

**Timestamp:** prefix each line with `YYYY-MM-DD HH:MM` (local 24-hour time). 

Canonical shapes:

- **`phase_override`** — operator skipped or jumped a sequence step the selected flow expected.
  ```
  2026-05-12 14:05 — phase_override: skipped campaign-architecture; drafted post-1 copy directly. Reason: "trying a quick test, will back-fill if it works."
  ```
- **`post_published`** — a post canonical `-v{N}.md` reconciled to `status: delivered` after the operator confirmed the live URL.
  ```
  2026-05-12 09:30 — post_published: post-1 → https://www.linkedin.com/posts/{activity-id}
  ```
- **`cancellation`** — campaign moved to `LIFECYCLE.status: cancelled`.
  ```
  2026-05-12 16:20 — cancellation: reason "{one-line cancellation reason}"
  ```
- **`frontmatter_manual_edit`** — operator-approved drift fix from the schema-drift check above.
  ```
  2026-05-12 11:48 — frontmatter_manual_edit: corrected current_phase from 4-production to 5-launch-and-learn (drift fix).
  ```
- **`plan_revised`** — an open-flow campaign's plan changed (phase added/merged/dropped, deliverable added to or removed from scope).
  ```
  2026-06-12 10:20 — plan_revised: added phase-3-followup; parked the video teaser. Reason: "workshop feedback lands first."
  ```

When a flow file mentions appending an event (`post_published`, `phase_override`, `cancellation`, etc.), use these shapes. Skipping a required retro is logged as a `phase_override`; pattern of overrides surfaces at quarterly self-review.
