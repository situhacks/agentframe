---
name: project-consolidate
version: 0.1.0
description: |
  Consolidate, compact, and archive stale project knowledge and governance documents to keep active context files lean and readable.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - AskUserQuestion
---

# Project Consolidate (Dream Pass)

As long-horizon projects progress, governance files (`raid-log.md`, `decision-log.md`, `workback-schedule.md`, `activity.md`) bloat with historical data, causing context limits to be reached and reducing agent performance. This skill consolidates active files and moves history into an archive.

It runs when explicitly triggered by the operator (e.g., at milestone reviews, phase boundaries, or monthly).

## Procedure

### Step 1 — Audit (Cheap Check)
Scan the active project files under `knowledge/` and the `activity.md` file. Analyze:
- **Line counts:** If `raid-log.md` or `decision-log.md` exceeds 300 lines, or `activity.md` exceeds 500 lines.
- **Active-vs-Resolved ratio:** Count open vs. closed/resolved items in tables. If resolved items exceed 70% of total rows, consolidation is recommended.
- Report these findings to the operator and ask if they wish to proceed.

### Step 2 — Archive & Prune (Status-Based)
For each file containing resolved, completed, or decided historical records:
1. **RAID Log (`knowledge/raid-log.md`):**
   - Extract all closed risks, validated assumptions, resolved issues, and completed dependencies.
   - Write them to a date-stamped file in `knowledge/_archive/raid-log-{YYYY-MM}.md` (grouping by month or quarter).
   - Strip these rows from `knowledge/raid-log.md`. Add a single-line reference note pointing to the archive:
     `> [!NOTE]`
     `> Resolved/closed entries prior to {date} were archived to [raid-log-{YYYY-MM}.md](_archive/raid-log-{YYYY-MM}.md).`
2. **Decision Log (`knowledge/decision-log.md`):**
   - Move decisions older than 30 days that are fully resolved to `knowledge/_archive/decision-log-{YYYY-MM}.md`.
   - Strip them from the active file, leaving a markdown note linking to the archive.
3. **Workback Schedule (`knowledge/workback-schedule.md`):**
   - Extract completed milestones and tasks.
   - Move them to `knowledge/_archive/schedule-{YYYY-MM}.md`.
4. **Activity Trail (`activity.md`):**
   - If the file exceeds 200 lines, extract older lines (retaining the most recent 50 lines).
   - Move the extracted history to `knowledge/_archive/activity-{YYYY-MM}.md`.

### Step 3 — Consolidate Entity Pages
Scan files in `knowledge/people/` and `knowledge/meetings/`:
1. **Merge Duplicates:** Identify duplicate meeting notes or duplicate person entries and propose merging them.
2. **Refresh Compiled-Truth Headers:** For person/meeting pages using theCompiled-Truth + Timeline pattern, compare the header summary with recent timeline entries. If the header does not reflect the latest timeline updates, draft an updated header.
3. **Identify Gaps:** Flag any critical missing information (e.g., missing owners, unmitigated open risks, empty timeline events).

### Step 4 — Propose (FIRST-PASS-THEN-APPROVE)
Present a clear report of all proposed operations:
- Files to be pruned and their line savings.
- New archive files to be written.
- Proposed header consolidations or merges.
- Surfaced gaps or contradictions.

Ask the operator for approval. **Do not modify any project files until the operator explicitly confirms.**

### Step 5 — Verify & Log
1. Run `python system/af.py doctor` on the project. Ensure books are clean.
2. Append a log entry to `activity.md`:
   `{timestamp} — knowledge_consolidation: Consolidated project knowledge; archived resolved governance entries to knowledge/_archive/; pruned {lines_saved} lines.`
