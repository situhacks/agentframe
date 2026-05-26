# AgentFrame Marketing — Open Campaign Flow

A dynamic, "build-as-you-go" flow for campaigns that do not fit a clean sequential phase structure. This file is a process router, teaching the agent how to navigate a fluid campaign using the deliverable library and frontmatter state.

## Purpose

Run campaigns where the exact scope and sequence of deliverables cannot be known upfront. The operator and agent sketch the anticipated deliverables initially, but continuously recalibrate after each step.

## Phase Sequence

There is only one phase: `active`. There are no numbered phase transitions until the campaign is closed.

| Phase ID | Phase | Gate To Advance |
|---|---|---|
| `active` | Active Production | Campaign completed or cancelled. |

## Deliverables By Phase

Because there are no strict phases, any deliverable from `library/deliverables/` can be created at any time during the `active` phase.

When creating a deliverable:
- The path is simply `{deliverable-type}/{filename}.md` (no phase-prefix folders are required).
- The deliverable follows its own canonical template for structure and lock criteria.

## Flow Mechanics

This flow operates as a continuous routing loop:

1. **Kickoff:** When the operator starts a new Open Flow campaign, ask: *"What deliverables do you anticipate needing, and what is your desired cadence?"* Do not lock a rigid plan; just get a sketch.
2. **The Loop:** Ask *"What do you want to tackle first?"* Use the relevant deliverable template (`library/deliverables/{type}/template.md`) to draft and lock the request.
3. **State Tracking:** Every time a deliverable is created or updated, update the `deliverables` block in `workspace/campaigns/{slug}/campaign.md`. This is the campaign's working memory.
4. **Recalibration:** Immediately after a deliverable locks, read the `campaign.md` frontmatter and ask the operator: *"That's locked. Looking at our sketch, do we still want to do [Next Item] next, or are we pivoting?"*
5. **Completion:** The campaign is complete when the operator explicitly decides it is done.

## Tracker Updates

Use [`campaign-frontmatter.md`](../campaign-frontmatter.md) for schema, allowed values, and drift checks.

- New open campaigns start with `current_phase: active`, `campaign_flow: open-flow`, and `deliverables: {}`.
- As deliverables are created, add them to the `deliverables` dictionary.
- When the operator declares the campaign complete, set `LIFECYCLE.status: complete` and `completed_at: {timestamp}`.

## Overrides And Skips

Because there is no rigid sequence, there are no "skips" or "phase overrides." The operator simply builds what they need. If they change their mind about a planned deliverable, remove it from scope or mark it cancelled.
