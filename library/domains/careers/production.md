# Careers production — the application sprint runbook

Operator-mode routing for `domain: careers` work. The board (`workspace/pipeline/pipeline.md`) owns stage state; buttons own transitions; templates own artifact shape. This file owns the order of operations.

## The sprint

| Step | Do | Owner |
|---|---|---|
| 0 Find | `job-scout` skill (operator-invoked, never scheduled) sweeps the watchlist → triage report in `pipeline/scout/`, newest posting first, <72h flagged apply-now | `system/skills/job-scout/` |
| 1 Save | `af pipe save` — board row at stage `saved`; a fetched JD parks in `scout/jd-cache/{slug}.jd.md` | spine |
| 2 Start | `af pipe start <slug>` — scaffolds `applications/{slug}/`, moves the cached JD in as `jd.md`, stage → `preparing` | spine |
| 3 Research | `company-brief.md` per its template; depth matches stakes | template |
| 4 Map | `jd-map.md`: honeypot scan → 3-tier requirements → experience map → **gap stop** → operator coverage choice | template |
| 5 Tailor | Draft the materials the application declares: `resume/resume-v1.md` (+ `cover-letter/` only if required) via their templates; `deck/` or `demo/` per the material routes below; humanizer pass on all user-voiced prose | templates |
| 6 Verify | fill jd-map `## Verification`; fix findings before export or readiness | template |
| 7 Export | `doc-export` skill → format by ATS (table below), file under the deliverable's `media/`, record in `exports[]` | `system/skills/doc-export/` |
| 8 Ready | `af ready <slug> resume` (and `cover-letter`)—refuses without verification + filed exports | spine |
| 9 Submit | **The human submits** in a normal browser, on the company career site (never a bot; never Easy Apply when direct apply exists) → `af pipe stage <slug> applied` stamps the date, sets the nudge, records `shipped` | operator |
| 10 Interview | On a round being scheduled: read `career/interview-playbook.md` first, then draft `interview-prep/interview-prep-v{N}.md` per its template, one sheet per round. After the round, harvest what repeats back into the playbook's append block | template + [`career-harvest`](../../process/career-harvest.md) |
| 11 Track | `af doctor` surfaces nudges (7-day silence) and stale rows; interview notes accrete in `application.md` body | spine |

**Material rule:** Steps 5-8 apply to every `materials:` row; this extends the resume/cover-letter examples in the sprint table. Use the route below, file finals only for pack-declared exportables, and mark each submission material ready before the human submits. Step 10's `interview-prep` is internal and never a material: no export gate, no readiness stamp, one sheet per round.

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
