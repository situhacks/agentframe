# Process File Changes

Use this when adding or materially reshaping a reusable process file under `library/process/`.

## Read First

- `library/process/process-authoring.md`
- Nearby process files with similar load timing

## Procedure

1. **Classify the procedure:** shared process primitive, domain phase checklist, deliverable-specific runbook, or always-loaded invariant.
2. **Name the lowest owner:** shared procedures live in `library/process/`; optional domain phase checklists live in the pack's `production.md`; deliverable-specific gates live in that deliverable's template; cross-cutting route rules live in `AGENTS*.md`.
3. **Locate before inventing:** extend an existing process file when it already owns the job.
4. **Write for runtime use:** purpose, when to load, procedure, verification/logging, and boundaries. Size budget: 1,500 words (`af doctor` flags overages) — over budget, cut before adding.
5. **Verify:** search that the procedure is linked from the lowest loaded situation that needs it, and that its catalog row in `library/process/README.md` is current.

## Refusal Checks

- Do not create a process file for history, provenance, or patch notes.
- Do not use phrase lists as primary triggers when state or intent can define the trigger.
- Do not put domain phase sequencing into a shared process file.
