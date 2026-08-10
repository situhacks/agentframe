# Humanizer Integration

## Purpose

Own when and where the [vendored humanizer](../../system/skills/humanizer/SKILL.md) runs on public-facing, agent-authored prose. The skill owns rewrite technique; this process owns authorship timing and scope.

**Read the vendored file at that path. Never invoke a skill named `humanizer` by name** — a different skill of the same name may be installed globally, and its patterns are numbered differently and calibrated against a different voice. The carve-outs in `anti-patterns.md` cite the vendored file's numbering, so the wrong file silently applies the wrong rules.

## When To Load

Load when a public-facing deliverable template directly requires this process in `Before Writing`, `Humanizer Pass`, or its readiness criteria. Do not require a literal `## Humanizer Pass` heading: the resolved template's direct route is the trigger.

Reload before a resumed prose-drafting turn if the current authorship state is unclear.

## Procedure

### 1. Establish authorship state

**The humanizer runs once per deliverable.** It is a rewrite pass over whole paragraphs, so every additional run is another chance to overwrite wording the operator chose. `anti-patterns.md` is the opposite — a checklist that flags and repairs named moves without touching anything else — so it keeps running on every agent-authored region for the life of the deliverable. Rewriting is once; checking is forever.

Classify the prose being changed:

| State | Humanizer action |
|---|---|
| Initial agent-authored prose | **The one full pass.** Run it before the operator ever sees the draft |
| Later prose the agent materially rewrote | No humanizer. Apply `anti-patterns.md` to the rewritten region instead, and match the surrounding text (`pairs/plain-not-clever.md` mimic-nearby) |
| Structural/frontmatter-only change | No pass |
| Operator hand-tuning or fine copy edits | No pass unless the operator asks |
| Ready transition | Verify the one pass happened; never run it here |

If authorship is mixed, preserve operator-written regions. When region ownership cannot be recovered safely, surface the ambiguity rather than normalizing the whole artifact.

The full sequence the operator wants: **anti-patterns + humanizer on the first complete draft → the operator's own voice pass → anti-patterns only, on anything the agent writes after that.**

### 2. Apply scope

Apply the skill to prose paragraphs or the template-named prose surface. Skip frontmatter, code blocks, structured tables, labels, citations, and operator-authored text. A template may narrow scope further, such as slide prose or `video/SCRIPT.md`.

The output is revised prose, not a critique report. Recheck meaning, claims, links, and required voice markers after the rewrite.

### 3. Run the clean pass out of the drafting context

The style pass loads corpus exemplars; the clean pass must not run while they are loaded, or it grades the draft against the thing it was imitating. The move is not a new session and not a subagent: **write the draft to its version file, then run the pass as its own turn against the file on disk**, loading only `anti-patterns.md` and one short Brandon sample for calibration. Do not re-read the corpus in that turn. Rewrite the file in place before surfacing it.

This is a sequencing requirement, not a scheduling one — deferring the pass because the context is "wrong" is the failure it exists to prevent.

### 4. Preserve the fine-tuning boundary

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
