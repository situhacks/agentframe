# Template: Business Brief

## Purpose

Frame the business case for THIS opportunity. Sets the bar for everything that follows: if the business goal isn't measurable, no downstream deliverable can claim success. If the audience isn't named, the copywriter can't write to them. Stakeholder-facing when a stakeholder exists — exports to Word + PPT for external review in that case; otherwise drafts to lock without review.

## Inputs

- Locked Research Artifact at `phase-1-research/research-artifact-vF.md`
- `positioning.md` (for POV anchoring)
- **Note:** Do not load `voice.md` Writing Style Examples (would push tone too casual for stakeholders).

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

The `phase-2-strategy/business-brief/draft-vF.md` file carries this YAML frontmatter:

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
current_version: <integer; incremented on every Write-tool replacement>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason for this version bump>"}
exports:
  - {path: <relative path to exported file>, generated_at: <ISO-8601 timestamp>}
---
```

State transitions: `drafting` → `locked` (or `drafting` → `deferred`). When external review is the path, the deliverable stays at `status: drafting` while `review` advances `pending → complete`; the operator locks once `review: complete`. See [`library/process/lock-event.md`](../../process/lock-event.md) for lock-event mechanics.

## Lock Criteria

- Internal draft approved by operator
- Frontmatter `status` set to `locked`
- Campaign tracker updated per selected `campaign_flow`
- Word + PPT exports generated to `phase-2-strategy/business-brief/exports/` (see [`system/skills/export-assets/README.md`](../../../system/skills/export-assets/README.md))
- **If external review is the path:** reviewer feedback applied so `review: complete`, OR explicitly `waived` with reason logged to `activity.md` + `phase_override` entry in the campaign's `activity.md`. **If review is `not_required`**: no review step — drafting → lock is the path; no override log needed.
- Final markdown saved with `last_updated` frontmatter set
- Unblocks Campaign Brief

## Review Path

- **Path**: external **when a stakeholder exists** (manager, team lead, leadership review the brief). Otherwise the brief drafts to lock with no review. See the selected `campaign_flow` in `campaign.md` for flow-specific review defaults.
- **Reviewer**: typically manager or business stakeholder (when external is the path).
- **Export format**: Word (`.docx`) + PowerPoint (`.pptx`).
- **Tracker signal**: See [`library/process/campaign-frontmatter.md`](../../process/campaign-frontmatter.md) for the `deliverables.business-brief.review` enum.
- **Coordination**: when external review IS the path, agent offers to draft the email + calendar invite on export (see `runtime/review-coordination.md`).

## Publish / Export Mechanics

When this deliverable locks, export to Word + PowerPoint:

- **Supported formats**: `.docx`, `.pptx`
- **Master templates**: `system/skills/export-assets/templates/business-brief.{docx,pptx}` (per-campaign override resolves first; see `system/skills/export-assets/templates/README.md`)
- **Brand config**: `system/skills/export-assets/config.yaml`
- **Output path**: `workspace/campaigns/{slug}/phase-2-strategy/business-brief/exports/business-brief-v{N}.{ext}`

For `.pptx`, run discovery before slide drafting:

1. Ask audience, time budget, presenter style, and decision-vs-handoff in one turn.
2. Propose slide count + section structure + density tier and get approval.
3. Draft PPT-MD in chat, iterate, then render final `.pptx`.

For both formats, load `system/skills/{docx,pptx}/SKILL.md`, render via inline code, update `draft-vF.md` `exports:` entries, and append export activity events.

## Exceptions / Branches

- **Deferred-deliverable shape:** if the brief is intentionally skipped, `draft-vF.md` is a stub containing only frontmatter (`status: deferred`, `reason: "..."`, `back_fill_at: null`, `back_fill_owner: ...`). The reason lives here, not in `campaign.md`.
- **External review returns "kill it"**: follow the cancellation rule in [`library/process/campaign-frontmatter.md`](../../process/campaign-frontmatter.md).
