# AgentFrame Marketing — Standard Campaign Flow

This is the fuller multi-post, stakeholder-capable campaign flow. Solo campaigns should start from `solo-flow.md`.

Each deliverable's full template lives in `library/deliverables/{type}/template-vF.md`. This file is the map; templates are the territory.

## Read Once

- Versioning and snapshot mechanics are owned by each deliverable template: `library/deliverables/{type}/template-vF.md`.
- Campaign tracker schema and review-state enums are owned by [`library/process/campaign-frontmatter.md`](../campaign-frontmatter.md).
- In every phase, apply file edits and `campaign.md` tracker updates in the same turn.

## Phase 1 — Research

Owned by the agent in-session when the operator starts a detailed campaign.

| Step | Output | Owner |
|---|---|---|
| 1.1 Campaign idea bank | `phase-1-research/idea-bank.md` | Agent + user |
| 1.2 Research artifact | `phase-1-research/research-artifact-vF.md` | Agent + user |

Detailed campaigns start from an operator-launched research pass. Connected workspace context can inform candidate ideas when available, but it is optional.

Load [`library/process/research-and-signals.md`](../research-and-signals.md) for the workspace-context definition, the live Composio/Rube MCP scan procedure, the Gemini Deep Research API vs web-handoff offer, and the fallback rule.

Idea-bank shape (keep tight): a candidate list plus the selected pick, nothing more. Per candidate: title, 1-3 sentence thesis, and one provenance line. Name the selected pick in one line. Do not add per-candidate Risk, Research Questions, Workspace Signal Summary, or Next Research Step sections.

Save research output as `phase-1-research/research-artifact-vF.md` at `status: drafting`; only operator acceptance locks it. Create the campaign folder at `workspace/campaigns/{your-slug}/` with `campaign.md` populated from the v2 frontmatter schema in [`library/process/campaign-frontmatter.md`](../campaign-frontmatter.md). Set `campaign_flow: standard-flow` and `current_phase: 1-research`.

**Tracker update at end of Phase 1:** `current_phase: 2-strategy`. Add to `deliverables`:
```yaml
research-artifact:
  status: locked
  file: phase-1-research/research-artifact-vF.md
  last_updated: {date accepted}
```
This transition happens only after the operator approves the research artifact.

## Phase 2 — Strategy

Two deliverables. Both can be reviewed externally OR drafted-through-to-lock — neither path is "exception-handling." Default behavior depends on whether a stakeholder exists for this campaign (see "Review path defaults" below).

| Step | Deliverable | Produces | Review path | Blocks |
|---|---|---|---|---|
| 2.1 | Business Brief | `phase-2-strategy/business-brief/draft-vF.md` | External when a stakeholder exists; otherwise drafting → lock | 2.2 cannot start until 2.1 is locked (with or without review) |
| 2.2 | Campaign Brief | `phase-2-strategy/campaign-brief/draft-vF.md` | External when a stakeholder exists; otherwise drafting → lock | Phase 3 cannot start until 2.2 is locked (with or without review) |

When external review IS the path: agent offers to draft the Gmail + Calendar invite when the deliverable hits export (see `runtime/review-coordination.md`). `review: pending` while the brief is in flight with the reviewer, and `expected_feedback_by` should be recorded when the user has a real expectation date. User sends; reviewer responds; user pastes feedback into chat; agent applies revisions; agent flips `review: complete`. Operator then locks. If the reviewer returns "kill it", follow the cancellation rule in [`library/process/campaign-frontmatter.md`](../campaign-frontmatter.md).

When external review is NOT the path: brief drafts, gets locked, downstream unblocks. No `phase_override` log — review never being expected is not an override.

**Tracker update at end of Phase 2:** `current_phase: 3-planning`. Add to `deliverables`:
```yaml
business-brief:
  status: locked
  file: phase-2-strategy/business-brief/draft-vF.md
  last_updated: {date locked}
  review: {not_required | complete | waived}
campaign-brief:
  status: locked
  file: phase-2-strategy/campaign-brief/draft-vF.md
  last_updated: {date locked}
  review: {not_required | complete | waived}
```
Success criteria from the locked Business Brief stay IN the brief (the brief is the canonical source). `campaign.md` does not duplicate them.

