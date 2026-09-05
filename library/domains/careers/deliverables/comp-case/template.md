# Comp Case

## Purpose

The case for a specific number, the script that says it, and the answers to what comes back. It exists
because the number gets settled on a call under time pressure, and the reasoning has to be written down
before that call starts rather than assembled during it.

Lives at `comp-case.md` at the application root. It spans rounds, so it is not a round artifact and
never moves into a round folder.

## Inputs

- `application.md` — `salary_range` as posted, and any anchor already given.
- Every round's `debrief.md`, specifically `## Evidence the room confirmed`. That section is where the
  reasons come from; a comp case assembled from memory instead is the failure this file exists to stop.
- `jd-map.md` — scope confirmed against scope as posted. Scope that grew between the posting and the
  rounds is usually the strongest argument available.
- `company-brief.md` — published bands on adjacent reqs, funding, size, anything that prices the seat.
- `library/context/operator/career/profile.md` — comp targets and constraints, which set the floor.

## Output Shape

1. `## Where things stand` — a fact table, not prose. The anchor already given and when, the posted
   band, the competing situation if one exists, who the channel is, and the date that binds. An anchor
   given in an early screen before the scope was understood is the most common thing this file has to
   work around, so it is stated first rather than buried.

2. `## The reasons, ranked by what survives challenge` — ranked, and the ranking is the work. A reason
   that collapses the moment a recruiter pushes on it belongs below one that does not. Each reason
   carries its evidence and the strength of that evidence in the same line, so a number from an
   aggregator listing or a second-hand remark is used as *"my understanding is"* rather than quietly
   promoted to a hard citation.

3. `## The script` — what actually gets said, written out before the call. The number appears once, in
   a sentence, tied to the scope of the mandate rather than to personal need. Then it stops: the
   silence after a number belongs to the other side.

4. `## Objection handling` — a two-column table, if-they-say against you-say. It is written in advance
   because none of these compose well in the moment, and the one that matters most is the answer to an
   anchor the operator already gave.

5. `## What to be honest with yourself about` — the strength of the walk-away, stated flatly. A weak
   walk-away is not a reason to ask for less. It is a reason never to imply a walk that would not
   happen, because that bluff costs nothing to call. This section is what stops the rest of the file
   turning into self-persuasion.

6. `## Timeline` — dated. The deadline that actually binds, the days in the window that are not working
   days, and the one safety valve available if the timing slips.

## Hard Constraints

- **The published floor is a floor.** Never open below a posting's own published range, whatever the
  operator's history anchors them to.
- **Never volunteer a competing number that sits below the target's midpoint.** It anchors down, and it
  retires the scope argument that reaches the top of the band. Be direct about the operator's number
  and decline on theirs.
- **Never inflate a competing offer either.** A match request can ask to see it, and an overstated
  figure caps the ask at the overstatement.
- **Probe the whole package before trading anything.** Approved band, bonus target, sign-on, equity,
  relocation, review cadence. Naming a lever before knowing the menu gives away the trade for free.
- **Two turns, then close.** Turn one is the macro counter across the whole package. Turn two is one
  surgical trade paired with an explicit commitment to sign, which is what removes the other side's
  fear of endless haggling. A third and fourth pass buys nothing and reads as indecision.
- **The realistic worst case is that they say no.** A polite, evidence-backed ask is ordinary in
  professional hiring, and "understood, let's proceed" remains available after a refusal. What does
  cause real damage is renegotiating after accepting, or an ultimatum that cannot be met.
- **Never accept verbally on the call.** The numbers get checked as net cash before any commitment.
- **Not a submission material.** Never joins `materials:`, no export gate, no `af ready`. Same
  carve-out `round-sheet`, `interviewer-brief` and `correspondence` have.
- **Freeform, never versioned.** One `comp-case.md`, edited in place as the picture changes. No
  `-v{N}` filename, which would hand it to the version guard and the tracker.

## Draft Frontmatter Convention

```yaml
---
type: living_state
status: <drafting | live | closed>
last_updated: <ISO date>
target: "<the number being asked for>"
anchor_given: "<what was said, to whom, when - or none>"
posted_band: "<as published, or 'unpublished'>"
decision_by: <ISO date the answer is due, or null>
---
```

## Readiness Criteria

Not ready-gated. It is internal and the operator runs the conversation. Before the first comp call it
needs: the anchor situation stated, reasons ranked with evidence tiers, the script written, objections
answered, and the walk-away assessed honestly.
