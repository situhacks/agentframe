# Shared Auth Handoff

workflow_id: shared-auth-handoff
status: learning
approval_mode: human_secret_boundary

## Job

Get from a requested app URL to an authenticated app-ready page using deterministic visible browser steps where safe, then stop when a human-secret prompt appears.

## Scope

This workflow is shared across browser fallback workflows. App workflows call it before their app-specific steps when the target page is not already authenticated.

## Default Path

1. Open or reuse the target app URL in Work Browser Mode.
2. Observe the page and classify it as one of:
   - `app_ready`
   - `account_picker`
   - `email_entry`
   - `enterprise_sso`
   - `human_secret_required`
   - `unknown_auth_state`
3. If the target app is already ready, return to the calling workflow.
4. If an account picker appears and a known work account is visible, click it.
5. If an email field appears, use the configured non-secret work email if available; otherwise ask the operator which email to use.
6. If a non-secret routing button appears (`Next`, `Continue`, `Single sign-on`, `Sign in with SSO`, `Work or school account`), click it when the site note says it is expected.
7. Stop and call the operator at the first human-secret boundary.
8. After the operator completes the secret step, observe again. If the target app is ready, return to the calling workflow; otherwise continue from step 2.

## Human-Secret Boundary

Never type, record, or store:

- Passwords
- One-time codes
- Authenticator numbers
- Security question answers
- Recovery codes
- Session tokens
- Full auth callback URLs or query strings

At this boundary, tell the operator what the browser is asking for in coarse terms: password, code, authenticator approval, or unknown auth challenge.

## Site Notes

Before taking deterministic auth steps, check `site-index.md` for the current host and load the matching site note. Site notes should stay tiny: known non-secret clicks, stop conditions, and ready condition.

## Ready Condition

The target app's normal authenticated UI is visible. The calling workflow owns the exact app-ready check, such as Outlook mailbox controls, calendar controls, or SharePoint document library controls.

## Promotion Notes

Do not promote this into a deterministic auth script. Auth flows differ by host, policy, device state, and recent SSO freshness. Harden by adding small site notes and better classification, not by storing secrets or automating human-secret prompts.
