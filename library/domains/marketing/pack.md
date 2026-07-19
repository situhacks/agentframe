---
domain: marketing
prefix: mkt
extension_fields: []
verbs: [ready, publish, version, doctor]
assembly_record: post-FINAL.md
flows: [open-flow, marketing-solo-flow, marketing-standard-flow]
---

# Marketing domain pack

The marketing domain: campaigns that ship posts. A marketing project is a campaign — `domain: marketing`.

This pack is the only artifact that knows marketing. The generic spine (`af.py`) and the generic router (`AGENTS.md`) read what is declared here; they name no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | `extension_fields` (none hard-required) + `prefix` | `post_manifest` appears at a real manifest moment and project-level `shipped_at` appears on first publish; both stay absent for marketing work that does not use them. The prefix is `mkt-`. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | the minimal `project.md` index and body `new-project` writes for this domain. |
| Deliverable templates | [`deliverables/`](deliverables/) | post-final, body-copy, slide-copy, substack-essay, campaign-brief, campaign-architecture, research-artifact, business-brief. |
| Verb applicability + hooks | `verbs` above + [`rules.py`](rules.py) | `ready` runs the post-FINAL assembly hook; generic `publish` calls the marketing receipt hook; `assembly_record` names the unversioned accumulator. |
| Derived post totals | [`rules.py`](rules.py) | Publish receipts count published `post-*` rows across the working tracker and archive without persisting a second counter. |
| Persona routing | [`production.md`](production.md) | the post-production / carousel / publish / performance routing the Operator lazy-loads for marketing production work. |

## Flows this domain offers

`open-flow` (the agnostic default), plus the opt-in marketing flows `marketing-solo-flow` and `marketing-standard-flow` (post ladders, manifest moment, post-FINAL assembly), named deliberately by a marketer. Flow files live shared in `library/process/flows/`; this pack only lists which it offers.
