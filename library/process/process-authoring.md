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
- If a trigger depends on a project's phase, the project plan or the domain pack's production routing should point here.

## Catalog Row

Every process file has a row in [`README.md`](README.md) (what it owns + when to load). Add or update that row in the same change that creates or reshapes the file; remove the row when a file retires. A process file without a catalog row is unfindable outside its routing chain.

## Ownership Rules

- Shared procedures live in `library/process/`.
- The project shape lives in `library/process/project-flow.md`; optional domain phase checklists live in the pack's `production.md`.
- Deliverable-specific output requirements live in `library/deliverables/{type}/template.md`.
- Cross-cutting invariants live in `AGENTS*.md`.

## Runtime Cleanliness

Every section must help a future agent decide, execute, compare, or verify. Do not keep provenance or patch history in runtime process files unless the agent needs it to choose behavior; a process file's history is its git log.
