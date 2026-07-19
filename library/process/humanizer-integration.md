# Humanizer Integration

## Purpose

Own when and where the [`humanizer` skill](../../system/skills/humanizer/SKILL.md) runs on public-facing, agent-authored prose. The skill owns rewrite technique; this process owns authorship timing and scope.

## When To Load

Load when a public-facing deliverable template directly requires this process in `Before Writing`, `Humanizer Pass`, or its readiness criteria. Do not require a literal `## Humanizer Pass` heading: the resolved template's direct route is the trigger.

Reload before a resumed prose-drafting turn if the current authorship state is unclear.

## Procedure

### 1. Establish authorship state

Classify the prose being changed:

| State | Humanizer action |
|---|---|
| Initial agent-authored prose | Full pass before the first review surface |
| Later prose the agent materially rewrote | Delta pass on agent-changed regions before the next review surface |
| Structural/frontmatter-only change | No pass |
| Operator hand-tuning or fine copy edits | No pass unless the operator asks |
| Ready transition with no new agent-authored prose | Verify the earlier pass; do not rerun |

If authorship is mixed, preserve operator-written regions and limit the pass to text the agent generated or materially rewrote. When region ownership cannot be recovered safely, surface the ambiguity rather than normalizing the whole artifact.

### 2. Apply scope

Apply the skill to prose paragraphs or the template-named prose surface. Skip frontmatter, code blocks, structured tables, labels, citations, and operator-authored text. A template may narrow scope further, such as slide prose or `video/SCRIPT.md`.

The output is revised prose, not a critique report. Recheck meaning, claims, links, and required voice markers after the rewrite.

### 3. Preserve the fine-tuning boundary

Humanizer belongs early in the agent-drafting loop. Once the operator begins fine-tuning wording, their edits become the higher-priority voice signal. Do not run another broad pass at readiness or publish and wash those edits back into generic prose.

If the agent makes a later substantive rewrite after operator tuning, run a delta pass only on the agent-rewritten region and compare the boundary sentences for continuity.

## Verification Or Logging

Before surfacing an initial or materially rewritten draft, state briefly that the pass ran and name its scope. At readiness, verify that the relevant early/delta pass occurred and that no new agent-authored region was added afterward.

Do not append a `humanizer_pass` event for every pass to `activity.md`; routine drafting narration belongs in the version chain or conversation. Log only a material override or failure when downstream work depends on it.

## Boundaries

- This process does not decide whether a deliverable is public-facing; the template does.
- It does not run on operator-authored prose without a request.
- It does not replace the voice system, banned-word audit, factual review, or template readiness criteria.
- Readiness is a verification point, not the default time for the first humanizer pass.
