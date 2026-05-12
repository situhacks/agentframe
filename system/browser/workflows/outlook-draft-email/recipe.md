# Outlook Draft Email

workflow_id: outlook-draft-email
status: cursor_replay
app_url: https://outlook.office.com/mail/
approval_mode: human_review

## Job

Draft an Outlook email in the operator's controlled Edge work browser, then stop before Send so the operator can review and send manually.

## Tool Routing

Use browser-harness in the controlled Edge Work Browser. Cursor remains the reasoning layer and must stop before `Send` unless the operator explicitly authorizes sending.

## Demonstrated Path

1. Open or reuse Outlook Web in Work Browser Mode.
2. If Outlook is not already authenticated, run `workflows/_shared/auth-handoff/recipe.md` with the Outlook URL and return here only when the mailbox is ready.
3. Click `New mail` or the equivalent compose control.
4. Fill the recipient using the runtime request.
5. Fill the subject using the runtime request.
6. Insert the body at the top of the message body editor using `insertText` with `position: "start"` and `trailingBlankLine: true`. Do not use `fill()` for the body; Outlook injects the operator's signature before typing, and `fill()` replaces it.
7. Observe the draft and confirm recipient, subject, and body are visible.
8. Stop before Send and report that the draft is ready for human review.

## Known Controls

- `New mail` or `New message` button opens compose.
- Recipient field may appear as `To`, a recipient textbox, or a resolved recipient pill.
- Subject field usually exposes `Subject` or `Add a subject`.
- Body field usually exposes `Message body`, `Start typing`, or a contenteditable editor.
- `Send` is a human gate in the default workflow.

## Human Gate

Default behavior is draft-only. Cursor must stop before Send unless the operator explicitly says this workflow is approved for autonomous send for the current run or this workflow has been separately promoted and whitelisted.

## Promotion Notes

Only change promotion status if this exact draft-email shape repeats enough that Cursor-token cost is the problem and a local non-Playwright replay path has been designed.

Promotion is not earned yet. Run at least two more signature-preserving drafts across normal Outlook states before changing this recipe's status.
