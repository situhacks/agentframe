# Campaign Flows

Campaign flows are the first-class campaign process definitions for AgentFrame Marketing. A flow owns phase sequence, required deliverables by phase, tracker transitions, flow-specific skips, and completion criteria.

## Default Flow

`solo-flow.md` is the default for new campaigns. Most operators are solo marketers; the default path should optimize for a single accountable owner without assuming stakeholder review.

Each campaign instance records its selected flow in `campaign.md` frontmatter as `campaign_flow`. During campaign work, read `campaign_flow` first, then lazy-load `library/process/campaign-flows/{campaign_flow}.md`.

## Available Flows

| Flow | Status | Use Case |
|---|---|---|
| `solo-flow.md` | Default | Solo marketer, no stakeholder review, lean phase sequence. |
| `standard-flow.md` | Optional | Fuller campaign with business/stakeholder review cycles. |
| `open-flow.md` | Optional | Build-as-you-go: the agent proposes a plan (phases + deliverables) scaled to the objective; the operator narrows and the plan rolls forward. |
| `enterprise-flow.md` | RESERVED | Not yet authored. |

## Ownership

- Flow registry and default selection live here.
- Per-campaign flow selection lives in `workspace/campaigns/{slug}/campaign.md` frontmatter.
- Flow-specific phase sequencing lives in the flow file.
- Shared process primitives stay in sibling process files such as `campaign-frontmatter.md`, `lock-event.md`, `voice-mini-retro.md`, and `composio-notes.md`.
- Deliverable details stay in `library/deliverables/{type}/template.md`.

## Adding Or Changing A Flow

Use `system/skills/agentframe-structure/SKILL.md`, then load `campaign-flow-changes.md` from that skill.