## Phase 3 — Planning

Three deliverables. Internal — not stakeholder-facing. Outputs prep for production.

| Step | Deliverable | Produces | Depends on |
|---|---|---|---|
| 3.1 | Messaging Architecture | `phase-3-planning/messaging-architecture/draft-vF.md` | 2.2 locked |
| 3.2 | Design Language ([template](../../deliverables/design-language/template-vF.md)) | `phase-3-planning/design-language/design-language-vF.md` (+ `tokens.yaml`, `tokens.css`, `preview/`) | 2.2 locked |

3.1 and 3.2 can run in parallel after 2.2 locks.

Hard rule for multi-post campaigns: ALL post skeletons (3.2) must exist BEFORE Phase 4 begins. Posts in a series talk to each other; copywriter can't honor cross-post callbacks if it can't see them.

**Tracker update at end of Phase 3:** `current_phase: 4-production`. Add (or update) in `deliverables`:
```yaml
messaging-architecture:
  status: locked
  file: phase-3-planning/messaging-architecture/draft-vF.md
  last_updated: {date locked}
design-language:
  status: locked
  file: phase-3-planning/design-language/design-language-vF.md
  last_updated: {date locked}
```
Also add one row per planned post (`post-1`, `post-2`, …) at `status: not_started` with a folder pointer. The Phase 4 work fills the per-post `vF` files in.

## Phase 4 — Production and Launch

Per-post deliverables. Run in parallel after Phase 3 completes. Phase 4 owns launch execution for each post; Phase 5 is for post-launch learning after the active production arc closes.

When a production deliverable has many unresolved directions, multi-session scope, or risky edits across canonical artifacts, offer a scratchpad in the post folder.

**Scratchpads (recommended for first-version posts and major version bumps):** write `phase-4-production/posts/post-{n}/scratchpad-v{N}.md`, where `N` matches the version it informed. Scratchpads are throwaway planning notes: never read a prior-version scratchpad when working on a later version.

| Step | Deliverable | Produces | Depends on |
|---|---|---|---|
| 4.1 | Post Copy (per post) | `phase-4-production/posts/post-{n}/copy-vF.md` | 3.1 locked, 3.2 locked |
| 4.2a | Carousel Spec → HTML render (per post, when applicable) | `phase-4-production/posts/post-{n}/carousel-spec-vF.md` | 3.1 locked, 3.2 locked, 3.2-tokens locked |
| 4.2b | Video Spec → video project/render (per post, when applicable) | `phase-4-production/posts/post-{n}/video-spec-vF.md` + `video/` and/or `edit/` | 3.1 locked, 3.2 locked when visual continuity matters |
| 4.3 | Image Prompt + Gemini Nano Banana variants (per post, when applicable) | `phase-4-production/posts/post-{n}/image-prompt-vF.md` + `images/` | 4.2a carousel for slide context OR 4.2b video for stills/backgrounds/transitions |
| 4.4 | Publish coordination + media reconciliation (per post) | shipped frontmatter in the post's canonical `-vF.md` + `activity.md` `post_published` entry | copy/media locked and operator confirms the live URL |

External review (only when the post goes to a leadership stakeholder before publish — same default rule as Phase 2): agent offers to coordinate. Per-post.

When both per-post copy and visuals are ready, run the coherence cross-check defined in [`carousel-spec/template-vF.md`](../../deliverables/carousel-spec/template-vF.md) or [`video-spec/template-vF.md`](../../deliverables/video-spec/template-vF.md).

When post copy is locked and publish media exists or has been selected, follow the publish-prep procedure in [`composio-notes.md`](../composio-notes.md).

