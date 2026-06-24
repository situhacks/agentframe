# AgentFrame Marketing — Standard Campaign Flow

This is the fuller multi-post, stakeholder-capable campaign flow. Solo campaigns should start from `marketing-solo-flow.md`.

Each deliverable's full template lives in `library/deliverables/{type}/template.md`. This file is the map; templates are the territory.

## Read Once

- State transitions are button-owned: `python system/af.py` (`lock`, `publish`, `version`, `new-campaign`, `doctor`) does the mechanics atomically and prints the judgment checklist. Never hand-edit a terminal `status:`.
- Versioning conventions and the surgical-vs-replacement judgment are owned by [`deliverable-versioning.md`](../deliverable-versioning.md); lock triggers by [`lock-event.md`](../lock-event.md).
- Campaign tracker schema and review-state enums are owned by [`library/process/project-frontmatter.md`](../project-frontmatter.md).
- In every phase, apply file edits and `project.md` tracker updates in the same turn.

## Phase 1 — Research

Owned by the agent in-session when the operator starts a detailed campaign.

| Step | Output | Owner |
|---|---|---|
| 1.1 Campaign idea bank | `phase-1-research/idea-bank.md` | Agent + user |
| 1.2 Research artifact | `phase-1-research/research-artifact-v{N}.md` | Agent + user |

Detailed campaigns start from an operator-launched research pass. Connected workspace context can inform candidate ideas when available, but it is optional.

Load [`library/process/research-and-signals.md`](../research-and-signals.md) for the workspace-context definition, the live Composio/Rube MCP scan procedure, the Gemini Deep Research API vs web-handoff offer, and the fallback rule.

Idea-bank shape (keep tight): a candidate list plus the selected pick, nothing more. Per candidate: title, 1-3 sentence thesis, and one provenance line. Name the selected pick in one line. Do not add per-candidate Risk, Research Questions, Workspace Signal Summary, or Next Research Step sections.

Save research output as `phase-1-research/research-artifact-v{N}.md` at `status: drafting`; only operator acceptance locks it. Scaffold the campaign folder with `python system/af.py new-campaign <slug> --flow marketing-standard-flow` (schema-true `project.md`, `activity.md`, `feedback-log.md`).

**Tracker update at end of Phase 1:** `current_phase: 2-strategy`. Add to `deliverables`:
```yaml
research-artifact:
  status: locked
  file: phase-1-research/research-artifact-v{N}.md
  last_updated: {date accepted}
```
This transition happens only after the operator approves the research artifact.

## Phase 2 — Strategy

Two deliverables. Both can be reviewed externally OR drafted-through-to-lock — neither path is "exception-handling." Default behavior depends on whether a stakeholder exists for this campaign (see "Review path defaults" below).

| Step | Deliverable | Produces | Review path | Blocks |
|---|---|---|---|---|
| 2.1 | Business Brief | `phase-2-strategy/business-brief/draft-v{N}.md` | External when a stakeholder exists; otherwise drafting → lock | 2.2 cannot start until 2.1 is locked (with or without review) |
| 2.2 | Campaign Brief | `phase-2-strategy/campaign-brief/draft-v{N}.md` | External when a stakeholder exists; otherwise drafting → lock | Phase 3 cannot start until 2.2 is locked (with or without review) |

When external review IS the path: agent offers to draft the Gmail + Calendar invite when the deliverable hits export (see `runtime/review-coordination.md`). `review: pending` while the brief is in flight with the reviewer, and `expected_feedback_by` should be recorded when the user has a real expectation date. User sends; reviewer responds; user pastes feedback into chat; agent applies revisions; agent flips `review: complete`. Operator then locks. If the reviewer returns "kill it", follow the cancellation rule in [`library/process/project-frontmatter.md`](../project-frontmatter.md).

When external review is NOT the path: brief drafts, gets locked, downstream unblocks. No `phase_override` log — review never being expected is not an override.

