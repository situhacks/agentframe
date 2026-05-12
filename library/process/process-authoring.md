# Process File Authoring

Use this when creating or materially reshaping reusable process files under `library/process/`.

## Purpose

Process files define reusable procedures that are too specific for `AGENTS*.md` and too procedural for deliverable templates. They should load only when the situation needs the procedure.

## Required Sections

Use this order:

1. `Purpose` — what procedure this file owns and why it exists.
2. `When To Load` — state or intent triggers for loading it.
3. `Procedure` — ordered steps the agent can execute.
4. `Verification Or Logging` — how the agent proves or records the outcome.
5. `Boundaries` — what this process file does not own.

## Trigger Rules

- Prefer state and intent over quoted phrases.
- Phrase examples are allowed only as secondary examples.
- If a trigger depends on campaign phase sequence, the campaign flow file should point here.

## Ownership Rules

- Shared procedures live in `library/process/`.
- Flow-specific phase sequencing lives in `library/process/campaign-flows/`.
- Deliverable-specific output requirements live in `library/deliverables/{type}/template-vF.md`.
- Cross-cutting invariants live in `AGENTS*.md`.

## Runtime Cleanliness

Every section must help a future agent decide, execute, compare, or verify. Do not keep provenance, patch history, or retro narration in runtime process files unless the agent needs it to choose behavior.

Log system-level process changes in `system/audit/agentframe.db`.
