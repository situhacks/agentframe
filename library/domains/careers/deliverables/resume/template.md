# Resume (tailored)

## Purpose

One submission-ready resume per application: the operator's real experience, selected and rephrased against this JD, formatted so every major ATS parses it cleanly and a human scanning for seven seconds lands on quantified proof immediately.

## Inputs

- `jd-map.md` — requirements, experience map, coverage target. Draft only after its gap stop has cleared.
- Career bank: `master-cv.md` (the only source of bullets), `profile.md` (contact header, exact strings), `proof-points.md` (every number cited), the active track file under `tracks/`.
- Voice (`library/context/operator/voice/`) for any prose line.
- The board row's `ats` → export format and screen traits (table in the pack's `production.md`).

## Output Shape

Section order is the operator's real format: contact header → **Work Experience** → **Projects** (when the JD values them) → **Education** (short) → **Skills / Extras**. **No Professional Summary, profile paragraph, or positioning line.** A JD title that doesn't match the most recent role title is not a trigger; mirror the title inside the bullets instead. Add one only when the operator asks for it in the session, and never as an agent judgment call. `profile.md`'s standing style rules are binding here.

- Contact header: name, phone, email, city + province/state, then full visible URLs (`linkedin.com/in/...`, `github.com/...`) — in the body, never in a document header/footer.
- Bullets are selection, not generation: pick the master-cv bullets the jd-map names, rephrase to mirror hard-requirement phrasing honestly. CDO shape — context → the decision/trade-off → quantified outcome. One dense chained bullet beats two thin ones.
- Skills section lists every mirrored hard skill; each also appears inside a quantified bullet (dual placement). Acronym plus full term on first use.

## Hard Constraints

- Single column. No tables, text boxes, icons, images, or skill bars.
- Canonical headings only: "Work Experience", "Projects", "Education", "Skills".
- Dates `Month YYYY - Month YYYY` (plain hyphen) — explicit months; year-only ranges zero out tenure calculators. **No em- or en-dashes anywhere in the artifact**; round bullets only.
- Never invent experience, metrics, or tools. Every number traces to `proof-points.md`.
- Length: 1 page US default; up to 2 pages for senior Canadian-enterprise roles with real tenure — never padded.
- AI-tell blacklist is a hard gate (spearheaded, delve, orchestrated, synergies, "proven track record", uniform verb openings across roles).
- GPA only if above 3.5/4.0.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | ready | deferred>
last_updated: <ISO date>
exports: []   # filed export paths under resume/media/—required before readiness
---
```

## Humanizer Pass

Mandatory before verification — resume prose is public-facing and recruiters auto-reject on AI tells. Scope: every rewritten bullet and any positioning line.

## Readiness Criteria

- jd-map `## Verification` complete and current for this head version (pack-rules enforced).
- Exports filed under `resume/media/` and recorded in `exports[]`, format per target ATS: **DOCX** for Workday/Taleo/iCIMS/SuccessFactors/unknown-legacy; **PDF** (text-layer, browser-printed) for Greenhouse/Lever/Ashby/direct email; both when unsure.
- Plain-text paste test on the export passes: reading order intact, headings present, no garbled glyphs.
