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
| 5 Tailor | `resume/resume-v1.md` (plus `cover-letter/` only when required or asked) from the map; humanizer pass | templates |
| 6 Verify | fill jd-map `## Verification`; fix findings before export | template |
| 7 Export | `doc-export` skill → format by ATS (table below), file under the deliverable's `media/`, record in `exports[]` | `system/skills/doc-export/` |
| 8 Lock | `af lock <slug> resume` (and `cover-letter`) — refuses without verification + filed exports | spine |
| 9 Submit | **The human submits** in a normal browser, on the company career site (never a bot; never Easy Apply when direct apply exists) → `af pipe stage <slug> applied` stamps the date, sets the nudge, records `shipped` | operator |
| 10 Track | `af doctor` surfaces nudges (7-day silence) and stale rows; interview notes accrete in `application.md` body | spine |

## Export format by ATS

| Board `ats` | Submit | Why |
|---|---|---|
| workday, taleo, icims, successfactors, unknown | **DOCX** | XML structure parses natively; Workday's PDF extractor scrambles bullets and drops lines |
| greenhouse, lever, ashby, email/direct | **PDF** (text-layer, browser-printed) | 96%+ parse fidelity; the recruiter sees the native file |
| unsure | build both, submit DOCX | asymmetric downside |

## Screen traits worth evidencing

When the watchlist names the screen: Workday Illuminate / Phenom — outcomes in context beat keyword density, and applying via the company career site feeds engagement ranking. Eightfold-style screens — evidence learnability and adjacent-skill progression. Voice-agent screens (Ezra, Talent Llama) — expect a structured verbal screen at `interviewing`; prep from the jd-map, not the resume.

## Bank maintenance

A win worth telling in an interview is a win worth banking: run [`career-harvest`](../../process/career-harvest.md) at project closeouts and ad hoc. The bank feeds every future sprint.
