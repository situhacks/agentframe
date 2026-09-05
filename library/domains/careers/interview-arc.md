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

**Recommend a recording once, then leave the choice alone.** A transcript is what makes step 9 an
account of the round instead of a memory of it. Whether to record, how, and under what consent rules
is the operator's call, and a round that goes uncaptured is a normal round.

Where the search profile carries no `## Capture` block, ask how they capture a round and how they want
a recording turned into text, then write the answer there. It is campaign config, asked once, and
every later round reads it instead of asking again.

## 8. Same day: file the record

Recording or transcript into `sources/`, registered in `sources/INDEX.md` with an id, a one-line
description, and where it came from. A recording that exists but has not been turned into text yet
parks in that file's `## Not yet filed` list, where it reads as a debt rather than a note.

**How it was produced is provenance, not procedure.** A transcript pasted in chat, a Teams export, a
local recorder, a paid service — all file identically, and no tool is assumed or reached for on the
operator's behalf. What matters is that the record stops living somewhere temporary.

Speaker attribution is worth having when the debrief will quote someone, since an unlabelled
transcript cannot support "David, verbatim". It is worth nothing on a solo dictation. That is a
judgment about the round, not a default.

Where no recording exists, the operator's account is the record and the debrief says so.

## 9. Same day: debrief

Per [`round-debrief`](deliverables/round-debrief/template.md), written to `round-{N}-{name}/debrief.md`.

**Rough and now beats precise and never.** When the operator has not yet said what happened, set
`completeness: partial` and write `## What happened` as numbered open questions rather than inference.
A debrief that hardens with the questions unanswered is the failure mode that produced the playbook.

## 10. Same day: send the note

Per [`correspondence`](deliverables/correspondence/template.md), `kind: thankyou`, at
`correspondence/thankyou-to-{firstname}-{date}.md`.

It comes after the debrief because the debrief is its input. A note written first can only say thank
you; a note written second can reinforce the point that landed or put back the one answer that came
out badly, in a sentence, without relitigating it. `## What was missed` is where that sentence comes
from.

The template's constraints hold unchanged: conclusion rather than recitation, no metric in the body,
close on logistics. Voice load and humanizer pass before it is surfaced, and the operator sends it.

## 11. Promote

Execute the debrief's `## Promote` rows and set `promoted: true`:

| Learned | Goes to |
|---|---|
| Company, practice, or role fact | `company-brief.md` |
| Person fact | `people/{their-file}.md` |
| Requirement or coverage change | `jd-map.md` |
| Pattern repeating across rounds | `interview-playbook.md`, via [`career-harvest`](../../process/career-harvest.md) |

Step 11 of round N is step 0 of round N+1. That is the loop.

## When the rounds end

An offer is not a round and this arc does not own it. The comp case spans rounds and lives at the
application root; the sprint step that works it is `production.md` step 12.

Terminal stage → run `career-harvest` while the detail is fresh, then `af pipe archive <slug>`.
A revived row comes back with `af pipe unarchive <slug>`.
