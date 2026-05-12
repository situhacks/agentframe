# Browser Fallback Process

Lazy-load this file when Composio/Rube MCP, another approved API/MCP, or an approved CLI is unavailable and the operator asks Cursor to use a browser workflow instead.

## Principle

Cursor is the brain. Playwright is the default fast hand. Local browser-use/browser-harness is the rescue/ad hoc hand.

The browser fallback copies the Stagehand pattern without using Stagehand as the runtime: observe the browser, let an LLM decide the next step, execute one bounded browser primitive, observe again, and update workflow memory only when something durable changes. Do not introduce a new app-specific script when a workflow file plus generic browser primitives is enough.

## When To Use

Use browser fallback only after checking that Composio/Rube or another native integration is not available or not worth wiring for the current task. Prefer approved API, MCP, or CLI when they exist.

Use browser fallback for visible web-app work the operator can already do manually: Outlook, calendar, SharePoint-style browser surfaces, or similar enterprise apps where SSO/session access works in Edge.

Classify each new browser workflow before recording:

- `playwright_first`: repeatable demonstrated sequences where success is mostly "do these known steps again."
- `browser_use_first`: volatile or interpretation-heavy work where the target changes by row, comment, queue, board state, or user intent.

If unsure, choose `playwright_first` only when the operator can demonstrate a clear replay path. Otherwise choose `browser_use_first`.

## Record Loop

1. Start the tracker from `system/browser`:

   ```powershell
   npm run record -- --workflow-id=<workflow-id> --start-url=<app-url> --workflow-path=workflows/<workflow-id>/recipe.md
   ```

2. Tell the operator to demonstrate the workflow in the controlled Edge window.
3. Stop the tracker:

   ```powershell
   npm run record:stop
   ```

4. Compact the run:

   ```powershell
   npm run compact
   ```

5. Read the latest `actions.json` and update `workflows/<workflow-id>/recipe.md` with the durable steps, known controls, human gate, and tool routing. Do not add run notes.

## Replay Loop

1. Read the workflow `recipe.md`.
2. If the selected tool has workflow-specific notes, read only that subfolder: `workflows/<workflow-id>/playwright/` or `workflows/<workflow-id>/browser-use/`.
3. Ensure Work Browser Mode is running for the workflow's `app_url`.
4. If the target app is not authenticated, run `system/browser/workflows/_shared/auth-handoff/recipe.md` before app-specific steps.
5. Observe the page.
6. Decide the next browser primitive from the workflow file plus live observation.
7. Execute one primitive: observe, click, fill, press, insertText, or screenshot.
8. Observe again before deciding the next primitive.
9. Stop at the workflow's human gate unless the operator explicitly authorized autonomous completion for this run.
10. Update `recipe.md` only when the workflow shape or routing changes. Put durable tool-specific quirks in the tool subfolder instead.

If Playwright replay fails from selector drift, overlay behavior, option drift, or other same-page UI movement, call local browser-use/browser-harness to rescue inside the same controlled Edge Work Browser. If browser-use completes a human-like path, capture the durable correction back into the Playwright recipe/path. If browser-use uses deep CDP/JS/helper behavior, keep that memory in a `browser-use/` artifact for the workflow and do not pretend Playwright can replay it. If the same failure family needs browser-use rescue more than two consecutive times, mark the step or workflow `browser_use_first`.

For text fields that may already contain user-owned content, signatures, templates, or disclosures, prefer insertion over replacement. In practice: focus the field, move to the insertion point, then `insertText`. Do not use `fill()` unless replacing the entire field is the intended behavior.

## Shared Auth Handoff

Authentication is a shared browser workflow, not an app-specific recipe step. Use `system/browser/workflows/_shared/auth-handoff/recipe.md` when the browser lands on account picker, SSO, or login pages. Site-specific callouts live in that workflow's `site-index.md` and `sites/` notes.

The auth handoff may click deterministic, non-secret routing controls such as a known work account, `Next`, `Continue`, or `Single sign-on`. It must stop at password, one-time code, authenticator approval, security question, device compliance, or any other human-secret boundary. Do not store credential values, auth codes, session tokens, or full auth callback URLs.

## Hardening Rule

Do not harden upfront. Cursor-driven replay is the default while the workflow is fluid.

Promote only when the same workflow succeeds repeatedly and the cost of Cursor reasoning is the bottleneck. The default bar is three similar successful runs, but operator judgment wins. Promotion means Playwright-native fast replay artifacts under the workflow folder, not a new Outlook/email/calendar script and not run history inside `recipe.md`.

If promoted replay fails, fall back to Cursor-driven replay from the workflow file and live observations, then Browser Use rescue if needed. Do not add a second deterministic verifier stack.

## Anti-Patterns

- One script per app task.
- Browser-action schemas that reject useful Cursor judgment before the first real workflow is stable.
- Hard-coded semantic gates for every app.
- Treating Stagehand as an API dependency for this system.
- Adding security machinery beyond the simple approval mode already declared in the workflow file.
- Treating `recipe.md` as a run journal, promotion counter, or telemetry sink.
