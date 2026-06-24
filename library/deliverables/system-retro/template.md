# Template: System Behavior Retro

## Purpose

The System Behavior Retro is the Builder-owned Phase 5 closeout for system behavior, not project performance and not deliverable template evolution. It answers one question: what system changes were earned during the project and still need a Builder decision?

The artifact is a mini plan file. It should let the operator approve immediate execution, move approved work to Builder backlog, or mark items no-patch.

## Inputs

- Project reached Phase 5 per the selected `flow` in `project.md`.
- Final active deliverable is delivered, cancelled, or removed from project scope.
- Project `feedback-log.md` and `activity.md` exist, even if sparse.
- Operator has not chosen to skip or defer Builder closeout.

Read only the smallest set needed to classify Builder-owned system behavior:

- `system/builder-backlog.md` active unresolved entries; pull in any entries surfaced by this project or still blocking this project's workflow.
- Project `feedback-log.md`, limited to agent behavior, routing, mode, state, connector, audit, or process failures.
- Project `activity.md`, limited to overrides, deferrals, mode swaps, state repairs, and closeout events.
- `system/audit/agentframe.db` pending validations from recent `system_changes` rows when they affect this project's observed behavior.
- Current system/process/persona files only when a recommended change needs target-file proof before execution.
- Do not load voice profile; this is an analytical/operational doc.

The v1 to head deliverable diff analysis arrives via `system/skills/deliverable-harvest/SKILL.md` (the deliverable lens of the harvest pass that feeds this retro); do not re-derive it manually here.

## Output Shape

Use exactly these three sections.

### Recommended Changes

Use this item shape:

#### {Short Item Title}

- **Action:** What to patch, keep open, backlog, or reject.
- **Why:** The shortest evidence-backed reason this is system-level.
- **Target change:** The exact file or `BB-*` entry and kind of change.
- **Current state:** `needs decision`, `applied`, `backlogged`, or `no patch`.

- Start with the patch/backlog action, not a recap.
- `Current state` must be one of: `needs decision`, `applied`, `backlogged`, or `no patch`.
- Include existing unresolved `BB-*` items when they are relevant to this project.
- Include newly discovered Builder work when it is earned by project evidence and has not already been patched.
- If the user approves an item and it is practical to patch now, execute it immediately in Builder mode.
- If the user approves an item but defers execution, attach it to an existing `BB-*` item or append a new active Builder backlog entry before leaving the retro.

### Already Applied

Use the same item shape. Set `Current state` to `applied` and include validation status there.

- Include system/process/persona/schema/template patches already applied during the project when they close a Builder-owned issue.
- Include completed backlog-equivalent work even if it did not start as a formal `BB-*` item.
- Keep validation facts here only when they establish whether the applied patch still needs future evidence.

### No Patch Needed

Use the same item shape. Set `Current state` to `no patch`.

- Use for project-specific friction, rejected recommendations, low-confidence signals, or issues already covered by a stronger existing rule.
- Do not create backlog entries from this section.

### Approval Prompt

End the artifact with this exact prompt:

`Approve these recommended changes for execution now, move selected items to Builder backlog, or mark selected items no-patch?`

## Hard Constraints

- No project history recap unless it directly justifies one of the three buckets.
- No separate sections for process observations, template coherence, or backlog count. If those facts matter, classify them inside one of the three buckets.
- Builder backlog is an input/source and a disposition target, not its own report section.
- No inline arrow chains. Every item uses the four-bullet shape.
- Every recommended change names a target file or `BB-*` entry in `Target change`.
- At least one of the three buckets must contain an explicit decision; if there are no recommendations, say why under `No Patch Needed`.
- If an approved deferred item has no existing `BB-*`, append one to `system/builder-backlog.md` in the same turn.
- Log applied system behavior changes to `system/audit/agentframe.db` when they affect system behavior, schema, process files, templates, personas, or runtime machinery.

## Review Path

- **Reviewer:** operator approves, edits, defers, or rejects recommended changes inline.

## Draft Frontmatter Convention

Target file: `phase-5-launch-and-learn/system-retro-v{N}.md`

```yaml
---
status: <drafting | locked>
last_updated: <ISO-8601 timestamp>
---
```

Follow standard versioning conventions for snapshots. See `library/process/lock-event.md` for lock mechanics and project tracker updates.

## Lock Criteria

- The artifact uses exactly the three bucket sections.
- Every item uses the `Action` / `Why` / `Target change` / `Current state` bullet shape.
- Every item in `Recommended Changes` has a user decision or explicit deferred state.
- Approved immediate patches have been applied and logged.
- Approved deferred patches have an existing or newly appended active Builder backlog entry.
- `system-retro-v{N}.md` is saved to `phase-5-launch-and-learn/system-retro-v{N}.md` with frontmatter `status: locked`.
- Project tracker updated per `library/process/lock-event.md`.
