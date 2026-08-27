# Interviewer Brief

## Purpose

Know who is in the room before walking into it: verified background, what they own, what they are likely
to test, and what changes about the plan because it is them rather than someone else.

Two files, one job, split by lifetime:

- **`people/{firstname-lastname}.md`** — the living dossier. Verified facts about this person, edited in
  place, accumulating across every round they appear in. Survives the round.
- **`round-{N}-{name}/interviewer-brief.md`** — this room's read. What changes about *this* conversation
  because it is them. Cites the dossier, never restates it.

Deep by default, every round. A recruiter screen is the only round type that earns a thin brief, because
a coordinator's background rarely changes the plan. From the hiring manager on, research properly.

## Inputs

- The name and title, from the scheduling mail or the recruiter. Treat a recruiter's offhand
  characterisation as a claim to verify, not a fact.
- `company-brief.md` — where this person sits in the org.
- `jd-map.md` — what they are likely to probe.
- Search order: web search first for the public record (company bio, press, conference talks, podcasts,
  posts). Route to the `agent-reach` skill for LinkedIn, which the default toolchain cannot fetch.

## Output Shape

**Dossier (`people/{name}.md`)** — facts only, each with its source:

1. `## Verified` — role, tenure, scope, prior employers with real dates, education. One line per fact,
   each carrying where it came from.
2. `## Unverified` — claims heard but not confirmed, marked as such and attributed to who said it.
3. `## In their own words` — anything they have said publicly about the work, the market, or how they
   hire. The highest-value section and the most often skipped.
4. `## History with us` — every round they have been in, what they asked, what landed.

**Round brief (`interviewer-brief.md`)** — judgment for this conversation:

1. `## Who this is` — three lines, and the one fact that matters most here.
2. `## What changes because it is them` — the register shift, which stories move up, which material
   drops to background. This is the section the folder exists for.
3. `## What they will probe` — their remit crossed with the gap stop.
4. `## Ask them` — questions only this person can answer. Half a title nobody outside the firm can
   define is a better question than anything on a generic list.

## Hard Constraints

- **Every fact carries its source, or it is marked unverified.** A recruiter saying "eight or nine years"
  is not a tenure. Wrong specifics are more expensive than absent ones, because they get said out loud.
- **Tenure and employer claims get checked against the public record before they inform framing.**
- **Never restate the dossier in the round brief.** Cite it. Duplication is how two versions of a person
  end up in one application folder disagreeing with each other.
- **Freeform, never versioned.** No `-v{N}` filenames on either file; both are edited in place.
- A person who becomes a durable relationship beyond this application is promoted to
  `library/context/people/` by [`career-harvest`](../../../../process/career-harvest.md), not copied there by hand.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready>
last_updated: <ISO date>
person: "<Name>"
title: "<Title at time of writing>"
rounds: [<round numbers they appear in>]
confidence: <high | medium | low>
---
```

## Readiness Criteria

Not ready-gated. It is usable when every framing-relevant claim is either sourced or explicitly marked
unverified, and `## What changes because it is them` says something that actually changes the round-sheet.
If it changes nothing, either the research is too shallow or this is a recruiter screen.
