# Careers production — the application sprint runbook

Operator-mode routing for `domain: careers` work. The board (`workspace/pipeline/pipeline.md`) owns stage state; buttons own transitions; templates own artifact shape. This file owns the order of operations.

## The sprint

| Step | Do | Owner |
|---|---|---|
| 0 Arrive | **Inbound is the default entry point** (see below). Outbound: the `job-scout` skill (operator-invoked, never scheduled) sweeps the watchlist → triage report in `pipeline/scout/` | operator / `system/skills/job-scout/` |
| 1 Save | `af pipe save` — board row at stage `saved`; a fetched JD parks in `scout/jd-cache/{slug}.jd.md`. On an inbound contact, **sweep the whole board first** (see below) | spine |
| 2 Start | `af pipe start <slug>` — scaffolds `applications/{slug}/` with `sources/`, `correspondence/`, `people/`; moves the cached JD in as `jd.md`; stage → `preparing` | spine |
| 3 Research | `company-brief.md` per its template; depth matches stakes | template |
| 4 Map | `jd-map.md`: honeypot scan → 3-tier requirements → experience map → **gap stop** → operator coverage choice | template |
| 5 Reply | On an inbound contact, `correspondence/reply-to-{name}-{date}.md` per its template — declared inputs, five moves, call-questions block | template |
| 6 Tailor | Draft materials declared in `materials:` (`resume/resume-v1.md`, `cover-letter/` only if required); offer to scaffold a proof-of-concept repo (`demo/`) via [`technical-build`](../../process/technical-build.md); humanizer pass on all user-voiced prose | templates + process |
| 7 Verify | fill jd-map `## Verification`; fix findings before export or readiness | template |
| 8 Export | `doc-export` skill → format by ATS (table below), file under the deliverable's `media/`, record in `exports[]` | `system/skills/doc-export/` |
| 9 Ready | `af ready <slug> resume` (and `cover-letter`) — refuses without verification + filed exports | spine |
| 10 Submit | **The human submits** in a normal browser, on the company career site (never a bot; never Easy Apply when direct apply exists) → `af pipe stage <slug> applied`. **Agency or referral submits instead:** go straight to `af pipe stage <slug> interviewing` and record `submitted_by:` in `application.md` — there is no self-submission to stamp and no `shipped` material | operator |
| 11 Interview | Per round, run the arc in [`interview-arc.md`](interview-arc.md) | process + templates |
| 12 Offer | `comp-case.md` at the application root per its template — assembled from each round's confirmed-evidence section, worked before the first comp call, and the ask itself goes out through `correspondence/` | template |
| 13 Track | `af doctor` surfaces nudges (7-day silence) and stale rows | spine |
| 14 Close | Terminal stage → `af pipe archive <slug>` moves the folder to `applications/completed/`. Run [`career-harvest`](../../process/career-harvest.md) first, while the detail is fresh | spine + process |

**Material rule:** Steps 6-9 apply to every `materials:` row; this extends the resume/cover-letter examples in the sprint table. Use the route below, file finals only for pack-declared exportables, and mark each submission material ready before the human submits. `correspondence`, `round-sheet`, `interviewer-brief`, `round-debrief` and `comp-case` are internal and never materials: no export gate, no readiness stamp, no `materials:` row.

## Inbound contact (the default entry point)

`search-profile.md` declares inbound-first as the strategy and the board bears it out. A recruiter DM or referral, not a posting, is how most rows start. Two things follow.

**The named req is a hypothesis, not the target.** Sweep the whole board before replying (below) — a better-fitting req at the same company is common and is usually the reason the reply is worth sending.

**The funnel skips `applied`.** An agency-submitted candidate never self-applies. `preparing → interviewing` is legal for exactly this; do not fake an `applied` event to reach it.

## Whole-board capture

On an inbound contact, hit the target's ATS posting API once for every open req. It returns full JD text, locations, publish dates, and comp where published — none of which a single posting URL gives you. The board row's `ats` field says which endpoint to use.

```
Ashby:      https://api.ashbyhq.com/posting-api/job-board/{org}?includeCompensation=true
Greenhouse: https://boards-api.greenhouse.io/v1/boards/{org}/jobs?content=true
Lever:      https://api.lever.co/v0/postings/{org}?mode=json
```

