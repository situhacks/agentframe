---
name: project-consolidate
version: 0.5.0
description: |
  Dream pass — consolidate, compact, and archive stale project knowledge; re-synthesize compiled-truth headers from full timelines; inventory project people and promote earned relationships to the global layer. Keeps long-horizon context files lean.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
  - Agent
  - AskUserQuestion
---

# Project Consolidate (Dream Pass)

Operator-invoked consolidation for **one project at a time**. Never scheduled, never automated: `af doctor` prints a `dream pass recommended` note when its thresholds trip; the agent surfaces that note and offers this skill — the operator decides whether to run it.

## Scope — what one run covers

| Layer | Treatment |
|---|---|
| The named project (`workspace/projects/{slug}/`) | Full consolidation subject (Steps 4–5) |
| Global people layer (`library/context/people/`) | Write target on every run: engagement updates and approved promotions only (Step 5) |
| Rest of the operator layer (`library/context/`) | Health-checked in Step 1; consolidated only if its own thresholds trip (Step 6); otherwise report "operator layer healthy" and skip |

There is no all-projects run. The only cross-project read is the people scan in Step 5.

## Step 0 — Preconditions

- Confirm one project slug. Active and completed projects are both valid.
- This pass mutates Markdown only. Do not edit binaries or private files outside the named project and the snapshotted global context scope.
- Do not require a clean working tree. Project and operator files can be gitignored, so Git is not the review or rollback mechanism.

## Step 1 — Audit (cheap check)

1. Run `python system/af.py doctor {slug}`. Pre-existing schema issues belong to the drift procedure in `library/process/project-frontmatter.md`, not this pass — note them, don't fix them here.
2. Measure the project: line counts for `knowledge/raid-log.md`, `knowledge/decision-log.md`, `knowledge/workback-schedule.md`, `activity.md`, and `project.md`; resolved-vs-open ratios (a log >70% resolved rows recommends archiving); tracker rows published >30 days ago; compiled-truth headers older than their latest timeline entry.
3. Glance at the operator layer: any `library/context/` file over ~300 lines, or global person profile with a stale compiled-truth header.
4. State what the pass will do in one short message and proceed on acknowledgement. New-person promotions have their own batch approval in Step 5; do not add other mid-pass checkpoints.

## Step 2 — Capture the rollback boundary

Before the first live mutation, run:

```text
python system/skills/project-consolidate/scripts/consolidation_review.py snapshot --project {slug}
```

The command prints the run directory. Keep that exact path for the whole pass. It snapshots every Markdown file in the named project and `library/context/people/`, records sizes and SHA-256 hashes, and does not consult Git. If Step 1 tripped broader operator-layer work, restart with a fresh empty run directory and add `--include-operator-context`; never widen a snapshot after mutations begin.

Use these mechanics:

```text
python system/skills/project-consolidate/scripts/consolidation_review.py diff --run-dir {run-dir}
python system/skills/project-consolidate/scripts/consolidation_review.py seal-review --run-dir {run-dir} --path {exact-repo-relative-path} [--path {another-exact-path}]
python system/skills/project-consolidate/scripts/consolidation_review.py verify-review --run-dir {run-dir}
python system/skills/project-consolidate/scripts/consolidation_review.py restore --run-dir {run-dir}
```

`diff` covers every addition, edit, and deletion inside the explicit scopes, including ignored and concurrent files; the broad snapshot is an observation boundary, not a claim that this run owns every later change. At final review, seal only exact paths intentionally mutated by this pass. The seal records every scope change, a digest of that full state, and the owned subset; it renders the full diff from the same in-memory state. `verify-review` later requires the entire scope state to match. `restore` requires that verification, then restores or deletes only the sealed subset. Unsealed changes remain untouched when the reviewed state is unchanged. Keep the run directory until the operator accepts the final review.

## Step 3 — Dispatch mechanics

