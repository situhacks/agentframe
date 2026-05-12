# AgentFrame Browser Runtime

This folder is AgentFrame's thin browser integration layer. Use it only when an approved API/MCP/CLI cannot do the job and the operator can perform the same task in a normal browser.

Cursor remains the reasoning layer. The vendored [`browser-harness`](../skills/browser-harness/) skill is the local browser hand: it connects to the controlled Edge Work Browser through CDP so Cursor can inspect page state, execute bounded actions, manage tabs, and capture screenshots when needed.

## Active Surface

- `src/work-browser.js` launches or reuses the controlled Microsoft Edge Work Browser.
- `workflows/README.md` defines the workflow-memory convention for future workflow folders.

## Runtime Storage

The following paths are local runtime state and are not workflow memory:

- `local/` - controlled Edge profile, marker file, screenshots, and debug files.
- `cache/` - disposable install/cache state.
- `node_modules/` - local JavaScript dependencies.
- `workflows/**/runs/` - one-off run captures when a human deliberately saves them.

These paths are ignored by the repository and should not be published.

## Workflow Folder Convention

Workflow folders are memory for Cursor, not scripts:

```text
workflows/<workflow-id>/
  recipe.md        workflow instructions and routing
  browser-use/     browser-harness notes/artifacts
```

Load `recipe.md` first for every run. Load `browser-use/notes.md` only when the workflow needs browser-harness-specific quirks. Keep both files lean.

Write recipes for a fresh-context agent:

- What job is being done.
- What inputs and outputs matter.
- What browser path is approved.
- Where the agent must stop for the operator.

Do not put run journals, screenshots, timing logs, or low-level mechanics in `recipe.md`.

## Browser-Harness Interaction Policy

Use the lightest reliable browser signal:

1. Inspect accessible controls, labels, focused element state, URLs, tabs, and targeted DOM when available.
2. Use screenshots to confirm visual state, resolve ambiguity, or inspect controls that do not expose useful DOM.
3. Use coordinate clicks only as a fallback for browser chrome, canvas-like surfaces, or controls that cannot be reached semantically.

Avoid full-page DOM/text extraction on large streaming pages. Prefer targeted checks around the active control or region.

## Auth And Human Boundaries

The agent may route through deterministic, non-secret controls only when the workflow recipe says those controls are safe.

Always stop at passwords, one-time codes, authenticator approvals, security questions, device compliance, permission prompts, or any other human-secret boundary. Do not store credential values, auth codes, session tokens, or full auth callback URLs.

## Default Loop

1. Read the workflow `recipe.md`.
2. Read `browser-use/notes.md` only if browser-harness-specific quirks are needed.
3. Start or attach to the controlled Edge Work Browser with `npm run work-browser`.
4. Use browser-harness to execute the workflow with live page state.
5. Update workflow notes only when the run reveals a durable routing, auth, selector, or repair pattern that helps future runs.

Routine success/failure notes, timing, counters, screenshots, and run captures are not durable workflow memory. The practical test is whether the operator-visible browser-harness workflow completes the requested job.
