# Template: Business Frame

## Purpose

Turn UC1 intake plus a Copilot Researcher artifact into a structured Deloitte Canada Marketing Business Frame. This is the strategy bridge before downstream campaign brief work: it captures the internal sponsor's business need, external target audience, competitive landscape, GTM hypotheses, evidence, and questions for business leaders.

## Depends On

- UC1 intake worksheet: `phase-1-research/intake.md`
- Copilot Researcher artifact or text source: `phase-1-research/source-material/*`
- UC1 generation prompt: `docs/uc1-browser-agent-handoff/copilot-studio-prompt.txt`
- UC1 output contract: `docs/uc1-browser-agent-handoff/BUSINESS_FRAME_OUTPUT_PARAMETERS.md`

## Sections

**Author:** a marketer preparing a leader-ready business frame for validation with a sponsoring service line.
**Reader:** internal marketing, manager, or business stakeholder who needs to decide whether the campaign direction is credible enough to proceed.

Use stakeholder-facing prose. Be specific, evidence-grounded, and direct. Bullets are preferred where they improve skim value. Do not pad thin evidence; mark sections `draft` or `needs_clarification` when the artifact does not support confident content.

The generated frame follows the UC1 17-section registry in `docs/uc1-browser-agent-handoff/BUSINESS_FRAME_OUTPUT_PARAMETERS.md`:

1. Executive Summary
2. Business Need - Context
3. Business Need - Problem
4. Business Need - Opportunity
5. Business Need - Why Now
6. Business Need - Value Drivers
7. Target Audience - Who They Are
8. Target Audience - Decision Behaviors / Buying Criteria
9. Target Audience - Assumptions Requiring Validation
10. Competitive Landscape - Competitors & Narratives
11. Competitive Landscape - Deloitte Differentiation
12. Competitive Landscape - White Space
13. Operationalizing - Market Gaps & Opportunities
14. Operationalizing - Deloitte Capabilities to Activate
15. Operationalizing - GTM Hypotheses & Angles
16. Evidence & Sources - Appendix
17. Clarifying Questions - For Business Leaders

## Hard Constraints

- Canonical file: `phase-2-strategy/business-frame/draft-vF.md`. Versioning and snapshots follow the shared convention.
- The source artifact must be saved in the campaign's `phase-1-research/source-material/` folder before generation.
- `sponsoring_service_line` must be included in the Researcher prompt and Business Frame generation context even when request JSON omits it.
- Generated JSON must contain exactly 17 sections in registry order before it can be converted into `draft-vF.md`.
- Each generated section must include the UC1 fields: `section_id`, `section_name`, `parent_section`, `order`, `status`, `content`, and `clarifying_questions`.
- Valid statuses are `complete`, `draft`, and `needs_clarification`.
- Claims must be grounded in the research artifact or transcript; unsupported claims become assumptions or clarifying questions.
- The executive summary is written last, after the other sections are known.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
current_version: <integer>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason>"}
review: <not_required | pending | complete | waived>
source_artifacts:
  - {path: <relative path>, received_at: <ISO-8601 timestamp>, kind: research_artifact}
exports: []
---
```

## Review Path

External when a stakeholder exists. Otherwise drafting to lock is valid with `review: not_required`.

## Lock Criteria

- Intake answers are captured.
- Research artifact is saved under `phase-1-research/source-material/`.
- Generated output has passed the 17-section registry validation.
- Any `needs_clarification` sections are either resolved or explicitly accepted by the operator as open leader questions.
- `draft-vF.md` frontmatter is updated to `status: locked`.
- Tracker updates and lock events follow `library/process/lock-event.md` and the active campaign flow.

## Exceptions / Branches

- **Artifact is copied from Downloads with the wrong filename:** keep the file, record the actual path in `source_artifacts`, and rename only if the operator confirms.
- **Copilot cannot produce `.docx`:** save the best available export or copied text under `phase-1-research/source-material/`, note the deviation in `feedback-log.md`, and continue only if the artifact is parseable.
- **Transcript or manager feedback provided after first draft:** save the canonical input under `phase-2-strategy/business-frame/feedback/`. If AgentFrame extracts it into a machine-readable form, save the extracted file beside the canonical input.
