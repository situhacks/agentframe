---
domain: marketing
prefix: mkt
extension_fields: []
verbs: [lock, publish, version, doctor]
assembly_record: post-FINAL.md
flows: [open-flow, marketing-solo-flow, marketing-standard-flow]
---

# Marketing domain pack

The marketing domain: campaigns that ship posts. A marketing project is a campaign — `domain: marketing`.

This pack is the only artifact that knows marketing. The generic spine (`af.py`) and the generic router (`AGENTS.md`) read what is declared here; they name no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | `extension_fields` (none hard-required) + `prefix` | the post fields a marketing project adds **when it runs a post campaign** (`post_manifest`, `post_count`, `posts_published`, `shipped_at`/`shipped_media`) — optional, since a case study or workshop ships no posts — plus the `mkt-` folder prefix. The skeleton seeds them; `doctor` does not require them. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | the `project.md` body `new-project` writes for this domain (carries the MANIFEST block + counters). |
| Deliverable templates | [`deliverables/`](deliverables/) | post-final, body-copy, slide-copy, campaign-brief, campaign-architecture, research-artifact, business-brief. |
| Verb applicability + hooks | `verbs` above + [`rules.py`](rules.py) | `publish` is marketing-only (a domain that omits it from `verbs` has `publish` rejected); `lock` runs the post-FINAL assembly hook; `assembly_record` names the unversioned accumulator. |
| Doctor rules | [`rules.py`](rules.py) `check()` | the post-counting reconciliation (`posts_published` = count of delivered `post-*` rows). |
| Persona routing | [`production.md`](production.md) | the post-production / carousel / publish / performance routing the Operator lazy-loads for marketing production work. |

## Flows this domain offers

`open-flow` (the agnostic default), plus the opt-in marketing flows `marketing-solo-flow` and `marketing-standard-flow` (post ladders, manifest moment, post-FINAL assembly), named deliberately by a marketer. Flow files live shared in `library/process/flows/`; this pack only lists which it offers.
