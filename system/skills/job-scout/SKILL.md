---
name: job-scout
description: Operator-invoked job discovery — sweep the search-profile watchlist (or pull one named posting) via public ATS feeds, filter ghosts, and write a recency-first triage report feeding the pipeline board.
---

# job-scout

Find real postings and feed the funnel. **Operator-invoked only — never scheduled, never auto-applying.** What counts as a good job lives in `library/context/operator/career/search-profile.md` (targets, dealbreakers, watchlist); this skill owns only the mechanics of finding and triaging.

## Two modes

- **Single** — the operator names a company + role: pull that posting (tiers below), then `af pipe save` the row and cache the JD.
- **Sweep** — "anything worth applying to?": walk the watchlist, collect postings, filter, write a triage report the operator picks from.

## The tiered pull — cheapest, cleanest, most legal first

Work down; stop at the first tier that returns the posting. Tier 1 needs no key, no login, no scraping.

### Tier 1 — public ATS feeds (TRY FIRST)

Identify the ATS from the careers-page URL host, then hit its public feed:

| ATS | URL tell | Public feed |
|---|---|---|
| Greenhouse | `boards.greenhouse.io/{token}` | `GET https://boards-api.greenhouse.io/v1/boards/{token}/jobs?content=true` |
| Lever | `jobs.lever.co/{site}` | `GET https://api.lever.co/v0/postings/{site}?mode=json` |
| Ashby | `jobs.ashbyhq.com/{board}` | `GET https://api.ashbyhq.com/posting-api/job-board/{board}?includeCompensation=true` |
| Workable | `apply.workable.com/{slug}` | `GET https://www.workable.com/api/accounts/{slug}?details=true` |
| Workday | `{tenant}.wd{N}.myworkdayjobs.com/{site}` | `POST https://{tenant}.wd{N}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs` body `{"limit":20,"offset":0,"searchText":"{title}","appliedFacets":{}}` |

Search the **per-company target title from the watchlist**, not a generic title — the wrong tier's title wastes the sweep.

### Tier 2 — WebSearch + logged-off fetch

Company career pages and postings not behind auth. Extract the JD text from the fetched page.

### Tier 3 — aggregator API (only if the operator has set a key)

Broad cross-board discovery via a configured aggregator key (see the watchlist's notes). No key: skip — never ask the operator to buy anything mid-sweep.

### Tier 4 — paste (always works)

Gated/proprietary portals, LinkedIn-only postings: leave a `paste` row in the report — the operator drops the JD in. **Never log in, never bypass anti-bot, never automate LinkedIn/Indeed** (account-ban territory; the report says which rows need a paste).

## Ghost and noise filters (before the report)

1. **Age**: drop postings older than 30 days (legitimate roles fill in ~41; older is evergreen pipeline bait) unless the watchlist row says otherwise.
2. **Mirror test**: a posting found on an aggregator must exist in the company's own ATS feed; missing there = ghost, drop it.
3. **Reality score**: JD names concrete deliverables, team context, and scope → real; generic responsibilities and no team signal → flag `low-reality`.
4. **Dedup**: skip anything already on the board (match by URL or slug).
5. **Dealbreakers** from search-profile apply silently — dropped rows get one summary line, not entries.

## The triage report

Write `workspace/pipeline/scout/scout-{YYYY-MM-DD}.md`, **newest posting first** — early applications convert best, so freshness outranks fit in presentation order:

```markdown
# Scout — {date} ({n} candidates from {m} sources; {k} dropped: {reasons})

| # | Posted | Company | Role | Fit signal | Flags | Link |
|---|--------|---------|------|-----------|-------|------|
| 1 | 2026-07-09 (1d) APPLY-NOW | ... | ... | matches track X; comp in band | — | url |
```

Flag anything under 72h old as **APPLY-NOW**. Fit signal is one honest clause against the search-profile, not a score.

## After the operator picks

For each pick: `af pipe save --company ... --role ... --url ... --ats ... --source scout --posted ...` then **immediately cache the JD verbatim** at `workspace/pipeline/scout/jd-cache/{slug}.jd.md` (postings vanish; `af pipe start` moves it in as `jd.md`). Offer `af pipe start` for the one they want to work first — the runbook (`library/domains/careers/production.md`) takes it from there.

## Honest limits

- Only pulls JDs the operator asked for; never scrapes or scores *candidate* data.
- JDs are saved verbatim, never paraphrased.
- If a tier returns nothing, say which and fall through — never fabricate a posting to fill a row.
- Old reports are disposable; prune `scout/` freely (the board is the durable record).
