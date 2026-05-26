# AgentFrame Marketing — Solo Campaign Flow

Default campaign flow for solo marketers. This file is a map, not a runbook: it names phase order, expected deliverables, tracker transitions, flow-level gates, and campaign-level completion.

## Purpose

Run a lightweight campaign for one accountable operator with no assumed stakeholder review. The flow keeps enough rigor to set a success bar, produce usable assets, ship, and learn without forcing enterprise review ceremony.

## Phase Sequence

| Phase ID | Phase | Gate To Advance |
|---|---|---|
| `1-research-and-architecture` | Research & Architecture | Campaign Architecture locked. |
| `2-design-language` | Design Language | Visual direction locked, or explicitly deferred for text-only work. |
| `3-produce-ship` | Produce + Ship | Active post deliverables are shipped, cancelled, or removed from scope. |
| `4-learn-close` | Learn + Close | Required retros and campaign closeout are complete or explicitly deferred. |

## Deliverables By Phase

| Phase | Deliverable | File Target | Owner |
|---|---|---|---|
| `1-research-and-architecture` | Campaign Idea Bank | `phase-1-research/idea-bank.md` | flow-owned shape below |
| `1-research-and-architecture` | Campaign Scratchpad | `phase-1-research/campaign-scratchpad-v{N}.md` | throwaway, see below |
| `1-research-and-architecture` | Research Artifact | `phase-1-research/research-artifact-v{N}.md` | [`research-artifact`](../../deliverables/research-artifact/template.md) |
| `1-research-and-architecture` | Campaign Architecture | `phase-1-research/campaign-architecture/draft-v{N}.md` | [`campaign-architecture`](../../deliverables/campaign-architecture/template.md) |
| `2-design-language` | Design Language | `phase-2-design/design-language-v{N}.md` | [`design-language`](../../deliverables/design-language/template.md) |
| `3-produce-ship` | Post Copy | `phase-3-production/posts/post-{n}/copy-v{N}.md` | [`post-copy`](../../deliverables/post-copy/template.md) |
| `3-produce-ship` | Media deliverable, when needed | `phase-3-production/posts/post-{n}/{media}-v{N}.md` | [`carousel-spec`](../../deliverables/carousel-spec/template.md), [`image-production`](../image-production.md), or [`video-spec`](../../deliverables/video-spec/template.md) |
| `4-learn-close` | System Behavior Retro | `phase-4-close/system-retro-v{N}.md` | [`system-retro`](../../deliverables/system-retro/template.md) |
| `4-learn-close` | Template Evolution Retro | `phase-4-close/template-retro-v{N}.md` | [`template-retro`](../../deliverables/template-retro/template.md) |
| `4-learn-close` | Performance Data | `phase-4-close/performance-data.csv` | shipped post frontmatter + [`composio-notes`](../composio-notes.md) |
| `4-learn-close` | Campaign Retro | `phase-4-close/campaign-retro-v{N}.md` | [`campaign-retro`](../../deliverables/campaign-retro/template.md) |

**Idea-bank shape (keep tight):** a candidate list plus the selected pick, nothing more. Per candidate: title, 1-3 sentence thesis, and one provenance line. Name the selected pick in one line. Do not add per-candidate Risk, Research Questions, Workspace Signal Summary, or Next Research Step sections.

**Campaign Scratchpad (optional):** When an idea is selected but unstructured, ask: *"Do we have enough direction to start the Campaign Architecture, or would you like to open a `campaign-scratchpad-v1.md` to collect thoughts while we do the initial research passes?"* This prevents locking down a half-baked idea too early and solves context rot. Throwaway once Campaign Architecture starts drafting.

**Post Scratchpads (recommended for first-version posts and major version bumps):** when planning a new post or substantive rewrite, offer `phase-3-production/posts/post-{n}/scratchpad-v{N}.md`, where `N` matches the version it informed. Scratchpads are throwaway planning notes: never read a prior-version scratchpad when working on a later version.

## Phase Pointers

Load-on-demand procedures by phase. Solo flow is the same shape as standard flow on these beats — both flows lazy-load the same shared files.

