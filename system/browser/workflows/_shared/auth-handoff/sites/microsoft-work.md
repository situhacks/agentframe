# Microsoft Work Account

## Applies When

The browser is on Microsoft login/account-selection surfaces such as `login.microsoftonline.com`, `login.microsoft.com`, `login.live.com`, or an app redirecting through Microsoft work/school authentication.

## Known Deterministic Steps

- If `Pick an account` appears and the known work account is visible, click the known work account.
- If an email field appears and the operator has configured a non-secret work email, enter that email and continue.
- If `Work or school account`, `Next`, `Continue`, or equivalent non-secret routing controls appear, click only when the visible page clearly belongs to the Microsoft work/school flow.
- If the app returns directly to the requested app page, return to the calling workflow.

## Stop Conditions

Stop and call the operator when the page asks for:

- Password
- One-time code
- Authenticator code
- Microsoft Authenticator approval
- Recovery or security verification
- Any unexpected prompt that asks for a secret or private verification value

## Ready Condition

The requested Microsoft-backed app is visible again, not merely the Microsoft login page. For Outlook, that means mailbox controls such as `New mail`, `Inbox`, `Drafts`, or `Sent Items` are visible.

## Notes

SSO freshness varies. If the operator already authenticated earlier in the day or since the last system wake, this note may do nothing because the target app is already ready.
