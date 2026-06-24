# Template: Campaign Retro

## Purpose

The Campaign Retro is the Phase 5 outcome closeout. It answers: did this campaign hit the Business Brief success criteria, and what should the operator conclude from the evidence?

It is not a Builder patch queue and it does not run performance capture. System and template learning belong to the harvest retro (`system-retro-v{N}.md`). Performance capture is this step's first move and this doc's input, not its content.

## Inputs

- **Read only:**
  - `phase-2-strategy/business-brief/draft-v{N}.md` for the canonical success criteria.
  - `project.md` frontmatter for shipped count, lifecycle state, and deliverable pointers.
  - `phase-5-launch-and-learn/performance-data.csv` (captured first, same step).
  - `activity.md` only for ship dates, scope changes, back-fill/override evidence, and completion events.
  - Public comment/reply evidence or operator-provided qualitative notes when needed to score a Business Brief criterion.
  - `feedback-log.md` for campaign-scoped observations that bear on the Worked / Did Not Work notes.
- **Dependencies:**
  - Final active post shipped, cancelled, or removed from scope.
  - `system-retro-v{N}.md` is complete or intentionally deferred.
  - Performance capture (same step, runs first) has either produced `performance-data.csv` or the operator chose to close with partial data.
- **Context Loading:**
  - Do not load voice context; this is an analytical closeout doc.

## Output Shape

Use this exact section shape.

### Verdict

One short paragraph. Say whether the campaign was a hit, miss, mixed, or not fully knowable yet. Include the most important caveat.

### Success Criteria Scorecard

Mirror the Business Brief criteria exactly. One item per criterion:

#### {Criterion Short Name}

- **Status:** `HIT`, `NOT_HIT`, `PARTIAL`, or `UNKNOWN`.
- **Basis:** The number, artifact, or operator evidence behind the status.
- **Caveat:** Include only if the evidence is incomplete, the criterion was back-filled, or the scope changed.

Use `UNKNOWN` when required evidence is unavailable. Do not mark unknown data as `NOT_HIT`.

### Performance Snapshot

Report only raw metrics that exist in `performance-data.csv`, with the `source` and caveat. Keep unsupported fields blank/unknown; do not calculate rates unless they clarify the verdict.

If performance data is partial, say so plainly.

### Outcome Notes

Use two short lists:

- **Worked:** 1-3 specific observations tied to the scorecard, performance data, public signal, or operator evidence.
- **Did Not Work:** 1-3 specific observations with the same evidence bar.

Skip generic lessons. If an observation does not affect the campaign verdict, cut it.

### ROI And Quarterly Tie-Back

Keep this short. Include time/cost/quarterly-goal notes only when they change the closeout judgment or future campaign decision.

### Closeout Decision

State one of:

- `lock and complete campaign`
- `route back to 5.3 performance capture`
- `defer campaign retro until {date/event}`
- `close with partial-data caveat`

## Hard Constraints

- **Boundary Rules:** Do not patch system, template, voice, positioning, or process files from Campaign Retro. If market-outcome evidence suggests future Builder work, mention it only as an outcome note. Route actual changes through the harvest retro / `system-improvement` later. Do not restate campaign history. Use only facts needed to score the Business Brief and close the campaign.
- **Criterion Honesty:** Attach compact caveats to the relevant scorecard item when the Business Brief was back-filled after the work was underway, the campaign scope changed after criteria were set, evidence is operator-reported rather than externally observable, or the metric is unavailable through Phase 5.3 capture. External review `not_required` is not a caveat by itself for solo work.
- **Analytics Collection:** Campaign Retro does not run connector discovery, Composio/Rube tools, exports, or manual analytics collection. See `library/process/composio-notes.md`. Treat missing metrics as `unknown`, not `0`. For operator-only qualitative criteria, ask only for the criterion evidence needed to score the Business Brief item. Do not turn this into an analytics collection step.

## Draft Frontmatter Convention

Canonical file: `phase-5-launch-and-learn/campaign-retro-v{N}.md`. Reference the shared versioning convention for snapshot accumulation.

```yaml
---
status: <drafting | locked>
last_updated: <ISO-8601 timestamp>
current_version: <integer>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason>"}
---
```

See `library/process/campaign-frontmatter.md` for schema details.

## Lock Criteria

- Retro uses the exact output shape above.
- Every Business Brief success criterion has a status and basis.
- Unknown evidence is marked `UNKNOWN`, not guessed.
- Performance data is referenced from `performance-data.csv` or explicitly marked partial/unknown by operator choice.
- User approved the closeout decision.
- `campaign-retro-v{N}.md` frontmatter set to `status: locked`.
- Follow `library/process/lock-event.md` and the selected campaign flow for tracker updates, lifecycle completion, and folder moving.

## Exceptions / Branches

- **Missing Performance Data:** If `performance-data.csv` is missing data needed to score a criterion, route back to Phase 5.3 Performance Capture if the operator wants more analytics. Proceed only if the operator explicitly chooses to close with partial data.

