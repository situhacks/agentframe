# Project Flow Authoring

Use this when adding or materially reshaping a project flow under `library/process/flows/`.

## Purpose

Project flows define the lightweight map for a project type. They name phase order, expected deliverables, tracker transition points, flow-level gates, and project-level completion conditions.

Flow files are high-read surfaces during project work: agents read `project.md` frontmatter for state, then lazy-load the selected flow when phase rules or next steps are needed. Keep flow files as maps, not runbooks.

## Load Chain

1. Active `AGENTS.md` routes the situation.
2. `workspace/projects/{slug}/project.md` frontmatter owns project state, current phase, selected `flow`, deliverable tracker, and counters.
3. `library/process/flows/{flow}.md` loads when phase rules, next steps, deliverable sequence, or flow-specific gates are needed.
4. Deliverable templates load only when drafting, revising, locking, publishing, or analyzing that deliverable.
5. Process files load only when their specific procedure is needed.

Put content in the lowest file already loaded for the situation. If the flow only needs to point to deeper behavior, link to the lazy-loaded owner.

## Required Sections

Use this order:

1. `Purpose` — what project shape this flow serves.
2. `Phase Sequence` — phase names and order, plus the gate that moves the project to the next phase. Keep phase-specific procedures out; link to the process file or deliverable template that owns the work.
3. `Deliverables By Phase` — deliverable names, file targets, and template links. Do not restate deliverable output shapes, hard constraints, lock criteria, voice/context rules, or export mechanics.
4. `Tracker Updates` — flow-specific project frontmatter transition points: phase advancement, expected deliverable rows, and required status changes. Link to `project-frontmatter.md` for schema, allowed values, and drift checks.
5. `Overrides And Skips` — state-changing departures from the expected path, such as skipping an expected deliverable, moving ahead with a deferred artifact, cancelling a project, or closing with partial Phase 5 data. Define when something counts as an override and where to record it; reusable mechanics stay in `project-frontmatter.md`, `activity.md` conventions, or the relevant process file.
6. `Completion Criteria` — project-level terminal gates only. This is not deliverable lock criteria. Say what makes this flow complete, cancelled, or ready to archive, then point to the templates/process files that own the details.

## Optional Sections

Optional sections are rare because the flow is read often. Add one only when it changes how an agent uses the flow itself.

- `Load Notes` — only when the flow must load, partially load, or deliberately skip a process/template/context file to execute correctly. Name the source; do not restate its content.
- `Flow-Specific Branches` — only for branches that materially change this flow's phase path and are not already covered by `Overrides And Skips`.
- `Compatibility Notes` — only when renaming or replacing a loaded path. Omit for new flows with no compatibility concern.

Do not create placeholder sections to say a thing does not apply.

## Default Selection

`library/process/flows/README.md` owns the default flow. `project.md` frontmatter owns the selected flow for each project instance through `flow`. Individual flow files describe themselves; they do not declare themselves default.

## Ownership Rules

- Flow files own phase order, deliverable inclusion, tracker transition points, and flow-specific skip/override rules.
- Do not duplicate deliverable template details. Templates own artifact shape, lock gates, publish/export details, and per-deliverable frontmatter.
- Do not duplicate shared process primitives. Process files own reusable procedures such as lock events, performance capture, Composio notes, frontmatter schema, humanizer, voice mini-retro, and cancellation routines.
- Do not duplicate flow registry details. `flows/README.md` owns available flows, default selection, and high-level selection context.
- Do not duplicate project instance state. `project.md` frontmatter owns the selected flow for each project instance.
- Do not put project-specific content in a flow file.

## Duplication Check

- Flow `Deliverables By Phase` must not copy template `Output Shape`, `Hard Constraints`, `Draft Frontmatter Convention`, or `Lock Criteria`.
- Flow `Tracker Updates` may name transition points, but schema and per-deliverable fields stay in `project-frontmatter.md` and deliverable templates.
- Flow `Overrides And Skips` covers phase-path departures; template `Exceptions / Branches` covers deliverable-local state branches.
- Flow `Completion Criteria` covers project-level terminal conditions; deliverable templates own per-deliverable lock criteria.

## Checks Before Adding A Flow

1. Compare against existing flows.
2. If an existing flow covers 70%+ of the job, extend it or document a branch condition.
3. Update `flows/README.md` so future agents can discover the flow.
4. Keep a compatibility shim if a loaded path is renamed.
5. Log the structural change in `system_changes`.
