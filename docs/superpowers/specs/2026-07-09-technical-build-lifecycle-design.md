# Technical Build Lifecycle — Design

**Date:** 2026-07-09
**Mode:** Builder
**Status:** Approved design, pending implementation plan

## Problem

The operator's mandate now includes vibe-coding technical proof-of-concepts (managed agents, small apps on Claude/agent SDKs) as phases of client engagements. AgentFrame has no concept of this today. The naive answers all fail:

- **Code inside the project folder** — mangles an idiomatic repo into a deliverables tree.
- **Fully separate workspace** — loses AgentFrame's planning history and forces manual context re-dumping (the chaos-agent precedent).
- **Parallel handoff docs** — rot the moment scope shifts mid-build; nobody reliably reconciles two sources of truth.
- **Hardcoded SDK documentation** — stale within weeks; the industry moves faster than any written reference.

External research (Gemini deep research, 2026-07-09) confirmed: no tool solves bidirectional planning↔code sync automatically. The closest working pattern is Git-as-the-bus, separated repos, dynamically fetched SDK context (`llms.txt`), and tests as the drift detector. Full SDD frameworks (Spec Kit, BMAD) are over-scaffolding for solo proposal-stage POCs.

## Core Principle

**One brain during the build, brain transplant at graduation.**

While AgentFrame orchestrates a build, the external repo deliberately has no independent agent context — it carries a thin *umbilical stub* pointing back to the AgentFrame project. Two brains drift; one brain plus a stub cannot. The full context transfer happens exactly once, at **graduation**: a compile step that distills AgentFrame's accumulated knowledge plus the actual repo state into the repo's own native context files, making it self-sufficient. The handover artifact cannot rot because it does not exist until it is derived.

## Lifecycle

### Phase A — Build starts

A project phase turns technical. The operator (with the agent) creates the external repo at an idiomatic location outside AgentFrame (its own git history). In the same turn:

- `project.md` frontmatter gains `build_repo: <absolute path>` (LIFECYCLE block).
- The repo gets the **umbilical stub**: a minimal `CLAUDE.md` containing (1) "this repo is orchestrated from AgentFrame project `{slug}` at `{path}`; plans, decisions, and BDRs live there; if you are an agent here without that workspace, stop and ask the operator", (2) durable conventions only — commit style, test-before-done, and the `llms.txt` rule below. No plans, no duplicated context.
- `knowledge/build-log.md` is created in the project (status, decisions, BDR register).
- Activity event appended: `build_started`.

### Phase B — Build (AgentFrame is the only brain)

- The AgentFrame session works across both directories. Plans and decisions live only in the project's `knowledge/`; code lives only in the repo.
- **Derived status, never pushed status:** `knowledge/build-log.md` is updated by reading the repo — `git log`, diffs, test output — not from session memory. On session resume, the agent re-derives current build state from the repo before trusting any written status.
- **BDRs as definition of done:** each feature gets one Behavior Decision Record — an observable Given/When/Then contract — registered in `build-log.md`. Its verification test lives *in the repo*, so drift surfaces as a failing test and the tests travel with the repo at graduation.
- **SDK specifics are never written down:** before writing code against any fast-moving SDK, the agent fetches the provider's live docs (`llms.txt` or equivalent). The umbilical stub carries this rule; AgentFrame stores no API documentation.

### Phase C — Graduation (or disposal)

**Trigger (judgment, not threshold):** graduate when the repo will be touched by anyone — client developer, future operator, another agent — *without AgentFrame present*. Disposable proposal POCs never graduate; they die with the project, noted at close-out.

Graduation checklist (in the process doc, executed by the agent):

1. Verify all BDR tests pass in the repo.
2. Compile the repo's native context — real `CLAUDE.md`, `README`, decision records — from project `knowledge/` plus verified repo reality. Derived from both sources, checked against actual code, not remembered.
3. Replace the umbilical stub with the compiled context.
4. Set `build_graduated_at: <ISO date>` in `project.md`; append `build_graduated` activity event.

**Post-graduation:** AgentFrame stops orchestrating the repo. The project keeps only the engagement record (what shipped, decisions, retro). Any later sync is a manual, pull-based re-read of the repo — no standing watch. Graduation means it graduated for a reason.

## Mechanics

### 1. Frontmatter fields (LIFECYCLE block, both optional)

| Field | Type | Notes |
|---|---|---|
| `build_repo` | absolute path or `null` | Presence marks an active or graduated technical build. |
| `build_graduated_at` | ISO date or `null` | `null` while the build is active. Disposal is recorded at project close-out, not here. |

`af doctor` must tolerate these as optional fields (implementation must confirm doctor does not flag unknown/optional fields; patch validation if it does).

### 2. Routing row (anti-context-drop)

One new row in the Operator persona's Routing Index (edited in `AGENTS.operator.md`, then resynced to root via the mode-swap command):

| Situation | Load First |
|---|---|
| `project.md` has `build_repo` set and the build is not graduated | `library/process/technical-build.md`, before any build action or build-state report |

This survives compaction structurally: state-loads already re-read frontmatter every session (schema-drift rule), so the trigger re-fires without relying on session memory.

### 3. Process doc

`library/process/technical-build.md` — the single home for this design's operational content: the lifecycle, the umbilical stub template (inline), the derived-status rule, the BDR shape, the `llms.txt` rule, and the graduation checklist. Registered in `library/process/README.md`.

### 4. Activity event shapes (added to `project-frontmatter.md` canon)

- `build_started: {repo path}; stub written, build-log created.`
- `build_graduated: {repo path}; context compiled into repo; {one-line what shipped}.`

## Explicitly Not Built

- No domain pack, no new mode, no new skill, no `af` verb.
- No automation of graduation — operator-invoked judgment, per the system's judgment-over-thresholds principle.
- No SDK/API reference material anywhere in AgentFrame.
- No standing post-graduation sync mechanism.

If after 2–3 real engagements the graduation compile feels mechanical, it may earn an `af` verb or skill. Not before.

## Implementation Scope (for the plan)

1. Write `library/process/technical-build.md`; register in `library/process/README.md`.
2. Patch `library/process/project-frontmatter.md`: two optional LIFECYCLE fields + two event shapes.
3. Add the routing row to `AGENTS.operator.md`; resync root `AGENTS.md` via the atomic swap command.
4. Verify `af doctor` tolerates the new optional fields; patch if needed.
5. Log `system_changes` audit rows for each system file touched.
