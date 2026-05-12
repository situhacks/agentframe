# Process File Changes

Use this when adding or materially reshaping a reusable process file under `library/process/`.

## Read First

- `library/process/process-authoring.md`
- Nearby process files with similar load timing
- `system/skills/system-improvement/SKILL.md` for ordinary process patches
- `library/process/campaign-flow-authoring.md` if the proposed process is really a campaign flow

## Procedure

1. **Classify the procedure:** shared process primitive, campaign-flow phase rule, deliverable-specific runbook, or always-loaded invariant.
2. **Name the lowest owner:** shared procedures live in `library/process/`; phase sequence lives in `campaign-flows/`; deliverable-specific gates live in that deliverable's template; cross-cutting route rules live in `AGENTS*.md`.
3. **Locate before inventing:** extend an existing process file when it already owns the job.
4. **Write for runtime use:** purpose, when to load, procedure, verification/logging, and boundaries.
5. **Verify:** search that the procedure is linked from the lowest loaded situation that needs it.
6. **Log:** append a `system_changes` row for new process files or material reshapes.

## Refusal Checks

- Do not create a process file for history, provenance, or patch notes.
- Do not use phrase lists as primary triggers when state or intent can define the trigger.
- Do not put flow-specific sequencing into a shared process file.
