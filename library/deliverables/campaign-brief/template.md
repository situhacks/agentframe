# Template: Campaign Brief

## Purpose

Where the Business Brief framed the opportunity, the Campaign Brief frames the campaign that delivers on it. At a level the marketing team can react to and steer. NOT the full execution plan — that lives in Campaign Architecture. The sweet spot here is "here's the story we'd tell, here are 2 angle options per post, react before we go build." Stakeholder-facing when a stakeholder exists; exports to Word + PPT in that case. Otherwise drafts to lock without review.

## Depends On

- Business Brief locked (audience direction + success criteria become inputs; do NOT re-derive them)
- `library/context/operator/positioning.md` loaded (for Key message anchoring)

## Output Shape

A structured business document for skim, markup, and approval. Bullets and sub-bullets over paragraphs; declarative prose where it appears. Institutional or team voice for stated positions; first person sparingly, when a specific call needs an owner. Don't narrate your own document.

Author: a marketer drafting this for review. Reader: the same stakeholder who approved the business brief, deciding whether to greenlight planning.

- **Theme** — one sentence
- **Audience direction** — inherited from Business Brief; sharpened only if the original proves too broad
- **Channel hypothesis** — named channels with reasoning, not "social media"
- **Key message** — anchored in `positioning.md`
- **Design direction** — 2-3 light visual hypotheses (palette intent + typography mood + motif territory). Seeds Phase 3 `design-language`; do not drift into full `direction-compare.html` fidelity.
- **Timeline** — milestones, not a Gantt
- **Success metrics** — binary or quantified
- **Light narrative arc** — 3-5 sentences (the STORY, not the post list)
- **Light post list with 2 angle options per post** — explicitly NOT a locked plan; leaves room for stakeholder direction

## Hard Constraints

- Stakeholder-facing. Story-led. Treat the reader as someone who'll react with "I'd lean toward angle B for post 3" — give them something to react to, not a plan to approve. Keep it tight; if it's longer than 2 pages of Word, you've drifted into Campaign Architecture territory.
- Theme stated in one sentence
- Channel hypothesis names specific channels with reasoning
- Design direction lists 2-3 light hypotheses, not locked picks — room for Phase 3 to develop
- Light arc has beginning, middle, end (a STORY, not a list)
- Post list shows angle options, not locked picks — room for stakeholder direction
- Success metrics binary or quantified
- Every post has a clear role (hook/build/payoff/CTA), no filler
- Key message anchored in `positioning.md`

## Draft Frontmatter Convention

The `phase-2-strategy/campaign-brief/draft-v{N}.md` file carries this YAML frontmatter:

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
exports:
  - {path: <relative path to exported file>, generated_at: <ISO-8601 timestamp>}
---
```

## Lock Criteria

- Internal draft approved by operator
- Frontmatter `status` set to `locked`
- Campaign tracker `deliverables.campaign-brief.status` set to `locked` in the same turn
- Word + PPT exports generated to `phase-2-strategy/campaign-brief/exports/` (per Publish / Export Mechanics below)
- **If external review is the path**: reviewer feedback applied OR explicitly `waived` with reason logged + `phase_override` entry in the campaign's `activity.md`. **If review is `not_required`**: no review step — drafting → lock is the path; no override log needed.
- Final markdown saved with `last_updated` frontmatter set
- Unblocks Phase 3

## Review Path

- **Path**: external **when a stakeholder exists** (typically the same reviewer as the Business Brief). Otherwise the brief drafts to lock with no review — that is the normal path for solo work, not exception-handling. See the selected `campaign_flow` in `campaign.md` for flow-specific review defaults.
- **Reviewer**: typically manager or business stakeholder (when external is the path); same reviewer as Business Brief by default.
- **Coordination**: when external review IS the path, agent offers to draft the email + calendar invite on export. Often combined with Business Brief review in one meeting.

## Publish / Export Mechanics

When this deliverable locks, the agent exports it to Word + PowerPoint per the agent-first export pipeline:

- **Supported formats**: `.docx`, `.pptx`
- **Template source**: campaign-local templates are optional at `workspace/campaigns/{slug}/exports/templates/campaign-brief.{docx,pptx}`
- **PPT discovery first**: ask audience, time budget, presenter style, and decision-vs-handoff; propose slide count + section structure + density tier; get approval before drafting slides.
- **PPT-MD intermediate** (for `.pptx`): `system/skills/pptx/pptx-md.md` — agent drafts PPT-MD in chat, iterates with user, then renders the final `.pptx`.
- **Output path**: `workspace/campaigns/{slug}/phase-2-strategy/campaign-brief/exports/campaign-brief-v{N}.{ext}`

Mechanics: agent loads the relevant skill (`system/skills/{docx,pptx}/SKILL.md`), reads the master template + config, writes inline `python-docx` / `python-pptx` code, saves the file, updates `draft-v{N}.md` frontmatter `exports:` array, and appends `lock_event` + `export_generated` events to the campaign's `activity.md`. No standalone export script exists or is needed.

Full architecture is defined by the deliverable template, `system/skills/{docx,pptx}/SKILL.md`, and campaign state files.

## Exceptions / Branches

- **Stakeholder steers the post list significantly**: revise here, do not let Campaign Architecture absorb the change silently. The brief is the artifact of record.
