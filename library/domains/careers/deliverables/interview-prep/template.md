# Interview Prep

## Purpose

One usable prep sheet per round: the openers this panel will actually pull on, the questions to ask back, and the two or three risks specific to this room. Internal only, never submitted, and sized to be re-read in the ten minutes before the call.

## Before Writing

Read these first; a prep sheet written without them repeats a failure the bank already recorded.

- `library/context/operator/career/interview-playbook.md` — the standing cross-round reminders. Its repeating patterns, do-not-say list, and standing cautions are binding here, and this sheet does not restate them. Pull only the drill-set rows this JD earns.
- `proof-points.md` — every number in this sheet cites a `pp-NNN`. An uncited number does not go in the room.
- `jd-map.md` — the tier-1 requirements and the coverage choice already made.
- `company-brief.md` — refresh `## Now` if it was drafted more than ~30 days ago.

## Inputs

- The board row's stage and round type (recruiter screen, hiring manager, client round, panel, technical).
- Interviewer names and roles when known, since register is set per questioner.
- The active track file under `tracks/` for framing and tone.

## Output Shape

1. `## This room` — round type, who is in it, what each one owns, and the register each needs. One line per interviewer.
2. `## Openers` — the three to five drill-set rows this JD earns, copied from the playbook with their `pp-NNN` citations. Number first. Mark any that need rehearsing out loud.
3. `## Likely questions` — the questions this specific JD and company brief imply, each with the opener or proof-point that answers it. Not a generic question bank.
4. `## Risks in this room` — the two or three things most likely to go wrong *here*: a title-versus-brief mismatch, a gap the JD probes directly, a caution from the playbook that this panel will test.
5. `## Ask back` — the scoping question for the first three minutes, plus two or three real questions for the end.
6. `## Facts to have ready` — comp number, notice period, relocation answer, availability. Decided before the call, never improvised.

## Hard Constraints

- **One screen per section.** If a section needs scrolling it will not be read in the room. Prep that has grown past a few hundred lines has become a research document and stopped being prep.
- Every number cites `pp-NNN`. Never introduce a figure that is not in the bank.
- Honour the playbook's standing cautions exactly: unverified proof-points stay out, and no claim to a level or title not yet earned.
- Do not restate the playbook. This sheet is the per-round selection from it.
- Not a submission material: it never joins `materials:`, has no export gate, and no filed finals.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready | deferred>
last_updated: <ISO date>
round: <recruiter-screen | hiring-manager | client | panel | technical>
interview_at: <ISO datetime, when scheduled>
---
```

## Readiness Criteria

Not ready-gated; it is internal prep, so `af ready` is unnecessary. One version per round, `interview-prep-v{N}.md`, since a second round is a new sheet rather than an edit of the first.

## After The Round

Two moves, in this order. Skipping the second is how the same lesson gets re-derived a month later.

1. Per-call detail stays here or in a sibling post-mortem in this application folder.
2. Anything that **repeats** across rounds goes to the playbook's append block via [`career-harvest`](../../../../process/career-harvest.md): the repeated pattern, any new do-not-say phrase, and which drill-set row worked or was missing.
