# Cover Letter

## Purpose

A short human-proof artifact carrying what the resume format cannot: one institutional fact an LLM wouldn't hallucinate, one mapped achievement, one ask. Generated **only when the posting requires a letter or the operator asks** — never by default.

## Inputs

- `jd-map.md` (the requirement the letter answers) and `company-brief.md` (the cited institutional hook).
- One story from the bank — roster-line selection, opening only the chosen arc.
- Voice (`library/context/operator/voice/`) — the letter is fully user-voiced.

## Output Shape

Default (US/AI-native and general): **100–180 words**, HCPA — **H**ook (specific, never "I am excited to apply"), **C**onnection (the institutional fact and why this operator), **P**roof (one quantified achievement mapped to the JD's core constraint, optionally linking an asset), **A**sk (brief, confident).

Segment escalations, only when the posting or segment mandates:
- **Canadian enterprise:** at most one page, direct, quantified, no boasting; Canadian spelling.
- **Consulting:** 250–350 words, four paragraphs — analytical-insight open, firm-specific why, proof paragraph (context-decision-outcome), confident close.

## Hard Constraints

- The institutional fact comes cited from `company-brief.md` — never from LLM prior.
- AI-tell blacklist is a hard gate: delve, testament to, excited to apply, unique blend, proven track record, beacon, catalyst, furthermore/moreover chains, "do not hesitate to contact me".
- No em-dashes; standard punctuation.
- Never restate the resume.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO date>
exports: []
---
```

## Humanizer Pass

Mandatory — recruiters treat an AI-scented letter as a stronger negative signal than no letter at all.

## Lock Criteria

- jd-map `## Verification` covers the letter (pack-rules enforced).
- Word count inside the chosen shape's band.
- Export filed under `cover-letter/media/` and recorded in `exports[]`, same format rule as the resume.
