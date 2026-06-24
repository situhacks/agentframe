---
domain: marketing
prefix: mkt
extension_fields: [post_manifest, posts_published, post_count, shipped_at]
verbs: [lock, publish, version, doctor]
assembly_record: post-FINAL.md
flows: [open-flow, marketing-solo-flow, marketing-standard-flow]
---

# Marketing domain pack

The "CMO" domain: campaigns that ship posts. A marketing project is a campaign — `domain: marketing`.

This pack is the only artifact that knows marketing. The generic spine (`af.py`) and the generic router (`AGENTS.md`) read what is declared here; they name no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | the `extension_fields` above + `prefix` | the fields a marketing project adds to the neutral core (`post_manifest`, the post counters, `shipped_at`/`shipped_media` publish metadata), and the `mkt-` folder prefix. `doctor` validates these for `domain: marketing`; `new-project` reads the prefix. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | the `project.md` body `new-project` writes for this domain (carries the MANIFEST block + counters). |
| Deliverable templates | [`deliverables/`](deliverables/) | post-final, body-copy, slide-copy, campaign-brief, campaign-architecture, research-artifact, business-brief. |
| Verb applicability + hooks | `verbs` above + [`rules.py`](rules.py) | `publish` is marketing-only (a domain that omits it from `verbs` has `publish` rejected); `lock` runs the post-FINAL assembly hook; `assembly_record` names the unversioned accumulator. |
| Doctor rules | [`rules.py`](rules.py) `check()` | the post-counting reconciliation (`posts_published` = count of delivered `post-*` rows). |
| Persona routing | [`production.md`](production.md) | the post-production / carousel / publish / performance routing the Operator lazy-loads for marketing production work. |

## Flows this domain offers

`open-flow` (the agnostic default), plus the opt-in marketing flows `marketing-solo-flow` and `marketing-standard-flow` (post ladders, manifest moment, post-FINAL assembly), named deliberately by a marketer. Flow files live shared in `library/process/flows/`; this pack only lists which it offers.
