# Round Sheet

## Purpose

The call sheet for one interview round: the openers this room will actually pull on, the questions to ask
back, and the two or three risks specific to these people. Internal, never submitted, and sized to be
re-read in the ten minutes before the call.

Lives at `round-{N}-{name}/round-sheet.md`, scaffolded by `af pipe round`.

## Inputs

Read these first; a sheet written without them repeats a failure the bank already recorded.

- `library/context/operator/career/interview-playbook.md` — the standing cross-round reminders. Binding
  here, and **not restated**. Pull only the drill-set rows this round earns.
- `proof-points.md` — every number cites a `pp-NNN`. An uncited number does not go in the room.
- `interviewer-brief.md`, in this folder — who is in the room and what changes because of it.
- The previous round's `debrief.md`, when there is one. Its `## For the next round` is the brief for this
  sheet, and its `## Promote` rows must already be written into the living dossiers before drafting here.
- `../jd-map.md`, `../company-brief.md` — the current requirements and the current company picture. Cite
  them; do not copy them in.

## Output Shape

1. `## This room` — round type, who is in it, what each owns, the register each needs. One line per person.
2. `## Openers` — the three to five drill-set rows this round earns, number first, with `pp-NNN` citations.
   Mark any that need rehearsing out loud.
3. `## Likely questions` — what this specific role and these specific people imply, each with the opener or
   proof-point that answers it. Not a generic question bank.
4. `## Risks in this room` — the two or three things most likely to go wrong *here*: a level or title
   mismatch, a gap this panel probes, a playbook caution these people will test.
5. `## Ask back` — the scoping question for the first three minutes, plus two or three real questions
   for the end.
6. `## Facts to have ready` — comp number, notice period, relocation answer, availability. Decided before
   the call, never improvised.

## Hard Constraints

- **One screen per section.** A section that scrolls will not be read in the room. Prep past a few hundred
  lines has become a research document and stopped being prep.
- **Cite the living dossiers, never restate them.** `company-brief.md`, `jd-map.md` and `people/` are
  current by construction — the previous debrief promoted into them. Copying their content in creates a
  second, immediately stale version inside this folder. That duplication is the specific failure this
  structure exists to prevent.
- Every number cites `pp-NNN`. Never introduce a figure that is not in the bank.
- Honour the playbook's standing cautions exactly: unverified proof-points stay out, and no claim to a
  level or title not yet earned.
- **Not a submission material.** Never joins `materials:`, no export gate, no filed finals.
- **Freeform, never versioned.** One `round-sheet.md` per round folder, edited in place. A second round is
  a new folder, not a new version — so no `-v{N}` filename.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready>
last_updated: <ISO date>
round: <N>
round_type: <recruiter-screen | hiring-manager | panel | technical | partner | client>
interview_at: <ISO datetime, when scheduled>
---
```

## Readiness Criteria

Not ready-gated; internal prep, so `af ready` is unnecessary. Usable when every section fits a screen,
every number carries a `pp-NNN`, and the prior debrief's promotions are done.

## After The Round

Two moves, in this order. Skipping the second is how the same lesson gets re-derived a month later.

1. File the recording or transcript in `../sources/` and register it in `sources/INDEX.md`.
2. Write `debrief.md` in this folder per its template, and execute its `## Promote` rows.