**Tracker update at end of Phase 2:** `current_phase: 3-planning`. Add to `deliverables`:
```yaml
business-brief:
  status: locked
  file: phase-2-strategy/business-brief/draft-v{N}.md
  last_updated: {date locked}
  review: {not_required | complete | waived}
campaign-brief:
  status: locked
  file: phase-2-strategy/campaign-brief/draft-v{N}.md
  last_updated: {date locked}
  review: {not_required | complete | waived}
```
Success criteria from the locked Business Brief stay IN the brief (the brief is the canonical source). `project.md` does not duplicate them.

## Phase 3 — Planning

Three deliverables. Internal — not stakeholder-facing. Outputs prep for production.

| Step | Deliverable | Produces | Depends on |
|---|---|---|---|
| 3.1 | Campaign Architecture | `phase-3-planning/campaign-architecture/draft-v{N}.md` | 2.2 locked |
| 3.2 | Design Language ([template](../../deliverables/design-language/template.md)) | `phase-3-planning/design-language/design-language-v{N}.md` (+ `tokens.yaml`, `tokens.css`, `preview/`) | 2.2 locked |

3.1 and 3.2 can run in parallel after 2.2 locks.

Hard rule for multi-post campaigns: the per-post breakdown in Campaign Architecture (3.1) must cover ALL posts BEFORE Phase 4 begins. Posts in a series talk to each other; copywriter can't honor cross-post callbacks if it can't see them.

**Tracker update at end of Phase 3:** `current_phase: 4-production`. Add (or update) in `deliverables`:
```yaml
campaign-architecture:
  status: locked
  file: phase-3-planning/campaign-architecture/draft-v{N}.md
  last_updated: {date locked}
design-language:
  status: locked
  file: phase-3-planning/design-language/design-language-v{N}.md
  last_updated: {date locked}
```
Record `post_manifest` (ingredients + generation preferences from the locked Campaign Architecture) in `project.md` in the same turn — schema in [`project-frontmatter.md`](../project-frontmatter.md). Also add one row per planned post (`post-1`, `post-2`, …) at `status: not_started` with a folder pointer. The Phase 4 work fills the per-post ingredient files in.

## Phase 4 — Production and Launch

Per-post deliverables. Run in parallel after Phase 3 completes. Phase 4 owns launch execution for each post; Phase 5 is for post-launch learning after the active production arc closes.

When a production deliverable has many unresolved directions, multi-session scope, or risky edits across canonical artifacts, offer a scratchpad in the post folder.

**Scratchpads (recommended for first-version posts and major version bumps):** write `phase-4-production/posts/post-{n}/scratchpad-v{N}.md`, where `N` matches the version it informed. Scratchpads are throwaway planning notes: never read a prior-version scratchpad when working on a later version.

| Step | Deliverable | Produces | Depends on |
|---|---|---|---|
| 4.1 | Post ingredients (per post) — each ingredient named by `project.md` `post_manifest` (e.g. slide-copy, body-copy, image-prompts, video-spec), each with its own version trail and lock | `phase-4-production/posts/post-{n}/{ingredient}-v{N}.md` | 3.1 locked, 3.2 locked |
| 4.2 | Post assembly — `post-FINAL.md` accumulates each ingredient as it locks, per [`post-final/template.md`](../../deliverables/post-final/template.md) | `phase-4-production/posts/post-{n}/post-FINAL.md` | created when the post's first ingredient starts drafting |
| 4.3 | Publish coordination + media reconciliation (per post) | publish block in `post-FINAL.md` frontmatter + `activity.md` `post_published` entry | all manifest ingredients locked and operator confirms the live URL |

Ingredient order within a post: slide copy locks before body copy drafts (the body diverges from the deck, so the deck has to exist first); image prompts consume the design language's treatment block and the locked slide text.

External review (only when the post goes to a leadership stakeholder before publish — same default rule as Phase 2): agent offers to coordinate. Per-post.