Read for: a better-fitting req than the one named; contradictions between a posting's structured fields and its body text (a `"Remote": true` req whose body demands relocation); and publish dates, since a req open eight months is either a high bar or evergreen — a real call question. On Windows, pipe these through Python with explicit `encoding='utf-8'` and `sys.stdout.reconfigure(encoding='utf-8')`; default cp1252 raises `UnicodeDecodeError` on curly quotes.

This is scoped to one company already in play, triggered by an inbound contact. It is not a watchlist sweep and does not revive `job-scout`.

## Where files live

Three tiers. The rule that keeps them honest: **a round folder cites the living dossiers; it never restates them.**

| Tier | Where | Rule |
|---|---|---|
| Living dossiers | `company-brief.md`, `jd-map.md`, `role-thesis.md`, `people/`, `application.md`, `comp-case.md` once an offer is live | Edited in place, always current, never versioned. A declared set — root is not a spawn zone |
| Raw | `sources/` (+ `INDEX.md`), `research/{date}-{topic}/` | Immutable, registered, mined once. **Never prep from here** |
| Round snapshots | `round-{N}-{name}/` | One conversation's judgment. Carries only the delta; its `debrief.md` promotes durable facts back to tier 1 |

Freeform careers files are **not versioned**. No `-v{N}` filename on a brief, sheet, dossier or debrief — that shape belongs to tracked deliverables (resume, cover-letter, deck) and hands the file to the version guard. Everything else is edited in place.

## Material routes and export formats

| Material | Drafting route | Export / filing |
|---|---|---|
| resume, cover-letter | pack templates + `doc-export` skill | PDF/DOCX per ATS table below, filed in `media/` + `exports[]` |
| deck | [`deck-production`](../../process/deck-production.md) (PPT Master default); content doc versioned as `deck/deck-v{N}.md` | `.pptx` filed in `deck/media/` + `exports[]` (pack-declared exportable) |
| demo | [`technical-build`](../../process/technical-build.md) conventions, thinned: repo lives outside AgentFrame; `demo/demo-v{N}.md` is the umbilical stub (repo path, scope, what it proves, run steps) | no exports gate; the repo link in the stub is the deliverable |

| Board `ats` | Submit | Why |
|---|---|---|
| workday, taleo, icims, successfactors, unknown | **DOCX** | XML structure parses natively; Workday's PDF extractor scrambles bullets and drops lines |
| greenhouse, lever, ashby, email/direct | **PDF** (text-layer, browser-printed) | 96%+ parse fidelity; the recruiter sees the native file |
| unsure | build both, submit DOCX | asymmetric downside |

## Screen traits worth evidencing

When the watchlist names the screen: Workday Illuminate / Phenom — outcomes in context beat keyword density, and applying via the company career site feeds engagement ranking. Eightfold-style screens — evidence learnability and adjacent-skill progression. Voice-agent screens (Ezra, Talent Llama) — expect a structured verbal screen at `interviewing`; prep from the jd-map, not the resume.

## Career sessions (internal, ongoing)

Career work between cases is a session, not a project and never always-on. On career-management intent (coach prep, KPI inventory, promotion planning), load the career bank (`library/context/operator/career/`), employer page (`career/employers/{slug}.md` - rubric, KPIs, cycle calendar), and relevant coach/manager people pages (`library/context/people/`). On interview intent specifically, `career/interview-playbook.md` is the first read, before any prep drafting. Work the session; before closing, harvest durable output to its home: win -> career-harvest; stated expectation or rubric change -> employer-page timeline; relationship fact -> the person's page. Nothing persists in chat and no session folder exists.

## Internal cases (promotion, level moves)

Run them as pipe rows (see pack.md): rubric in as `jd.md`, jd-map as rubric-map with the gap stop months ahead of the deadline ("no evidence for criterion X" -> plan the work that produces it, then bank it), materials usually `[deck]`, committee dates on the board's `deadline`/`next_nudge`. Promotion-deck and rubric-map templates remain unbuilt until the first real cycle supplies a real leveling rubric.

## Bank maintenance

A win worth telling in an interview is a win worth banking: run [`career-harvest`](../../process/career-harvest.md) at project closeouts and ad hoc. The bank feeds every future sprint.
