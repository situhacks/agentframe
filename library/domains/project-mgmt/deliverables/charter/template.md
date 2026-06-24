# Template: Charter (SOW)

## Purpose

The engagement's seed: scope, objectives, stakeholders, and success criteria. Everything else in the engagement derives from it. Normally an **input you receive** (the signed SOW / statement of work) — its instance lives in the project's `sources/`, read-only. This template is for structuring or distilling that input into a usable charter when one isn't handed over clean.

## Inputs

- The SOW / contract / engagement letter (in `sources/`).
- Kickoff notes or the client's stated goals.

## Sections

- **Scope** — what this engagement covers, in one tight paragraph.
- **Objectives** — the outcomes the engagement exists to produce. Each tied to a measurable result.
- **Stakeholders** — named people with roles (sponsor, decision-maker, day-to-day contact, affected teams). References the global people entities by slug where they exist.
- **Success Criteria** — binary, measurable yes/no outcomes the closeout retro will check against.
- **Constraints** — timeline, budget, fixed dates, what can't change.
- **Out of Scope** — explicit non-goals, to prevent scope creep.

## Hard Constraints

- Every objective has a measurable result; no vague "improve X".
- At least one binary, measurable success criterion.
- Stakeholders are named with roles — not "the client team".
- Out-of-scope section is not empty.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | locked>
last_updated: <ISO-8601 timestamp>
---
```

`status` + `last_updated` per [`deliverable-versioning.md`](../../../../process/deliverable-versioning.md).

## Lock Criteria

- Scope, objectives, stakeholders, success criteria, and out-of-scope all present.
- Success criteria are binary/measurable.
- Operator confirms the charter reflects the engagement.
- Lock-event mechanics per [`lock-event.md`](../../../../process/lock-event.md). Once locked, the four living governance docs are derived from it.
