# Bounded Autonomy

## Purpose

Own the governance for one agent-harness run that iterates toward a defined project outcome. The calling flow or production process supplies the work and quality criteria; this process supplies readiness, authority bounds, model routing, checkpoints, and stop rules.

The run state is one file at `workspace/projects/{slug}/knowledge/autonomy/{run-id}.md`. State changes use `python system/af.py autonomy`; the harness still owns model/subagent dispatch, tool use, and the work itself.

## When To Load

Load when the operator authorizes iterative autonomous work on a bounded goal: for example a technical PoC slice, a multi-pass artifact improvement, or a defined work package. Any project flow may call it.

Do not load for an ordinary single-pass task, recurring monitoring/scheduling, or project work without an existing project folder. A technical run also loads [`technical-build.md`](technical-build.md); deck work also loads [`deck-production.md`](deck-production.md).

## Procedure

### 1. Select authority level

- `plan-only`: inspect and plan; no implementation writes.
- `assisted` (default): execute inside the charter, then stop for human review or a blocking decision.
- `unattended`: iterate without turn-by-turn approval. This requires a real independent reviewer context plus deterministic evidence; otherwise downgrade to `assisted` and say why.

State the level, iteration cap, subagent cap, and requested model tiers before starting. If the operator already selected bounded autonomy with these limits, that selection is acknowledgement.

### 2. Create and complete the run contract

Create the scaffold:

```powershell
python system/af.py autonomy init <project> <run-id> --level assisted
```

Complete its frontmatter and body. A run is ready only when it has:

1. one concrete `goal`;
2. `done_when` stated as observable evidence, a named rubric, or an explicit human judgment criterion;
3. non-empty `allowed_paths` covering every permitted write surface;
4. non-empty `verification` that the agent can actually perform;
5. iteration and subagent budgets;
6. enough source context to avoid inventing material requirements.

Verification must exercise the property named in `done_when` through the surface where it can actually fail. File creation, successful export, or static/source validation does not prove rendered, visual, interactive, or playback quality; the matching production process supplies the inspection method and evidence.

If any semantic requirement is materially unclear, do not implement. Perform a read-only briefing pass, use existing sources first, then ask the single most decision-blocking question or propose a completed contract for approval. Do not interrogate once the goal is safely executable.

Run the deterministic readiness check, then start:

```powershell
python system/af.py autonomy check <project> <run-id>
python system/af.py autonomy start <project> <run-id>
```

### 3. Plan once and route models by role

The current frontier model is controller and planner. Do not spawn a redundant premium planner when the current model already performed that judgment.

Use these capability classes when the harness supports them:

| Role | Default tier | Authority |
|---|---|---|
| Scout | economical | Read-only reconnaissance; no state transition |
| Executor | workhorse | Implement one bounded unit inside `allowed_paths` |
| Reviewer | premium/high-reasoning | Independently test and challenge completion; no implementation |

When per-subagent model choice exists, request the configured tier. When it does not, record the actual behavior as `inherited` or `unknown` in `## Model Routing`; never imply a cheaper model ran. When subagents are unavailable, execute sequentially in the main context and keep the run `assisted`.

Write a short ordered plan in the run file. Each unit must be independently checkable and small enough for one executor pass.

### 4. Execute one unit, verify, checkpoint

For each iteration:

1. Re-read the run contract, current project truth, and last checkpoint.
2. Select exactly one unfinished plan unit.
3. Dispatch one executor when useful; otherwise work inline. The executor cannot expand `allowed_paths`, redefine `done_when`, or mark the run complete.
4. Run the listed deterministic checks or artifact validation.
5. For unattended runs, dispatch the independent reviewer with the original goal, allowed paths, diff/artifact, and check output. The reviewer starts from doubt, may return approve/reject/block, and cannot edit implementation files.
6. Record the result through the button:

```powershell
python system/af.py autonomy checkpoint <project> <run-id> `
  --outcome continue --summary "<what changed and what remains>" `
  --subagents-spawned <count-this-iteration>
```

Use `--outcome blocked` when authority, information, environment, or repeated failure prevents progress. Use `--outcome review --evidence "<commands/rubric/reviewer result>"` only when `done_when` is supported. The button increments the iteration and blocks automatically at the cap.

Do not retry an identical failed approach. A rejected review is another attempt, not permission to widen scope.

### 5. Resume or finish

A blocked run resumes only after the blocker, contract, or budget is deliberately changed:

```powershell
python system/af.py autonomy start <project> <run-id> --resume-reason "<what changed>"
```

A run in `review` finishes through:

```powershell
python system/af.py autonomy finish <project> <run-id> --approved-by operator
```

If `completion_gate: human`, only `operator` approval is valid. If it is `independent-review`, `reviewer` is valid after evidence is recorded. Completion closes the run only; it never marks a deliverable ready, publishes, merges, transmits, overwrites an operator-edited artifact, or completes the project.

## Verification Or Logging

The run file's `## Checkpoints` section is the iteration record. `af autonomy` owns timestamps, iteration counts, cumulative subagent use, states, blocked reasons, completion evidence, and approval. Every checkpoint declares its subagents spawned, including zero; exceeding the cap blocks the run. Material milestones (`autonomy_started`, `autonomy_blocked`, `autonomy_review_ready`, `autonomy_completed`) roll up to `activity.md`; ordinary iterations do not.

At handoff, report the run path, status, iterations used, requested versus actual model routing, verification evidence, and any remaining human decision.

## Boundaries

- This process does not schedule recurring loops, invoke model APIs, choose vendor model names, or estimate prices.
- It does not replace the calling flow, deliverable template, BDR, production process, or their quality criteria.
- It does not authorize external messages, deliverable readiness/publication, merging, purchases, permission changes, or scope expansion.
- It does not write routine run telemetry to `system/audit/agentframe.db`.
- Parallel runs and shared-state coordination are out of scope until dogfooding demonstrates a real need.
