# Browser Workflow Files

Workflow folders are memory for Cursor, not scripts.

Each `recipe.md` should tell a fresh-context Cursor agent what job to do, what browser surface to open, what inputs/outputs matter, what path was demonstrated or approved, what durable browser/app quirks affect execution, and where to stop for human review. Cursor uses this file plus live browser observation to decide the next browser action.

Browser-harness mechanics come from `system/skills/browser-harness/SKILL.md`. Workflow recipes add the repeatable use-case prompt on top: what to do, where to stop, and which browser signals are reliable for this specific app flow.

Treat each workflow folder like a small skill folder:

```text
workflows/<workflow-id>/
  recipe.md        workflow instructions, browser path, durable quirks, and routing
  runs/            optional one-off run captures when a human deliberately saves them
```

Load `recipe.md` for every run. The recipe is the single execution contract for that workflow; do not split durable browser-harness quirks into sidecar notes.

## Create A Workflow

Create a workflow only after a real run or operator-provided recording establishes the shape. Do not invent a browser workflow from desired end state alone.

1. **Record / learn.** Run the task once with the operator watching, or inspect an operator-provided recording. Use browser-harness normally; save screenshots or run captures under `runs/` only when they are useful evidence.
2. **Extract durable shape.** Keep only reusable facts: start URL, required inputs, safe auth boundaries, user-visible controls, long waits, export/download paths, app-specific traps, and human gates.
3. **Choose per-step signals.** For each risky step, say whether the agent should use screenshots, coordinate clicks, targeted DOM/aria, keyboard input, downloads inspection, or a mix. Do this in the recipe because the right signal depends on the app surface.
4. **Write `recipe.md`.** Convert the learned shape into the minimal sections below. Do not include the run diary.
5. **Replay once.** Run from the recipe in a fresh context. Patch the recipe when replay reveals durable behavior; discard one-off timing and screenshots.

If a workflow becomes too branchy, keep Cursor in the loop rather than turning the recipe into a brittle script. Promote helper code only when it is smaller than repeated reasoning.

## Minimal Shape

```markdown
# {Workflow Name}

workflow_id: {stable-id}
status: learning | cursor_replay
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

## Browser-Harness Execution
- Per-step signal strategy: screenshot, coordinate click, targeted DOM/aria, keyboard input, download inspection, or mixed.
- Durable browser/app quirk or repair pattern.
- Efficient observation strategy for long waits.

## Human Gate
Where Cursor must stop for the operator unless the workflow has explicit autonomous approval.
```

Keep recipes compact but complete enough for a fresh-context agent. If the workflow starts needing many branches, keep Cursor in the loop instead of over-hardening it.

Routine success/failure notes, timing, counters, screenshots, and run journals do not belong in `recipe.md`. Save one-off captures under `runs/` only when a human deliberately asks for the run artifact.

Patch the recipe when a run reveals a durable browser-harness quirk. Examples: a Microsoft web modal that appears during export, a browser download approval path, or a targeted state check that avoids expensive page polling.

Do not store durable workflow behavior in sidecar notes. Do not record pixel coordinates unless the control is genuinely unreachable through better signals.
