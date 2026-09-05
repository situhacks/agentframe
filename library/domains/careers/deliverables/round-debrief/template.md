# Round Debrief

## Purpose

What actually happened in the round, and — the part that makes it worth writing — where each thing
learned has to go so the next round starts from a current picture instead of re-deriving one.

Lives at `round-{N}-{name}/debrief.md`, beside the prep it grades.

## Inputs

- The round's transcript, filed in `sources/` and registered in `sources/INDEX.md`. Written from the
  recording, not from memory.
- `round-sheet.md` — what was planned, so the gap between plan and room is visible.
- `library/context/operator/career/interview-playbook.md` — the standing cross-round patterns this
  round gets tested against.

## Output Shape

1. `## Headline` — the outcome in two or three sentences, including anything said verbatim that decides
   the next step. Quote exactly; a paraphrased commitment is not a commitment.
2. `## What happened` — the substance, in the order it mattered rather than the order it occurred.
3. `## Pattern check` — this round against the playbook's recorded patterns. Name each one that held and
   each that recurred. A pattern that has now failed in three rooms is the most important line in the file.
4. `## What worked` — and it is stated explicitly, because the next round's prep will otherwise
   over-correct away a move that is already landing.
5. `## What was missed` — prepared material that never came out, questions never asked, numbers never said.
6. `## Room posture` — how the room behaved, in observable terms rather than a score. A screening room
   sounds evaluative and guarded. A selling room shifts to future tense about the work, volunteers
   internal detail nobody asked for, and moves scheduling faster than its own process. Name which this
   was and quote the behaviour that says so. A posture read with no quoted behaviour under it is a
   feeling, and a feeling will not survive the offer conversation it exists to inform.
7. `## Evidence the room confirmed` — what this round established about the scope of the seat, the bar
   being set, or the operator's standing against it, in the room's own words. Quote it and cite the
   transcript. Scope the operator was shown that the posting never described belongs here above
   everything else, because scope that grew between posting and room is the argument the comp case is
   later built on.

   This is a record, not an assessment. The agent does not grade the operator here and cannot be a
   third party about work it helped produce. What makes the section hold under pressure is that every
   line is attributable: a recruiter defending a band can be answered with what their own colleague
   said, and cannot be answered with encouragement.
8. `## Promote` — **the section this file exists for.** Every durable fact learned, routed to its owner:

   | Learned | Goes to |
   |---|---|
   | A fact about the company, practice, or role | `../company-brief.md` |
   | A fact about a person in the room | `../people/{their-file}.md` |
   | A requirement or coverage change | `../jd-map.md` |
   | A pattern that repeats across rounds | `interview-playbook.md`, via [`career-harvest`](../../../../process/career-harvest.md) |
   | Confirmed scope or standing that prices the seat | `../comp-case.md`, per [`comp-case`](../comp-case/template.md) |

   Write the promotion, then tick it. An unticked row means the living dossiers are stale and the next
   round will be prepped against an old picture.

9. `## For the next round` — what changes about the plan, who is likely next, and what to ask that this
   round opened up. Where the application carries a `demo` material, say whether this round surfaced
   anything worth a new `demo/demo-v{N}.md` and a commit to the repo behind it, per
   [`technical-build`](../../../../process/technical-build.md).

## Hard Constraints

- **Write it the same day.** A debrief written from memory a week later is the failure mode that produced
  the playbook in the first place. Rough and now beats precise and never.
- **`completeness:` is mandatory and honest.** When the operator has not yet supplied what happened, say
  so in that field and write `## What happened` as numbered open questions rather than inference. A file
  that quietly hardens with the questions unanswered is worse than an empty one.
- **Never infer content that only the operator can supply.** Everything derived from the record is
  labelled as such; everything from the room comes from the transcript or the operator.
- **The Promote section is not optional and not a summary.** It is a worklist that gets executed.
- **Freeform, never versioned.** One `debrief.md` per round folder, edited in place as the picture fills in.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready>
last_updated: <ISO date>
round: <N>
round_type: <recruiter-screen | hiring-manager | panel | technical | partner | client>
held_at: <ISO datetime>
duration_min: <actual, against the scheduled slot>
interviewer: "<Name, Title>"
source: <sources/ path to the transcript, or null with the reason>
outcome: "<what was said about next steps, verbatim where possible>"
completeness: <full | partial - what is still missing and who can supply it>
promoted: <false | true>
---
```

## Readiness Criteria

Ready when `completeness: full`, every `## Promote` row has been written into its destination, and
`promoted: true`. Until then it stays `drafting` and the next round's prep reads it knowing it is partial.
