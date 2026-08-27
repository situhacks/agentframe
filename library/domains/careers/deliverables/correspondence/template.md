# Correspondence

## Purpose

One outward message to a human at the target company — recruiter reply, post-round follow-up, thank-you,
scheduling note. It exists so the research already done actually reaches the person, instead of being
re-invented from the last application's file every time.

Lives at `correspondence/{kind}-to-{firstname}-{YYYY-MM-DD}.md`, where `kind` is `reply`, `followup`,
`thankyou`, or `logistics`. Threads accumulate, so the folder is the thread and the date is the sort.

## Inputs

Read all four before drafting. A message written without them repeats a failure already on the record.

- `jd.md` — what was actually posted.
- `company-brief.md` — the living company dossier. Its **conclusions** are what may appear; its content is not.
- `jd-map.md` — coverage and the gap stop. When the board carries a better-fitting req than the one named,
  that finding lives here and is the whole reason the message is worth sending.
- `library/context/operator/voice/` — outward-facing and user-voiced. Voice load plus a humanizer pass are
  required before this is shown to the operator.

## Output Shape

The five moves, in order. Skipping one is how this drifts back into a cover letter.

1. **Research signalled by conclusion only.** Never recite the target's product, market, or mission back
   to them. A company does not need to be told what it sells. State the conclusion the research produced
   and let it imply the work behind it.
2. **Alignment as a category of work.** Never scope, never the employer's name, never a metric. A number
   turns the paragraph into a resume point and drops the register from peer to applicant.
3. **The honest bound, once, flat — only when it is load-bearing.** State it when it explains why the
   message exists. A bound attached to nothing is a volunteered negative. Never open with one.
4. **Route, do not self-reject.** When the named req is a weak match and a better one exists, name the
   better one and keep both doors open.
5. **Close on logistics.** Availability, next step, or a direct question. No sentiment stamp.

Then, below the message body and never sent:

- `## Call questions` — findings the message deliberately did not use, phrased as questions to ask live.
  Contradictions in the posting, unexplained tenure of a req, a band that conflicts with the body text.
  This block is why research does not evaporate when it does not fit the reply.

## Hard Constraints

- **Not a submission material.** Never joins `materials:`, no export gate, no `af ready`, no filed finals.
  Same carve-out `round-sheet` has.
- **The profile generated the inbound.** Re-pitching it is redundant and costs register. No capability
  lists, no evidence paragraphs, no "I'd be a great fit."
- **No metric in the body.** Metrics belong in the round-sheet and the resume, not here.
- **Freeform, never versioned.** One file per message, edited in place until sent. No `-v{N}` suffix —
  that filename shape hands the file to the version guard and the af tracker, and this is neither.
- Once sent, the file is the sent copy and is canonical. Record any correction as a new dated message.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | sent>
last_updated: <ISO date>
kind: <reply | followup | thankyou | logistics>
to: "<Name, Title>"
channel: <email | linkedin | sms>
sent_at: <ISO date, once sent>
---
```

## Readiness Criteria

Not ready-gated. It is internal until the operator sends it, and the operator sends it. Before it is
surfaced for sending: voice load done, humanizer pass run, all five moves present or deliberately
waived, and the call-questions block populated with whatever the message left on the table.
