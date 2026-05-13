# Template: Template Evolution Retro

## Purpose

The Template Evolution Retro is the Builder-owned Phase 5 closeout for deliverable templates. It asks whether the current template failed to produce the reusable deliverable shape the operator needed.

Do not treat normal campaign content evolution as a template gap. Campaigns change. Templates should change only when the v1 to vF gap exposes a reusable missing section, ownership boundary, constraint, input, or lock check.

## Inputs

- **Voice Loading:** No. This is an analytical/operational doc.
- Campaign reached Phase 5 per the selected `campaign_flow` in `campaign.md`.
- At least one deliverable family in scope has both a first draft snapshot and a locked/current version.
- The relevant current deliverable template exists.
- For each deliverable family in scope, read only:
  - The current deliverable template at `library/deliverables/{family}/template.md`.
  - Campaign `v1` and `vF` pairs for that deliverable family.
  - Relevant campaign `feedback-log.md` entries for that deliverable family.
  - The template's `evolution.md` when it exists.
  - Related process/context files only when the gap points outside the deliverable template.

Do not analyze system behavior, mode routing, connector limits, or audit mechanics here. Those belong in `library/deliverables/system-retro/template.md`.

## Output Shape

**Scope:** Default to one template retro artifact (`phase-5-launch-and-learn/template-retro-v{N}.md`). Use `scope:` frontmatter to name what it covered (e.g., `copy-carousel`, `visual-design`). If more than two or three deliverable families are in scope, split the work across sessions, keeping the user-facing deliverable as `template-retro` unless multiple locked artifacts must coexist.

**Decision Filter:** For each meaningful v1 to vF delta, ask:
- Did the current template already ask for this? If yes, do not patch.
- Is the change reusable for future deliverables of this type? If no, do not patch.
- Is the issue really owned by an upstream file already loaded for the deliverable? If yes, do not restate that context in this template.
- Is the issue pure voice? If yes, exclude it unless `library/process/voice-mini-retro.md` failed to run or the template put voice-bearing copy in the wrong deliverable.
- Is the issue a missing section, ownership boundary, constraint, input, or lock check? If yes, it may earn a patch.

Use exactly these three sections:

- `Recommended Changes`
- `Already Applied`
- `No Patch Needed`

Each item must use this readable shape:

### {Short Item Title}

- **Action:** What to patch, keep open, backlog, or reject.
- **Why:** The shortest evidence-backed reason this is template-level, not normal campaign content drift.
- **Target change:** The exact file and kind of change.
- **Current state:** `needs decision`, `applied`, `backlogged`, or `no patch`.

No inline arrow chains. No separate evidence field. Put the evidence inside `Why`.

**Approval Prompt:** End the artifact with this exact prompt:
`Approve these template changes for execution now, move selected items to Builder backlog, or mark selected items no-patch?`

## Hard Constraints

- No campaign recap unless it directly justifies one of the three buckets.
- Do not mix system-behavior findings into this artifact.
- Do not patch a template just because campaign content changed.
- Do not restate upstream campaign context in downstream templates.
- Do not include pure voice findings (pure voice evolution is owned by `library/process/voice-mini-retro.md`).
- Every recommended change must name a target file and target change.
- If there are more than three deliverable families, split the work across sessions or explicit scoped artifacts.
- If an approved deferred item has no existing `BB-*`, append one to `system/builder-backlog.md` in the same turn.
- Log applied template/process/context changes to `system/audit/agentframe.db` when they affect system behavior, templates, process files, personas, context, schema, or runtime machinery.

## Draft Frontmatter Convention

**Canonical current file:** `phase-5-launch-and-learn/template-retro-v{N}.md`. Snapshots accumulate per the shared versioning convention.

Only template-specific fields are shown below. Reference `library/process/campaign-frontmatter.md` for standard schema.

```yaml
---
scope: <all | short scope slug>
families:
  - <deliverable-family>
---
```

## Lock Criteria

- The artifact uses exactly the three bucket sections.
- Every item uses the `Action` / `Why` / `Target change` / `Current state` bullet shape.
- Each in-scope deliverable family has at least one recommended, already-applied, or no-patch decision.
- Every recommended change has a user decision or explicit deferred state.
- Approved immediate patches have been applied and logged.
- Approved deferred patches have an existing or newly appended active Builder backlog entry.
- Campaign tracker updated per the selected `campaign_flow`.

