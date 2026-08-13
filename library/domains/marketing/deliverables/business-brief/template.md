# Template: Business Brief

## Purpose

Frame the business case for THIS opportunity. Sets the bar for everything that follows: if the business goal isn't measurable, no downstream deliverable can claim success. If the audience isn't named, the copywriter can't write to them. Stakeholder-facing when a stakeholder exists—exports to Word + PPT when feedback needs a shareable file; otherwise it can move directly from drafting to ready.

## Inputs

- Ready Research Artifact at `phase-1-research/research-artifact-v{N}.md`
- `positioning.md` (for POV anchoring)
- **Note:** Do not load the `voice/pairs/` examples (would push tone too casual for stakeholders).

## Output Shape

**Author:** A marketer drafting this for review.
**Reader:** A manager or business stakeholder who will approve before the campaign brief begins.

A structured business document for skim, markup, and approval. Bullets and sub-bullets over paragraphs; declarative prose where it appears. Institutional or team voice for stated positions; first person sparingly, when a specific call needs an owner. Don't narrate your own document. Stakeholder-facing — slightly more formal than user-voice; tight prose, not bullet-list filler.

**Sections:**
- **Opportunity** — what's the moment, why now, why us. Anchored in `positioning.md`. Cites at least 2 sources from the campaign's Research Artifact.
- **Target Audience** — directional persona. Specific enough that a copywriter could write to one person. (Detailed audience extraction happens in `messaging-architecture`; this is the steer.)
- **Business Goal** — tied to a quarterly goal from `positioning.md` Current Quarter Goals. Measurable.
- **Constraints** — what can't change (timeline, budget, channel mix, banned framings).
- **Risks + Open Questions** — never empty. If you can't think of risks, you don't understand the opportunity yet.
- **Binary Success Criteria** — measurable yes/no outcomes the campaign retro will check against.

## Hard Constraints

- Audience is specific enough a copywriter could write to one person (no "senior marketers" — say which segment, what stage of awareness, what context)
- At least one binary measurable success criterion
- Opportunity grounded in 2+ Research Artifact sources
- Risks section not empty
- Specific language about THIS opportunity, not boilerplate
- Ties back to at least one content pillar AND/OR a Current Quarter Goal
- Opportunity framing consistent with `positioning.md` (or explicit departure noted)

## Draft Frontmatter Convention

The `phase-2-strategy/business-brief/draft-v{N}.md` file carries this YAML frontmatter:

```yaml
---
status: <drafting | ready | deferred>
last_updated: <ISO-8601 timestamp>
exports:
  - {path: <relative path to exported file>, generated_at: <ISO-8601 timestamp>}
---
```

State transitions: `drafting` → `ready` (or `drafting` → `deferred`). Stakeholder feedback, when used, happens while drafting and does not create tracker fields. See [`library/process/ready-event.md`](../../../../process/ready-event.md) for readiness mechanics.

## Readiness Criteria

- Internal draft approved by operator
- Frontmatter `status` set to `ready`
- Campaign tracker updated per selected `flow`
- When stakeholder feedback or handoff needs them, Word + PPT exports generated to `phase-2-strategy/business-brief/exports/`
- Any stakeholder feedback the operator chose to seek has been applied or consciously left for a later version.
- Final markdown saved with `last_updated` frontmatter set
- Unblocks Campaign Brief

## Feedback / Handoff

- **Path**: optional stakeholder feedback when it improves the decision. Otherwise move directly to ready.
- **Reviewer**: typically manager or business stakeholder (when external is the path).
- **Export format**: Word (`.docx`) + PowerPoint (`.pptx`).
- **Coordination**: when feedback is requested, the agent may offer to draft the email or meeting note after export. Use an Attention item only if a response date matters.

## Publish / Export Mechanics

When this deliverable becomes ready, export to Word + PowerPoint when those formats are needed:

- **Supported formats**: `.docx`, `.pptx`
- **Template source**: campaign-local templates are optional at `workspace/projects/{slug}/exports/templates/business-brief.{docx,pptx}`
- **Output path**: `workspace/projects/{slug}/phase-2-strategy/business-brief/exports/business-brief-v{N}.{ext}`

For `.docx`, read `system/skills/docx/AGENTS.md` then load its `SKILL.md`, render the Word export, then validate through `python system/tools/docx_validate.py <file.docx>`. Add `--original <source.docx>` only when editing an existing Word file; new documents validate without a baseline. Update `draft-v{N}.md` `exports:` entries and append export activity events.

For `.pptx`, load `library/process/deck-production.md` and follow the central deck route. This template does not name individual deck tools directly; future PowerPoint routing changes happen in the deck-production process file.

## Exceptions / Branches

- **Deferred-deliverable shape:** if the brief is intentionally skipped, `draft-v{N}.md` is a stub containing only frontmatter (`status: deferred`, `reason: "..."`, `back_fill_at: null`, `back_fill_owner: ...`). The reason lives here, not in `project.md`.
- **External review returns "kill it"**: follow the cancellation rule in [`library/process/project-frontmatter.md`](../../../../process/project-frontmatter.md).
