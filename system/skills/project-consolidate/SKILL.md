---
name: project-consolidate
version: 0.2.0
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
| The named project (`workspace/projects/{slug}/`) | Full consolidation subject (Steps 3–4) |
| Global people layer (`library/context/people/`) | Write target on every run — promotions and engagement updates only (Step 4) |
| Rest of the operator layer (`library/context/`) | Health-checked in Step 1; consolidated only if its own thresholds trip (Step 5); otherwise report "operator layer healthy" and skip |

There is no all-projects run. The only cross-project read is the people scan in Step 4.

## Step 0 — Preconditions

- Working tree must be clean (`git status --porcelain`). If dirty, stop and tell the operator — the review surface for this skill is the git diff, and it must contain only dream-pass changes.
- Confirm which project.

## Step 1 — Audit (cheap check)

1. Run `python system/af.py doctor {slug}`. Pre-existing schema issues belong to the drift procedure in `library/process/project-frontmatter.md`, not this pass — note them, don't fix them here.
2. Measure the project: line counts for `knowledge/raid-log.md`, `knowledge/decision-log.md`, `knowledge/workback-schedule.md`, `activity.md`, and `project.md`; resolved-vs-open ratios (a log >70% resolved rows recommends archiving); tracker rows published >30 days ago; compiled-truth headers older than their latest timeline entry.
3. Glance at the operator layer: any `library/context/` file over ~300 lines, or global person profile with a stale compiled-truth header.
4. State what the pass will do in one short message and proceed on acknowledgement. This is the only checkpoint before the final diff review — do not ask again mid-pass.

## Step 2 — Dispatch mechanics

Mechanical work (Step 3's archive splits, line moves, header drafts) goes to subagents pinned to a cheap model when the harness supports per-subagent model selection (Claude harnesses: sonnet). Judgment — promotion decisions, gap flags, the final report — stays in the main session. No subagent support → run everything inline, sequentially.

## Step 3 — Consolidate project files

1. **RAID log:** move closed risks, validated assumptions, resolved issues, and completed dependencies to `knowledge/_archive/raid-log-{YYYY-MM}.md`; strip them from the active file; leave a one-line note linking the archive.
2. **Decision log:** move decisions older than 30 days and fully settled to `knowledge/_archive/decision-log-{YYYY-MM}.md`, same link note.
3. **Workback schedule:** move completed milestones/tasks to `knowledge/_archive/schedule-{YYYY-MM}.md`.
4. **Activity trail:** if `activity.md` exceeds 200 lines, retain the most recent 50 and move the rest to `knowledge/_archive/activity-{YYYY-MM}.md`.
5. **Entity pages** (`knowledge/people/`, `knowledge/meetings/`): merge duplicates; re-synthesize stale compiled-truth headers.
6. **Tracker rows (`project.md` DELIVERABLES):** move rows with `status: published` and `last_updated` older than 30 days to `knowledge/_archive/deliverables-archive.md` — one rolling file whose frontmatter is a top-level `deliverables:` map holding the rows **verbatim** (same shape as project.md; marketing publish receipts derive all-time totals across tracker + archive, so never reshape or summarize archived rows; create the file with that frontmatter on first use). Rows that stay: `ready` (they are the canonical-content pointers), `deferred` (back-fill obligation), and anything in flight. Touch no other current project state except the Step 6 stamp.
7. **`project.md` body:** collapse completed-phase plan detail to one line per phase, moving the detail to `knowledge/_archive/project-body-{YYYY-MM}.md` with a link note. Keep every declared phase id — the open-flow drift check reads them — and keep the thesis and anything still steering current work.

**Re-synthesis rule:** rebuild each compiled-truth header from the page's full dated timeline plus the relevant `_archive/` files — never by rewording the previous header. Compressing a summary from a summary strips nuance each pass until the page goes generic.

Flag, don't fix: entries without owners, unmitigated open risks, decisions missing rationale. These go in the final report.

## Step 4 — People inventory & promotion (always runs)

1. Inventory this project's `knowledge/people/*.md` overlays.
2. Cross-project scan: `workspace/projects/*/knowledge/people/*.md` (including `completed/`) and `library/context/people/*/profile.md`.
3. For each person, pick one of three outcomes:
   - **Update global** — the person already has a global profile: append an engagement-history entry for this project and refresh the global compiled-truth header from all overlays found.
   - **Promote** — create `library/context/people/{person-slug}/profile.md` per the shape in `library/context/_meta/person-profile.md`: header compiled from every overlay found, engagement-history entries linking each project overlay. Add the slug to this project's `stakeholders` frontmatter list (doctor enforces the global-profile + project-overlay pair), and to `knowledge/stakeholder-map.md` if it exists.
   - **Leave project-scoped** — the overlay stays where it is; nothing else happens.
4. **Promotion is a judgment call on the relationship, not a count.** Promote when engagement is sustained or compounding: recurring across projects, a role that will recur (repeat client, ongoing partner, recurring reviewer), or one long project with clear future collaboration. Do not promote on frequency alone — scattered one-off touches months apart with no trajectory stay project-scoped no matter how many there are.
5. **When uncertain, promote.** A wrong promotion costs the operator one folder deletion; a missed promotion silently loses relationship context. Never ask the operator person-by-person — every promotion lands in the diff, with one line of reasoning in the final report.

## Step 5 — Operator layer (only when Step 1 tripped)

Apply the Step 3 treatment to whatever tripped: archive resolved/stale material to an `_archive/` sibling, re-synthesize stale compiled-truth headers (re-synthesis rule applies), merge duplicates. Voice-system files are out of scope — they have their own harvest path.

## Step 6 — Stamp & verify

1. Set optional `last_consolidated: {today}` in `project.md` after the first completed pass.
2. Append to `activity.md`: `{YYYY-MM-DD HH:MM} — knowledge_consolidation: dream pass; archived {what}; pruned {n} lines; promoted {slugs|none}.`
3. Run `python system/af.py doctor {slug}` — the pass must not have introduced issues.

## Step 7 — Review gate (single)

Present one report: operations performed with line savings, promotions with one-line reasoning each, flagged gaps, and `git diff --stat`. The operator reviews the working-tree diff and either approves the commit or asks for a reset / selective revert. Never commit without approval; never end the turn leaving a dirty tree unmentioned.
