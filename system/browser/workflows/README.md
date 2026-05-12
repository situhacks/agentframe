# Browser Workflow Files

Workflow folders are memory for Cursor, not scripts.

Each `recipe.md` should tell a fresh-context Cursor agent what job to do, what browser surface to open, what inputs/outputs matter, what path was demonstrated or approved, and where to stop for human review. Cursor uses this file plus live browser observation to decide the next browser action.

Treat each workflow folder like a small skill folder:

```text
workflows/<workflow-id>/
  recipe.md        universal workflow instructions and routing
  browser-use/     browser-harness notes/artifacts
```

Load `recipe.md` first for every run. Load `browser-use/` only when the run needs local browser control through browser-harness.

Shared subroutines live under `_shared/`. App workflows can call those recipes instead of copying common behavior. Example: use `_shared/auth-handoff/recipe.md` for account picker, enterprise SSO, and human-secret handoff before returning to the app-specific workflow.

## Minimal Shape

```markdown
# {Workflow Name}

workflow_id: {stable-id}
status: learning | cursor_replay | promotion_candidate | promoted
app_url: {starting URL}
approval_mode: human_review | autonomous_when_whitelisted
tool_routing: browser_use_only

## Job
One sentence describing the user-visible outcome.

## Inputs
Only the files, folders, URLs, or operator choices needed to run.

## Path
1. Human-readable step.
2. Human-readable step.

## Known Controls
- Control label or observation hint.

## Human Gate
Where Cursor must stop for the operator unless the workflow has explicit autonomous approval.

## Promotion Notes
What evidence would justify changing this workflow's status.
```

Keep these files compact. If the workflow starts needing many branches, keep Cursor in the loop instead of over-hardening it.

Routine success/failure notes, timing, counters, screenshots, and run journals do not belong in `recipe.md`.

Put durable browser-harness quirks in `browser-use/notes.md` only when they are useful for future runs. Examples: a Microsoft web modal that appears during export, a browser download approval path, or a targeted DOM state check that avoids expensive page polling.

Do not use `browser-use/notes.md` as a diary. Do not put pixel coordinates there unless the control is genuinely unreachable through better signals.
