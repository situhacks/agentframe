# Workfront Login

## Applies When

The browser is on `deloittedigitalus.my.workfront.com` and the normal Workfront UI is not ready. Workfront may preserve login for the browser/computer session, so most runs should start by trying the app URL and observing readiness before assuming auth is required.

## Known Deterministic Steps

- Open or refresh `https://deloittedigitalus.my.workfront.com/home/workspaces`.
- If Workfront is already ready, return to the calling workflow.
- Workfront username is expected to be empty on a fresh browser/computer session. If a configured non-secret Workfront email is available, enter it and continue.
- If the username field is unexpectedly prefilled, leave it unless the operator says it is wrong.
- If non-secret routing controls appear (`Next`, `Continue`, `Log in`, `Sign in`), click them only when the page clearly belongs to Workfront.

## Stop Conditions

Stop and call the operator when the page asks for:

- Workfront password
- MFA, one-time code, or private verification
- Any unexpected prompt that asks for a secret value

At the password prompt, tell the operator: "Workfront is ready for your password. Please enter it in the browser, then tell me when you're done."

If the operator enters a wrong password and retries, keep treating the password step as human-owned. Do not record, store, or replay the password.

## Ready Condition

Workfront's authenticated UI is visible. Good ready signals include:

- `Main Menu`
- `Requests`
- `/home/workspaces`
- `/requests/submitted`
- Workfront home text such as `Good morning` / `Good afternoon`
