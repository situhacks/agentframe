---
name: agentframe-structure
description: |
  Use for structural AgentFrame system work: adding, retiring, or materially reshaping deliverable types or process files, or moving workflow ownership between processes, templates, skills, and task routers. Do not use for ordinary one-file patches or project deliverable work.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - AskUserQuestion
---

# AgentFrame Structure Changes

Use this for structural changes to AgentFrame's operating system. Ordinary one-file patches apply the Builder pre-write gate directly; structure changes start here because they affect taxonomy, routing, defaults, or ownership boundaries.

## First Decision

Load exactly one reference first:

- Deliverable type added, retired, or materially reshaped: read `deliverable-template-changes.md`.
- Process file added or materially reshaped: read `process-file-changes.md`.

If the change crosses categories, pick the category that owns the first durable product definition. Use the other references only after that decision.

## Gates

Before editing:

1. **Name the structural object.** Deliverable type, process file, template shape, routing pointer, or persona rule.
2. **Name the lowest owner.** The project shape lives in `library/process/project-flow.md`; deliverable shape lives in `library/deliverables/`; reusable procedures live in `library/process/`; generic change mechanics live in skills; cross-cutting invariants live in `AGENTS*.md`.
3. **Locate before inventing.** Check existing templates and process files before adding another one. If an existing object covers 70%+ of the job, extend or branch it unless the operator explicitly approves a new object.
4. **Plan compatibility.** Identify aliases, scaffolds, catalogs, and docs that must still resolve after the change.
5. **Verify discoverability.** Prove future agents can find the new shape by search or catalog pointer.

## Boundaries

- Do not put project-shape details in this skill. They live in `library/process/project-flow.md`.
- Do not embed long template/process skeletons here. Authoring standards live in `library/deliverables/_meta/template-authoring.md` and `library/process/process-authoring.md`.
- Do not patch `AGENTS*.md` unless the route or cross-cutting invariant is wrong.
- Do not write campaign deliverables from this skill.
