# Local Browser Runtime

This is the controlled Edge browser runtime for AgentFrame Marketing when an approved API/MCP/CLI cannot do the job and the operator can use the same workflow in a normal browser.

Cursor remains the reasoning layer. `browser-harness` is the local browser hand: it connects to the controlled Edge Work Browser through CDP and lets Cursor inspect page state, execute DOM/control actions, type, click, manage tabs, and capture screenshots when needed.

Do not treat `browser-harness` as the Browser Use Cloud agent. This workspace does not rely on Browser Use Cloud, Browser Use model keys, or external LLM provider keys for browser workflows.

## Active Surface

- `src/work-browser.js` launches or reuses the controlled Microsoft Edge Work Browser.
- `src/run-evidence.js` writes small redacted run artifacts.
- `workflows/*/recipe.md` stores stable workflow memory and tool-routing guidance. It is not a run journal.

Legacy/retiring surfaces still exist while the local recorder replacement is designed:

- `src/playwright-runtime.js`
- `src/playwright-primitives.js`
- `src/demo-recorder.js`
- `src/demo-compactor.js`

Do not make Playwright the default path for new browser workflow work. Keep any temporary use explicit and bounded to recorder compatibility until a local `browser-harness` recorder exists.

## Workflow Folder Convention

Treat each workflow folder like a small skill folder:

```text
workflows/<workflow-id>/
  recipe.md        universal workflow instructions and routing
  browser-use/     browser-harness notes/artifacts
```

Load `recipe.md` first for every run. Load `browser-use/` only when browser-harness is being used. Keep these notes lean: durable tool quirks, helper assumptions, and repair patterns only.

Write recipes for a fresh-context agent:

- What job is being done.
- What inputs and outputs matter.
- What path the operator demonstrated or approved.
- Where the agent must stop for the operator.

Do not put run journals, screenshots, timing logs, or low-level browser mechanics in `recipe.md`.

## Browser-Harness Interaction Policy

Use the lightest reliable browser signal:

1. Inspect accessible controls, labels, focused element state, URLs, tabs, and targeted DOM when available.
2. Use screenshots to confirm visual state, resolve ambiguity, or inspect controls that do not expose useful DOM.
3. Use coordinate clicks only as a fallback for browser chrome, canvas-like surfaces, or controls that cannot be reached semantically.

Avoid full-page DOM/text extraction on large streaming pages. Prefer targeted checks around the active control or region.

## Retired Surfaces

The old `dist/` tree was legacy build output from the overbuilt path and has been removed from the active workspace. Do not recreate or import these patterns for new browser fallback work:

- Deterministic Outlook replay and POCs: `recipe-replay.js`, `outlook-*.js`, `outlook-semantic-recipe.js`.
- Custom mini-platform contracts: `browser-action-protocol.js`, `browser-action-executor.js`, `browser-primitive-contract.js`, `browser-promotion-contract.js`, `supervised-adaptive-runner.js`.
- Stagehand runtime experiments: `stagehand-runtime.js`, `stagehand-connect-smoke.js`.

Do not add new app-specific runners by default. If a workflow repeats enough that Cursor token cost becomes wasteful, first improve the recipe and browser-harness notes so fresh agents can execute it reliably. Keep Cursor in the loop unless a local, non-Playwright replay surface is explicitly designed.
The evaluated `agent-browser` Workfront replay artifact is not part of the active runtime.

## Default Loop

1. Read the workflow `recipe.md`.
2. Read `browser-use/notes.md` only if the browser is needed.
3. Start or attach to the controlled Edge Work Browser.
4. Use browser-harness to execute the workflow with live page state.
5. Update workflow notes only when the run reveals a durable routing, auth, selector, or repair pattern that helps future runs.

Routine success/failure notes, timing, and counters are not durable workflow memory and should not be written into `recipe.md`.

`local/` is temporary runtime storage for the controlled Edge profile, browser marker, screenshots, and one-off debug files. It is not workflow memory.