The cross-ingredient coherence check (body doesn't retell the slides, cover aligns with the hook, CTA placement) runs at `post-FINAL.md` lock per [`post-final/template.md`](../../deliverables/post-final/template.md); video posts also run the cross-check in [`video-spec/template.md`](../../deliverables/video-spec/template.md).

When all ingredients are locked and publish media exists or has been selected, follow the publish-prep procedure in [`composio-notes.md`](../composio-notes.md).

**Tracker update during Phase 4** (per post, in the same turn as the file edit):
- Post starts drafting: `deliverables.post-{n}.status: drafting` + `file: phase-4-production/posts/post-{n}/post-FINAL.md` + `last_updated`, creating the `post-FINAL.md` stub in the same turn.
- An ingredient locks: its content lands in `post-FINAL.md` per [`lock-event.md`](../lock-event.md). The post row stays `drafting` until every manifest ingredient is in, then flips `locked`.
- Post publishes (Phase 4.3): `af publish` owns the delivered state — publish block in `post-FINAL.md`, tracker, `posts_published`, lifecycle `shipped_at` — per [`post-final/template.md`](../../deliverables/post-final/template.md) "Publish / Export Mechanics".
- Arc changes mid-campaign (post added, post dropped, post renumbered): update `post_count` AND the affected `post-{n}` rows in the same turn. Optional: add `framing_note` if the post's job in the arc shifted.

**Tracker update at end of Phase 4:** `current_phase: 5-launch-and-learn`. Set when every active post is `delivered`, `cancelled`, or explicitly removed from active campaign scope. Do not advance the whole campaign to Phase 5 on first ship; multi-post campaigns can publish early posts while later posts remain in production.

## Phase 5 — Post-Launch Learning

Two steps, run in order after the active arc ends.

| Step | Output |
|---|---|
| 5.1 Harvest retro | Run [`system/skills/deliverable-harvest/SKILL.md`](../../../system/skills/deliverable-harvest/SKILL.md) + [`system/skills/voice-harvest/SKILL.md`](../../../system/skills/voice-harvest/SKILL.md) over the campaign (shared source-read). Findings route on approval: template patches → `system-improvement`, voice pairs → `voice/pairs/`, recurrences → builder-backlog, campaign-specific notes → `feedback-log.md`. Summary lands in `phase-5-launch-and-learn/system-retro-v{N}.md`. |
| 5.2 Performance + campaign retro + completion | One closeout motion: capture `phase-5-launch-and-learn/performance-data.csv` per [`composio-notes.md`](../composio-notes.md) (connector-first MCP scan, manual gap-fill; metrics are meaningful ~14 days after each post's `published.posted_at`), then score the campaign in `phase-5-launch-and-learn/campaign-retro-v{N}.md`, then completion/archive when approved. |

**Tracker update during Phase 5:**
- Harvest retro lands: add `system-retro` row to `deliverables` at `status: locked` + set top-level `system_retro_completed: {date}`.
- Campaign retro lands: add `campaign-retro` row at `status: locked` + set `closeout_retro_completed: {date}` + LIFECYCLE `status: complete` + `completed_at: {date}`.
- Move folder to `workspace/projects/completed/{slug}/` (folder location is a side-effect of `LIFECYCLE.status: complete | cancelled`, not its own status value).

Retro shapes: harvest retro = the two harvest skills + `system-retro` template for the summary; campaign retro = the `campaign-retro` template (performance capture is its input — capture first, then score).

All retros run before the campaign moves to `workspace/projects/completed/{slug}/`. Skipping a required retro is logged to the campaign's `activity.md` as a `phase_override`; pattern of skipping surfaces in the quarterly meta-retro via `activity.md` + `system/audit/agentframe.db`.

## Skipping Ahead (Override Path)

The user can override sequence at any time. When they do:

1. Agent flags the missing prereq specifically and offers the cleaner path.
2. If user insists on skipping, agent appends a `phase_override` entry to `workspace/projects/{slug}/activity.md` using the canonical shape in [`project-frontmatter.md`](../project-frontmatter.md) "Activity event line shapes."
3. Agent proceeds. No moralizing.

Pattern of overrides surfaces at quarterly self-review or when an override repeats: "The operator overrode campaign-architecture in 4 of 5 last campaigns — is that step worth less than we think, or are they leaving money on the table?"
