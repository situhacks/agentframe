---
name: email-cleanup
description: Runs an inbox cleanup across any of his inboxes (Gmail, Outlook or Microsoft 365, or the browser) in a fixed order with his approvals (census, unsubscribe, trash old promotions, delete dormant accounts, record). Use when he asks to clean up, scan, or declutter an inbox, unsubscribe from mail, or close old accounts found in his email.
---

This sets the order and the approvals. How to drive each mail client is yours to work out.

## 1. File the order

File his instruction verbatim in `workspace/life/sources/orders/` per [`source-record.md`](../../../library/schema/source-record.md). Every loop and log below cites it.

## 2. Census, then his marks

Search each inbox by sender over recent years. Group the senders into keep, unsubscribe, delete the account (dormant services), and security flags (breach alerts, unknown sign-ins, new devices, changed recovery details). Show him the grouped table. His marks on it approve the unsubscribes, the trashing, and the deletion requests. Report security flags to him; never act on them.

## 3. Unsubscribe

Use the mail client's own unsubscribe first (Outlook's Subscriptions page, Gmail's unsubscribe button). Next, the unsubscribe link in the sender's latest email, when opening it alone unsubscribes. Anything that needs a form submit, a login, or a CAPTCHA goes on a short list for him. Never block a sender or create a rule unless he asks. Hand the browser clicking to a subagent, its model chosen by [the model rule](../../../library/schema/models.md).

## 4. Trash old promotions

Trash old promotional mail from the unsubscribed senders, soft delete only. Keep receipts, orders, bills, and account and security mail.

## 5. Delete dormant accounts

A `researcher` finds each service's privacy contact from a primary source. Send a deletion request from the address on the account: cite PIPEDA, plus GDPR or CCPA where they apply, and ask for confirmation by reply. Or use the company's own form, filled with only name, email, and region. His yes on the list covers these sends. Anything that needs a login goes to him.

## 6. Record

- Write a sent log in the accounts system folder, `workspace/life/systems/accounts/`.
- Run `af open add --section "Waiting on others"` for the replies and any verification asks, nudged in about two weeks; PIPEDA gives them 30 days.
- Update the cleanup loop with what was done and what is left for him.

## 7. Settings last

Change forwarding or other mailbox settings only when he asked, and only after the rest.

## Hard limits

- Never type or use a password, even a leaked one, and never log in for him.
- Never submit a form reached from an email link, and never solve a CAPTCHA.
- Never click a link inside an email body, except an unsubscribe link in step 3.
- Chrome site permissions are his to grant.
- Nothing outward-facing beyond what he approved.
