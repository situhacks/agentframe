# Deloitte SSO

## Applies When

The browser shows a Deloitte-branded sign-in, enterprise SSO, conditional access, or Microsoft work/school prompt for the operator's Deloitte account.

## Known Deterministic Steps

- If a Deloitte work email field appears and a configured non-secret work email is available, enter it and continue.
- If a `Single sign-on`, `SSO`, `Continue`, `Next`, or `Work or school account` routing button appears, click it only when the page clearly belongs to the Deloitte/Microsoft work authentication flow.
- If Windows-connected SSO completes automatically, wait for the target app to load and return to the calling workflow.

## Stop Conditions

Stop and call the operator when the page asks for:

- Deloitte password
- Microsoft Authenticator code or approval
- One-time code
- Device compliance action
- Any private verification value

## Ready Condition

The target app is visible again and no Deloitte/Microsoft auth challenge remains.

## Notes

Do not record password, one-time code, authenticator number, full callback URL, or token-bearing query strings. Only store coarse state transitions like `clicked known work account`, `auth code required`, and `target app ready`.
