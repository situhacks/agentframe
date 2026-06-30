# Flows

Flows are the first-class process definitions for the project harness. A flow owns phase sequence, required deliverables by phase, tracker transitions, flow-specific skips, and completion criteria.

## Default Flow

`open-flow.md` is the default for new projects and the standard across every domain. Open flow is build-as-you-go: the agent proposes a plan (phases + deliverables) scaled to the objective, the operator narrows it, and the plan rolls forward. Almost every project is open by nature, so it is the default unless the operator names another flow.

Each project instance records its selected flow in `project.md` frontmatter as `flow`. During work, read `flow` first, then lazy-load `library/process/flows/{flow}.md`.

## Available Flows

| Flow | Status | Use Case |
|---|---|---|
| `open-flow.md` | Default (all domains) | Build-as-you-go: the agent proposes a plan scaled to the objective; the operator narrows and the plan rolls forward. |
| `marketing-solo-flow.md` | Marketing, opt-in | Named deliberately by a marketer: lean fixed phase ladder, single accountable owner, no stakeholder review. |
| `marketing-standard-flow.md` | Marketing, opt-in | Named deliberately by a marketer: fuller project with business/stakeholder review cycles. |
| `project-mgmt-open-flow.md` | Project-mgmt, opt-in | Open flow + a governance kickoff. Pick when a PM engagement is long-horizon enough to need a charter and living governance docs; a one-off PM deliverable stays on `open-flow`. |

`marketing-solo-flow` and `marketing-standard-flow` are marketing-domain flows (post ladders, manifest moment, post-FINAL assembly). They are named deliberately, never defaults. `project-mgmt-open-flow` is the project-mgmt analogue — `open-flow` semantics plus a governance kickoff, also opt-in and never a default. A domain uses `open-flow` until it grows its own named flow.

## Ownership

- Flow registry and default selection live here.
- Per-project flow selection lives in `workspace/projects/{slug}/project.md` frontmatter.
- Flow-specific phase sequencing lives in the flow file.
- Shared process primitives stay in sibling process files such as `project-frontmatter.md`, `lock-event.md`, `voice-mini-retro.md`, and `composio-notes.md`.
- Deliverable details stay in `library/deliverables/{type}/template.md`.

## Adding Or Changing A Flow

Use `system/skills/agentframe-structure/SKILL.md`, then load `flow-changes.md` from that skill.
