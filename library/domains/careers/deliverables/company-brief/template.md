# Company Brief

## Purpose

Enough cited, current company knowledge to tailor honestly and interview credibly: what the company is doing *now*, why this role likely exists, and 2–3 hooks the cover letter or an interview opener can use.

## Inputs

- `jd.md` — what the posting itself reveals about the team and mandate.
- Grounding: light web search for ordinary targets. A target worth a full pass routes to the standard
  [`deep-research`](../../../../../system/skills/deep-research/SKILL.md) skill, which runs natively or
  drives the `gemini_deep_research` runtime (`system/research/`); its output files under
  `research/{date}-{topic}/` and is mined into this brief. There is no careers-specific research
  path. Every factual claim cited and dated.

## Output Shape

1. `## Facts` — size, segment, products/stack touching the role (cited).
2. `## Now` — dated recent moves: launches, funding, leadership, strategy shifts; prefer the last 12 months.
3. `## Why this role` — the JD's likely driver, marked `inferred` where it is inference.
4. `## Hooks` — 2–3 specific, cited facts usable as the letter's Connection or an interview opener.
5. `## Sources` — links with dates.

## Hard Constraints

- Date every claim; no undated "recently".
- LLM-prior is banned for company facts — web search or the research runtime only.
- Depth matches stakes: a `saved`-stage row gets nothing; a `preparing` application gets this brief; only genuinely serious targets earn a deep-research round-trip.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready | deferred>
last_updated: <ISO date>
---
```

## Readiness Criteria

Not ready — it feeds the letter and interview prep. Refresh `## Now` if the application reaches interviews more than ~30 days after drafting.