Mechanical work (Step 4's archive splits, line moves, header drafts) goes to subagents pinned to a cheap model when the harness supports per-subagent model selection (Claude harnesses: sonnet). Judgment — promotion decisions, gap flags, the final report — stays in the main session. No subagent support → run everything inline, sequentially.

## Step 4 — Consolidate project files

1. **RAID log:** move closed risks, validated assumptions, resolved issues, and completed dependencies to `knowledge/_archive/raid-log-{YYYY-MM}.md`; strip them from the active file; leave a one-line note linking the archive.
2. **Decision log:** move decisions older than 30 days and fully settled to `knowledge/_archive/decision-log-{YYYY-MM}.md`, same link note.
3. **Workback schedule:** move completed milestones/tasks to `knowledge/_archive/schedule-{YYYY-MM}.md`.
4. **Activity trail:** if `activity.md` exceeds 200 lines, retain the most recent 50 and move the rest to `knowledge/_archive/activity-{YYYY-MM}.md`.
5. **Entity pages** (`knowledge/people/`, `knowledge/meetings/`): merge duplicates; re-synthesize stale compiled-truth headers.
6. **Tracker rows (`project.md` DELIVERABLES):** move rows with `status: published` and `last_updated` older than 30 days to `knowledge/_archive/deliverables-archive.md` — one rolling file whose frontmatter is a top-level `deliverables:` map holding the rows **verbatim** (same shape as project.md; marketing publish receipts derive all-time totals across tracker + archive, so never reshape or summarize archived rows; create the file with that frontmatter on first use). Rows that stay: `ready` (they are the canonical-content pointers), `deferred` (back-fill obligation), and anything in flight. Touch no other current project state except the Step 7 stamp.
7. **`project.md` body:** collapse completed-phase plan detail to one line per phase, moving the detail to `knowledge/_archive/project-body-{YYYY-MM}.md` with a link note. Keep every declared phase id — the open-flow drift check reads them — and keep the thesis and anything still steering current work.

**Re-synthesis rule:** rebuild each compiled-truth header from the page's full dated timeline plus the relevant `_archive/` files — never by rewording the previous header. Compressing a summary from a summary strips nuance each pass until the page goes generic.

Flag, don't fix: entries without owners, unmitigated open risks, decisions missing rationale. These go in the final report.

## Step 5 — People inventory & promotion (always runs)

1. Inventory this project's `knowledge/people/*.md` overlays.
2. Run `python system/skills/project-consolidate/scripts/consolidation_review.py people --project {slug}`. Its explicit discovery paths cover both `workspace/projects/*/knowledge/people/*.md` and `workspace/projects/completed/*/knowledge/people/*.md`, plus matching global profiles.
3. For each person, pick one of three outcomes:
   - **Update global** — the person already has a global profile: append an engagement-history entry for this project and refresh the global compiled-truth header from all overlays found.
   - **Promotion candidate** — draft `library/context/people/{person-slug}/profile.md` under `{run-dir}/promotion-candidates/`, preserving that full relative path. Use `library/context/_meta/person-profile.md`: compile the header from every overlay found and link every engagement. Do not write the live global profile or its project stakeholder references yet.
   - **Leave project-scoped** — the overlay stays where it is; nothing else happens.
4. **Promotion is a judgment call on the relationship, not a count.** Promote when engagement is sustained or compounding: recurring across projects, a role that will recur (repeat client, ongoing partner, recurring reviewer), or one long project with clear future collaboration. Do not promote on frequency alone — scattered one-off touches months apart with no trajectory stay project-scoped no matter how many there are.
5. **When uncertain, leave project-scoped** and surface the person in the batch report. Do not turn ambiguity into global operator truth.
6. After every candidate is drafted, freeze and render the batch:

   ```text
   python system/skills/project-consolidate/scripts/consolidation_review.py stage-promotions --run-dir {run-dir}
   ```

   Present the complete candidate diff, frozen batch hash, exact run directory, and one-line reasoning per person. Then use the harness's native user-input surface when available, otherwise **end the turn and ask for explicit operator approval of that displayed batch**. The later operator response is the authority boundary: never infer, synthesize, or pre-fill it. Do not apply the batch or continue to Steps 6–8 in the staging turn. Do not ask person by person.
7. In a later turn, after the operator explicitly approves the displayed batch, write an exact excerpt of that approval as data to `{run-dir}/operator-approval.txt` using a safe file edit, not shell interpolation. Then apply the unchanged batch:

   ```text
   python system/skills/project-consolidate/scripts/consolidation_review.py apply-promotions --run-dir {run-dir} --approval-receipt-file {run-dir}/operator-approval.txt
   ```

   The helper stores the receipt against the frozen batch hash but cannot authenticate that a human supplied it; the agent is responsible for using only the operator's actual later approval. Then add approved slugs to this project's `stakeholders` frontmatter list and to `knowledge/stakeholder-map.md` if it exists. If the batch is declined, make no live global or stakeholder writes. If candidate bytes change after staging, the helper refuses them: restage, present the new batch, and end the turn for fresh approval.

## Step 6 — Operator layer (only when Step 1 tripped)

Apply the Step 4 treatment to whatever tripped: archive resolved/stale material to an `_archive/` sibling, re-synthesize stale compiled-truth headers (re-synthesis rule applies), merge duplicates. This step is allowed only when Step 2 snapshotted the full operator context. Voice-system files are out of scope — they have their own harvest path.

## Step 7 — Stamp & verify

1. Set optional `last_consolidated: {today}` in `project.md` after the first completed pass.
2. Append to `activity.md`: `{YYYY-MM-DD HH:MM} — knowledge_consolidation: dream pass; archived {what}; pruned {n} lines; promoted {slugs|none}.`
3. Run `python system/af.py doctor {slug}` — the pass must not have introduced issues.

## Step 8 — Final review gate

1. Run `diff` to identify each path intentionally mutated by this pass. Classify every other changed path as **unsealed concurrent/unrelated work**.
2. If pass-owned and unrelated edits share one file, do not seal it as wholly owned. Mechanical rollback cannot separate same-file authorship; stop and resolve that file manually with the operator.
3. Run `seal-review` with one repeated exact `--path` per pass-owned file. It reads the current scope once, stores exact records for **all** added/changed/deleted paths plus a full-state digest, and renders that same in-memory full diff with sealed/unsealed lists. Treat this output, not the earlier `diff`, as the decision artifact.
4. Present the report with operations, line savings, promotion outcomes, flagged gaps, the seal's digest and full diff, and sealed-versus-unsealed lists. End the turn for the operator's accept-or-rollback decision.
5. In the later decision turn, run `verify-review` before honoring either acceptance or rollback. It recomputes the full scope-change state and preflights sealed paths. If it fails for a sealed **or unsealed** path, rerender, reseal, and request a renewed decision; do not accept or roll back against stale review.
6. After successful verification, record acceptance or run `restore` for rollback. `restore` verifies the full state again immediately before selective mutation, then touches only sealed paths. When state is unchanged, every unsealed addition, edit, or deletion remains intact. Show the follow-up full diff after rollback.

Do not use `git diff`, `git reset`, or a clean-tree assumption for this gate. Never end the turn with unreviewed private mutations or discard the run directory before acceptance.
