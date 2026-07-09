# AgentFrame — Technical Build Lifecycle

When a project phase turns technical — vibe-coding a proof-of-concept app, managed agent, or SDK integration — AgentFrame is the planning/memory brain and the **code lives in a separate repo outside AgentFrame**, with its own git history. Load this on demand when `project.md` has `build_repo` set and the build is not graduated, before any build action or build-state report.

**Core principle: one brain during the build, brain transplant at graduation.** While AgentFrame orchestrates, the external repo carries only a thin *umbilical stub* pointing back here. Two brains drift; one brain plus a stub cannot. The full context transfer happens exactly once, at graduation.

---

## Phase A — Build starts

A phase turns technical. Create the external repo at an idiomatic location outside AgentFrame (its own `git init`, own `package.json`/toolchain). In the same turn:

1. Set `build_repo: <absolute path>` in `project.md` frontmatter (LIFECYCLE block). Its presence is the routing trigger that re-loads this file every session.
2. Write the **umbilical stub** — a minimal `CLAUDE.md` at the repo root (template below). No plans, no duplicated context.
3. Create `knowledge/build-log.md` in the project (status, decision notes, BDR register).
4. Append `build_started` to `activity.md` (shape in [`project-frontmatter.md`](project-frontmatter.md)).

### Umbilical stub template

```md
# <repo name>

This repo is orchestrated from AgentFrame project `<slug>` at `<absolute path to project folder>`.
Plans, decisions, and BDRs live there — not here. If you are an agent working in this repo
without that workspace loaded, stop and ask the operator before proceeding.

## Conventions (durable — safe to keep)
- Conventional Commits for all commit messages.
- No feature is done until its BDR test passes (see build-log in the AgentFrame project).
- Before writing code against any SDK, fetch the provider's live docs (`llms.txt` or equivalent)
  and follow current signatures. Do not rely on training-data recall or anything written here for API shape.
```

---

## Phase B — Build (AgentFrame is the only brain)

The session works across both directories: plans and decisions live only in the project's `knowledge/`; code lives only in the repo.

- **Derived status, never pushed status.** Update `knowledge/build-log.md` by *reading the repo* — `git log`, diffs, test output — not from session memory. On session resume or after compaction, re-derive current build state from the repo before trusting any written status.
- **BDRs as definition of done.** Each feature gets one Behavior Decision Record: an observable Given/When/Then contract, registered in `build-log.md`. Its verification test lives *in the repo*, so drift surfaces as a failing test and the tests travel with the repo at graduation. If a requirement cannot be written as a pass/fail scenario observable from outside the code, it is not yet a valid BDR — sharpen it first.
- **SDK specifics are never written down here.** Before writing code against a fast-moving SDK, fetch the provider's live docs. AgentFrame stores no API documentation; it goes stale in weeks.

### BDR shape

```md
## BDR-<n>: <observable capability>
- **Context:** <preconditions / world state>
- **Scenario:** Given <state>, When <action>, Then <observable result>.
- **Test:** <path to the test in the repo that verifies this>
- **Status:** open | passing
```

---

## Phase C — Graduation (or disposal)

**Trigger (judgment, not threshold):** graduate when the repo will be touched by anyone — client developer, future operator, another agent — *without AgentFrame present*. Disposable proposal POCs never graduate; they die with the project and are noted at close-out. Do not scaffold graduation ceremony for throwaway pitch code.

**Graduation checklist:**

1. Verify all BDR tests pass in the repo.
2. Compile the repo's native context — real `CLAUDE.md`, `README`, decision records — from the project's `knowledge/` **plus verified repo reality**. Derived from both and checked against actual code, not remembered.
3. Replace the umbilical stub with the compiled context.
4. Set `build_graduated_at: <ISO date>` in `project.md`; append `build_graduated` to `activity.md`.

**After graduation, AgentFrame stops orchestrating the repo.** The project keeps only the engagement record (what shipped, decisions, retro). Any later contact is a manual, pull-based re-read of the repo — no standing watch. Graduation means it graduated for a reason.

---

## What this deliberately is not

No domain pack, no new mode, no new skill, no `af` verb. This is a single process doc plus two optional frontmatter fields. If after two or three real engagements the graduation compile feels mechanical, it earns tooling then — not before.
