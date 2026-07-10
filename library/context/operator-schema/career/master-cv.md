# Master CV (schema)

Copy to `library/context/operator/career/master-cv.md`. The presentation layer: every role, all resume-ready bullets. Tailoring **selects** from here — it never writes new claims at draft time (new claims enter via career-harvest).

Per role:

```markdown
## {Company} - {Title} | {Month YYYY - Month YYYY} | origin: {project-slug | external}
- {CDO bullet: context → decision/trade-off → quantified outcome} [pp-XXX]
- ...as many bullets as the role's real substance supports — a governed program may carry a dozen, a mini-project two
```

Every number carries its proof-point ID (`[pp-XXX]`). Superseded bullets get marked, not deleted.

A role may carry **lens-tagged alternate bullets** alongside its canonical ones (`[pm-coded]`, `[consultant-coded]`, `[ai-builder-coded]`, `[banking-coded]`, ...): same fact, framed per JD signal. Tailoring picks the variant whose lens matches the jd-map; default to canonical when no lens is signalled.

Beyond roles: `## Projects`, `## Education`, `## Skills / Extras` blocks, mirroring the section order in the resume template (`library/domains/careers/deliverables/resume/template.md`).