| Phase | When | Load |
|---|---|---|
| `1-research-and-architecture` | Operator starts a new campaign | Offer menu: Workspace Scan (loads [`research-and-signals.md`](../research-and-signals.md)), Brainstorm Idea Bank, or Direct Input. |
| `2-design-language` | Operator ready for visual direction | Run as an interactive coworking session in chat to align on mood before drafting. |
| `3-produce-ship` | Both per-post copy and visuals drafted | Coherence cross-check defined in [`carousel-spec/template.md`](../../deliverables/carousel-spec/template.md) or [`video-spec/template.md`](../../deliverables/video-spec/template.md). |
| `3-produce-ship` | Post copy locked and publish media selected | [`composio-notes.md`](../composio-notes.md) "Publish Prep" for the connected-tools draft offer; PDF/document carousels stay manual. |
| `4-learn-close` | ~14 days after each post's `posted_at` | [`composio-notes.md`](../composio-notes.md) "Performance Capture" for the per-platform live MCP scan, canonical CSV columns, and partial-data rule. |
| Any phase | Operator overrides sequence | Activity event line shape in [`campaign-frontmatter.md`](../campaign-frontmatter.md) "Activity event line shapes." |

## Tracker Updates

Use [`campaign-frontmatter.md`](../campaign-frontmatter.md) for schema, allowed values, and drift checks.

- New solo campaigns start with `current_phase: 1-research-and-architecture`, `campaign_flow: solo-flow`, `deliverables: {}`, `post_count: 0`, and `posts_published: 0`.
- Phase 1 idea selected: add/update `idea-bank` at `status: locked` (or bypass if direct input).
- Phase 1 concurrent work: add/update `research-artifact` and `campaign-architecture` at `status: drafting`.
- Phase 1 locked: `campaign-architecture` locks, add planned `post-{n}` rows as `not_started`, update `post_count`, then set `current_phase: 2-design-language`.
- Phase 2 locked or deferred: add/update `design-language`, then set `current_phase: 3-produce-ship`.
- Phase 3 production starts: each active post moves through `not_started -> drafting -> locked -> shipped` in the same turn as its canonical file changes.
- Phase 3 complete: when every active post is `shipped`, `cancelled`, or removed from scope, set `current_phase: 4-learn-close`.
- Phase 4 lands system/template retros, performance data, and campaign retro as deliverable rows. Campaign Retro lock sets `campaign_retro_completed`, `LIFECYCLE.status: complete`, and `completed_at`.

## Overrides And Skips

An override is a state-changing departure from the expected solo path. Record the event in `activity.md` using the canonical line shape in [`campaign-frontmatter.md`](../campaign-frontmatter.md) "Activity event line shapes," and keep the durable reason in the lowest owning file.

- Research can be deferred only when the operator supplies enough source context to draft the Campaign Architecture. The deferred Research Artifact owns the reason.
- Design Language can be deferred for text-only work. The deferred design artifact owns the reason; production may proceed without visual assets.
- A planned post can be removed from scope before shipping. Update `post_count`, the affected `post-{n}` row, and `activity.md`.
- Performance capture can be partial. Unsupported metrics stay unknown, not zero; Campaign Retro carries the partial-data caveat.
- System Behavior Retro, Template Evolution Retro, or Campaign Retro can be explicitly deferred, but the campaign cannot move to `complete` until Campaign Retro is locked or the operator records a closeout override.
- Cancellation can happen from any phase. Set `LIFECYCLE.status: cancelled`, `cancelled_at`, `cancelled_reason`, and append the cancellation event to `activity.md`.

## Completion Criteria

A solo campaign is complete when:

- every active production deliverable is shipped, cancelled, or removed from scope;
- System Behavior Retro and Template Evolution Retro are locked or explicitly deferred;
- Performance Data exists or the operator chose to close with partial/unknown data;
- Campaign Retro is locked and has applied the closeout decision;
- `campaign.md` has `status: complete`, `completed_at`, and `campaign_retro_completed` set.

Folder movement to `workspace/campaigns/completed/{slug}/` is a side effect of terminal lifecycle state, not its own status.
