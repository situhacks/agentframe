---
name: agentframe-structure
version: 0.1.0
description: |
  Use when Builder mode is adding, renaming, defaulting, or retiring campaign flows; adding or retiring deliverable types; creating process files; changing phase systems; or moving workflow ownership between flows, process files, deliverable templates, skills, and personas.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - AskUserQuestion
---

# AgentFrame Structure Changes

Use this for structural changes to AgentFrame's operating system. Ordinary one-file patches still use `system-improvement`; structure changes start here because they affect taxonomy, routing, defaults, or ownership boundaries.

## First Decision

Load exactly one reference first:

- Campaign flow added, renamed, retired, or default changed: read `flow-changes.md`.
- Deliverable type added, retired, or materially reshaped: read `deliverable-template-changes.md`.
- Process file added or materially reshaped: read `process-file-changes.md`.

If the change crosses categories, pick the category that owns the first durable product definition. Use the other references only after that decision.

## Gates

Before editing:

1. **Name the structural object.** Campaign flow, deliverable type, process file, template shape, routing pointer, or persona rule.
2. **Name the lowest owner.** Flow definitions live in `library/process/flows/`; deliverable shape lives in `library/deliverables/`; reusable procedures live in `library/process/`; generic change mechanics live in skills; cross-cutting invariants live in `AGENTS*.md`.
3. **Locate before inventing.** Check existing flows/templates/process files before adding another one. If an existing object covers 70%+ of the job, extend or branch it unless the operator explicitly approves a new object.
4. **Plan compatibility.** Identify aliases, shims, starter scaffolds, flow registries, and docs that must still resolve after the change.
5. **Verify discoverability.** Prove future agents can find the new shape by search or registry pointer.
6. **Log the change.** Append `system_changes` rows for structural behavior changes.

## Boundaries

- Do not put flow details in this skill. Flow details live in `library/process/flows/`.
- Do not embed long template/process skeletons here. Authoring standards live in `library/deliverables/_meta/template-authoring.md`, `library/process/process-authoring.md`, and `library/process/flow-authoring.md`.
- Do not patch `AGENTS*.md` unless the route or cross-cutting invariant is wrong.
- Do not write campaign deliverables from this skill.
