# Deliverable Template Authoring

Use this when creating or materially reshaping `library/deliverables/{type}/template.md`.

## Purpose

Deliverable templates define reusable artifact shape: what the agent must produce, what inputs it needs, and what constraints make the output useful. They do not own campaign phase sequencing or multi-step workflow mechanics.

## Required Sections

Use this order unless a specific deliverable earns a different shape:

1. `Purpose` — what decision, output, or handoff this deliverable exists to create.
2. `Inputs` or `Depends On` — what must be loaded or locked before drafting. Include context-loading choices here only when the template must load, partially load, or deliberately skip a file/process to draft or lock correctly. Name the source; do not restate its content.
3. `Output Shape` or `Sections` — the artifact the agent writes. Include author, reader, stance, or register only when that changes the shape of the artifact.
4. `Hard Constraints` — non-negotiable rules earned by observed failure or explicit product need.
5. `Draft Frontmatter Convention` — only fields this deliverable file carries.
6. `Lock Criteria` — what must be true before `status: locked`.

## Optional Sections

Optional sections are not headings to copy into every template. Add one only when the section itself changes what a future agent loads, writes, verifies, routes, exports, or asks the operator to decide.

- `Review Path` — reviewer state, approval sequence, or downstream unlock rules that differ from ordinary operator approval. Do not restate the selected campaign flow's normal review cadence.
- `Humanizer Pass` — only for public-facing deliverables where prose is likely to ship (typically Phase 4 production artifacts). Name the trigger surface and scope. If the deliverable is internal, omit the section.
- `Publish / Export Mechanics` — non-trivial format generation, shipped-state reconciliation, or publish records owned by this deliverable. Keep simple export formats in `Review Path`; keep reusable mechanics in process or export files.
- `Exceptions / Branches` — state-changing branches that alter the normal path, such as stop, route upstream, waive, cancel, or defer. Not examples. If the item is always true, make it a hard constraint. If it is source loading, put it in `Inputs`. If it is required before lock, put it in `Lock Criteria`.

Do not create placeholder sections to say a thing does not apply.

## Ownership Rules

- Templates own artifact shape and hard constraints.
- Campaign flows own phase sequence and which deliverables appear in a flow.
- Process files own reusable procedures.
- Skills own change mechanics or reusable technical techniques.
- `AGENTS*.md` owns routing and cross-cutting invariants only.

This file is loaded when authoring or materially reshaping templates, not during ordinary campaign drafting. It tells template authors not to add audit-history, root-cause, or changelog sections to runtime templates. The actual patch-history write is owned by the structural-change or system-improvement process through `system_changes`.

## Grounding Discipline

Templates that produce factual content (audience claims, market context, language tells, competitive references, statistical claims, opportunity framing) must name the source layer the agent should ground in. Three layers, in order of strength:

1. **Research artifact** — the canonical source for campaign-specific facts. Cite `phase-1-research/research-artifact-v{N}.md` when the claim came from there.
2. **Light web search** — for time-sensitive checks (competitor name spelling, public stat verification). Earned only when the research artifact lacks the data and a quick check is cheaper than a deep-research rerun.
3. **LLM-prior** — pretrained-model knowledge. Earns its slot only for structural reasoning (for example, a typical campaign arc with hook/build/payoff), never for factual claims about audience, market, or competitors.

If a template section invites factual content without grounding, either name the source layer in the section description or move that section to research-artifact where ungrounded claims can be separated from hypotheses. Sections producing structural reasoning do not need a grounding tag.

## Frontmatter Pattern

Default deliverable frontmatter:

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
current_version: <integer>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason>"}
---
```

Add fields only when the deliverable itself owns them. Publishing fields belong on production deliverables. Export fields belong on exportable stakeholder deliverables.

## Checks Before Adding A New Template

1. Compare against existing deliverable families.
2. If an existing template covers 70%+ of the job, extend it or add a subtype convention instead of creating a new type.
3. If the new template belongs in a campaign flow, wire the flow separately through `library/process/flows/`.
4. Log the scaffold or material reshape in `system_changes`.
