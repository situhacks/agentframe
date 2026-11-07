---
name: split-bill
description: Splits a shared bill he paid and asks for repayment in a group chat, with his e-transfer email as its own message. Use when he shares a receipt or final total, says who was there, and asks to send the split or his e-transfer details.
---

He paid and wants the others to pay him back. Money only moves when they send it; never send or request it from his bank.

## 1. Check the number

The amount to split is the final amount charged. On a receipt, add the lines: subtotal, tax, tip, and any card surcharge must reach the total on the card slip. If the math or his stated total disagrees, tell him and send nothing. If he gives only a total, split that and leave out the breakdown. If a receipt has no card slip and he gives no final amount, ask for it, because the tip is missing.

## 2. Split it

Split evenly unless he says otherwise. Count him. Round each share down to the cent; he covers the leftover cents. State the amount he should get back.

## 3. Match the names

Open the chat he named and read its member list. Use each person's name as the chat shows it, because dictation mangles names. If a name matches no member, use the recent messages and who was at the outing, and tell him in your reply which name you changed. If it is still unclear, ask him before sending.

Group chats often include people who were not there, so the message names who owes.

## 4. Send two messages, in this order

Read the e-transfer email from the line in `workspace/life/knowledge/money.md` that says which email to give friends. It is the Interac auto-deposit address, and it may differ from his main inbox. Do not take it from the session context or an old message, and do not write it in a tracked file.

1. The bill: the final total, its breakdown, the share, the number of people, and the names of who owes. End with "you can e-transfer me at the email below." Keep it short and plain.
2. The email address alone, with no other words, so it copies cleanly.

Before each send, check the chat title, the text in the box, and that no mention popup is open. Then send, and confirm it appears in the thread. His own message in this chat, naming the group chat and asking you to send, is his yes. Otherwise show him both messages first and wait.

Do not attach the card slip: it shows card digits. Send the itemized receipt only if he asks, after checking it for card digits.

## 5. Record it

- Run `af place add` if the venue is new, and `af person add` for anyone with no page. Then run `af place visit` with `--with` naming the group's slug and who came.
- Run `af open add` with `--section "Waiting on others"`, `--from` the source id, the names, the share, and a nudge in three days.

When you review that loop, search Gmail for Interac notices and match each to a name and share before asking him. Auto-deposit notices can go to the other inbox, so no notice in Gmail does not prove someone hasn't paid. Close the loop when everyone has paid.
