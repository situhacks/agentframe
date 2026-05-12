# Template: Research Artifact

## Purpose

Capture the accepted research for a detailed campaign before strategy work begins. This is the Phase 1 handoff into Business Brief.

## Depends On

- `phase-1-research/idea-bank.md`
- Selected campaign direction from the operator
- Gemini Deep Research API output or operator-provided Gemini web output

## Output Shape

Use these as directional headings, not a rigid schema. The same section block is injected into Gemini DR prompts as the required return structure, so keep it tight and self-contained.

- **Campaign Direction** — selected idea, working angle, and why it is worth exploring.
- **Market Context** — what is happening, why now, and what makes the topic timely.
- **Audience Signals** — who seems to care, what they are trying to do, and what pressures or objections matter.
- **Messaging Territory** — useful themes, claims, tensions, whitespace, and angles to test.
- **Evidence And Sources** — cited sources and what each contributes.
- **Open Questions** — what needs validation before strategy locks.

## Hard Constraints

- Cite factual claims when sources are available.
- Separate evidence from hypotheses.
- Call out thin or conflicting evidence plainly.
- Preserve enough context that Business Brief can cite the artifact without rerunning research.
- Do not force empty sections. If a section has nothing useful, say so briefly.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | locked>
last_updated: <ISO-8601 timestamp>
current_version: <integer>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason>"}
research_method: <gemini_deep_research_api | gemini_web_handoff | manual_sources>
source_material:
  - {path: <relative path>, kind: <raw_export | uploaded_source | transcript | pasted_text>}
---
```

## Exceptions / Branches

For API runs, preserve Gemini's native interaction JSON under `source-material/` and use `system/research/gemini_deep_research.py` to extract the Markdown handoff. If extraction fails, keep the raw JSON and ask for Builder help; do not fabricate a research artifact from an unknown response shape.

## Lock Criteria

- Operator accepts the campaign direction as worth taking into Phase 2.
- Research artifact is saved at `phase-1-research/research-artifact-vF.md`.
- Key factual claims have citations or are clearly marked as hypotheses.
