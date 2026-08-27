# The interview arc

One round, start to finish. Load this when a round is scheduled. It repeats per round; the loop is what
makes round 3 cheaper than round 1 instead of more expensive.

Read [`interview-playbook.md`](../../context/operator/career/interview-playbook.md) first, every round,
before drafting anything. It is binding and no artifact here restates it.

## 0. The carry-forward gate

**Before anything else: open the previous round's `debrief.md` and check its `## Promote` rows are all
written into their destinations.**

If they are not, do that first. Everything downstream reads the living dossiers, and prepping round N
against dossiers that round N-1 never updated is exactly how an application accumulates two disagreeing
versions of the same fact. This gate is the whole reason the structure holds.

No previous round? The gate is `company-brief.md` and `jd-map.md` existing and current.

## 1. Find out who is in the room

A round with an unnamed interviewer is not ready to prep. Ask the recruiter directly — it is a normal
question and the answer changes everything downstream. Get name, title, and where they sit.

## 2. Scaffold

```
af pipe round <slug> <N> --with "Name, Title" --type hiring-manager --at 2026-09-02T10:00:00-07:00
```

Creates `round-{N}-{firstname}/` with a `README.md` folder map. Repeat `--with` for a panel. Round types:
`recruiter-screen`, `hiring-manager`, `panel`, `technical`, `partner`, `client`.

## 3. Research the room

Per [`interviewer-brief`](deliverables/interviewer-brief/template.md). Deep by default — a recruiter
screen is the only round that earns a thin brief.

Two files: the durable dossier at `people/{firstname-lastname}.md` (verified facts, accumulates across
rounds) and `interviewer-brief.md` in the round folder (what changes about *this* conversation because
it is them).

Web search first for the public record; route to `agent-reach` for LinkedIn. **Every framing-relevant
claim is sourced or explicitly marked unverified** — a recruiter's offhand characterisation of someone's
background is a claim to check, not a fact to build on.

## 4. Refresh the living dossiers

Only what this round makes stale. `company-brief.md` `## Now` older than ~30 days gets refreshed.
A second JD, a comp band, an org change discovered in step 3 goes into its dossier now — not into the
round folder.

## 5. Draft the round sheet

Per [`round-sheet`](deliverables/round-sheet/template.md). Openers, likely questions, risks in this room,
ask-back, facts to have ready. Every number cites `pp-NNN`. One screen per section.

**Cite the dossiers; never restate them.** They are current by construction, because step 0 made them so.

## 6. Finish the README

The folder map is what the operator actually opens. Three things earn their place:

- **Reading order with a real minute budget.** Prep that cannot be read in the gap before the call is not prep.
- **What changed since the last round.** Only the delta. A fact that contradicts something written earlier
  gets named as a contradiction, with which one is current.
- **The skip list.** Every file in this application that looks relevant and is not, with the reason.
  Superseded prep, week-one study aids, sent artifacts, raw transcripts already mined. This is the section
  that stops the operator re-reading stale material, and it is not optional once there is any history.

## 7. Run the round

Operator's. Nothing to do here except have step 6 finished in time to be read.

## 8. Same day: file the record

Recording or transcript into `sources/`, registered in `sources/INDEX.md` with an id and a one-line
description. MeetCap rotates `keep_last` transcripts out by design — an unfiled recording expires.

Where no recording exists, the operator's account is the record and the debrief says so.

## 9. Same day: debrief

Per [`round-debrief`](deliverables/round-debrief/template.md), written to `round-{N}-{name}/debrief.md`.

**Rough and now beats precise and never.** When the operator has not yet said what happened, set
`completeness: partial` and write `## What happened` as numbered open questions rather than inference.
A debrief that hardens with the questions unanswered is the failure mode that produced the playbook.

## 10. Promote

Execute the debrief's `## Promote` rows and set `promoted: true`:

| Learned | Goes to |
|---|---|
| Company, practice, or role fact | `company-brief.md` |
| Person fact | `people/{their-file}.md` |
| Requirement or coverage change | `jd-map.md` |
| Pattern repeating across rounds | `interview-playbook.md`, via [`career-harvest`](../../process/career-harvest.md) |

Step 10 of round N is step 0 of round N+1. That is the loop.

## When the process ends

Terminal stage → run `career-harvest` while the detail is fresh, then `af pipe archive <slug>`.
A revived row comes back with `af pipe unarchive <slug>`.
