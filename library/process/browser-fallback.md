# Browser Fallback Process

Lazy-load this file when Composio/Rube MCP, another approved API/MCP, or an approved CLI is unavailable and the operator asks Cursor to use a browser workflow instead.

## Principle

Cursor is the brain. Local browser-use/browser-harness is the browser hand.

Browser fallback is not an app-specific automation stack. The agent reads the workflow recipe, observes the controlled Edge Work Browser through browser-harness, executes one bounded browser action, observes again, and updates workflow memory only when something durable changes.

Do not introduce new browser scripts by default. Improve the workflow recipe and `browser-use/notes.md` first.

## When To Use

Use browser fallback only after checking that Composio/Rube or another native integration is not available or not worth wiring for the current task. Prefer approved API, MCP, or CLI when they exist.

Use browser fallback for visible web-app work the operator can already do manually: Outlook, calendar, SharePoint-style browser surfaces, or similar enterprise apps where SSO/session access works in Edge.

## Execution Loop

1. Read the workflow `recipe.md`.
2. Read `workflows/<workflow-id>/browser-use/notes.md` only when browser-harness-specific quirks are needed.
3. Ensure Work Browser Mode is running for the workflow's `app_url`. If setup is unclear, load `system/browser/README.md` and run `npm run work-browser` from `system/browser`.
4. If the target app is not authenticated, stop at the human-secret boundary and ask the operator to complete sign-in in the controlled browser.
5. Observe the page.
6. Decide the next browser primitive from the workflow file plus live observation.
7. Execute one primitive: observe, click, fill, press, insertText, or screenshot.
8. Observe again before deciding the next primitive.
9. Stop at the workflow's human gate unless the operator explicitly authorized autonomous completion for this run.
10. Update `recipe.md` only when the workflow shape or routing changes. Put durable browser-harness quirks in `browser-use/notes.md`.

For text fields that may already contain user-owned content, signatures, templates, or disclosures, prefer insertion over replacement. In practice: focus the field, move to the insertion point, then `insertText`. Do not use `fill()` unless replacing the entire field is the intended behavior.

## Auth Handoff

Authentication is a human-owned boundary. The agent may route through deterministic, non-secret controls such as a known work account, `Next`, `Continue`, or `Single sign-on` only when the workflow recipe explicitly says those controls are safe.

Stop at password, one-time code, authenticator approval, security question, device compliance, or any other human-secret boundary. Do not store credential values, auth codes, session tokens, or full auth callback URLs.

## Hardening Rule

Do not harden upfront. Cursor-driven browser-harness execution is the default while the workflow is fluid.

Promote only when the same workflow succeeds repeatedly and the cost of Cursor reasoning is the bottleneck. Promotion means improving the workflow recipe or browser-harness notes, not adding a new Outlook/email/calendar script and not adding run history inside `recipe.md`.

If a hardened note fails, fall back to Cursor-driven execution from the workflow file and live observations. Do not add a second deterministic verifier stack.

## Anti-Patterns

- One script per app task.
- Browser-action schemas that reject useful Cursor judgment before the first real workflow is stable.
- Hard-coded semantic gates for every app.
- Adding any separate browser automation framework for this system.
- Adding security machinery beyond the simple approval mode already declared in the workflow file.
- Treating `recipe.md` as a run journal, promotion counter, or telemetry sink.