**Tracker update during Phase 4** (per post, in the same turn as the file edit):
- Post starts drafting: `deliverables.post-{n}.status: drafting` + `file: phase-4-production/posts/post-{n}/copy-vF.md` + `last_updated`.
- Video spec starts before copy exists: add/update `deliverables.post-{n}-video.status: drafting` + `file: phase-4-production/posts/post-{n}/video-spec-vF.md` + `last_updated`. Keep `deliverables.post-{n}` tied to the post-copy/publish state owner.
- Post copy locks: `status: locked`.
- Post publishes (Phase 4.4): `status: shipped`. The shipping record (platform, URL, posted_at, shipped_media[]) lives in the post's canonical `-vF.md` frontmatter — see [`library/deliverables/post-copy/template-vF.md`](../../deliverables/post-copy/template-vF.md) "Shipped frontmatter" section for the standard copy-owned shape. **Increment `posts_published`** and update LIFECYCLE `shipped_at` if this is the first published post.
- Arc changes mid-campaign (post added, post dropped, post renumbered): update `post_count` AND the affected `post-{n}` rows in the same turn. Optional: add `framing_note` if the post's job in the arc shifted.

**Tracker update at end of Phase 4:** `current_phase: 5-launch-and-learn`. Set when every active post is `shipped`, `cancelled`, or explicitly removed from active campaign scope. Do not advance the whole campaign to Phase 5 on first ship; multi-post campaigns can publish early posts while later posts remain in production.

## Phase 5 — Post-Launch Learning

Four steps. The two Builder-owned retros run back to back after the active arc ends: system behavior first, then template evolution. Performance capture is window-driven CSV data entry. Campaign retro runs with the final analytics pass unless the operator explicitly closes early.

| Step | Output |
|---|---|
| 5.1 System behavior retro | `phase-5-launch-and-learn/system-retro-vF.md` after final active post completion; operator may delay to pair with campaign retro. |
| 5.2 Template evolution retro | `phase-5-launch-and-learn/template-retro-vF.md` for scoped v1 to vF template analysis; split large campaigns across multiple sessions only when needed. |
| 5.3 Performance capture | `phase-5-launch-and-learn/performance-data.csv` with one row per post/window. |
| 5.4 Campaign retro + completion | `phase-5-launch-and-learn/campaign-retro-vF.md` about 14 days after the final active post ships, then campaign completion/archive when approved. |

**Tracker update during Phase 5:**
- System retro lands: add `system-retro` row to `deliverables` at `status: locked` + set top-level `system_retro_completed: {date}`.
- Template retro lands: add `template-retro` row to `deliverables` at `status: locked`. Do not set a top-level counter.
- Campaign retro lands: add `campaign-retro` row at `status: locked` + set `campaign_retro_completed: {date}` + LIFECYCLE `status: complete` + `completed_at: {date}`.
- Move folder to `workspace/campaigns/completed/{slug}/` (folder location is a side-effect of `LIFECYCLE.status: complete | cancelled`, not its own status value).

**Performance capture** follows [`composio-notes.md`](../composio-notes.md): connector-first live MCP scan per platform, then manual gap-fill against the canonical CSV columns. Nudge around 14 days after each post's `published.posted_at`; for multi-post campaigns, capture can happen for early shipped posts while later posts are still in Phase 4, before system retro, or between system retro and campaign retro.

For retro shape and decision logic, use the relevant template (`system-retro`, `template-retro`, `campaign-retro`).

All retros run before the campaign moves to `workspace/campaigns/completed/{slug}/`. Skipping a required retro is logged to the campaign's `activity.md` as a `phase_override`; pattern of skipping surfaces in the quarterly meta-retro via `activity.md` + `system/audit/agentframe.db`.

## Skipping Ahead (Override Path)

The user can override sequence at any time. When they do:

1. Agent flags the missing prereq specifically and offers the cleaner path.
2. If user insists on skipping, agent appends a `phase_override` entry to `workspace/campaigns/{slug}/activity.md` using the canonical shape in [`campaign-frontmatter.md`](../campaign-frontmatter.md) "Activity event line shapes."
3. Agent proceeds. No moralizing.

Pattern of overrides surfaces at quarterly self-review or when an override repeats: "The operator overrode messaging-architecture in 4 of 5 last campaigns — is that step worth less than we think, or are they leaving money on the table?"
