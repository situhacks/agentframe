# AgentFrame Marketing — Manifesto

> **About this fork.** **AgentFrame Marketing** is the product name for the system originally built as **MarketingOS** by Brandon Situ to run his own marketing as a solo consultant-builder (same codebase and templates; rebrand only). The manifesto, principles, and machinery are intentionally forkable — anyone who wants a single-operator marketing OS for their own practice can swap the operator content (`library/context/operator/`), adjust campaign flows (`library/process/campaign-flows/`), and run their own campaigns through the same harness. "The operator" below is whoever runs this fork; if it's you, it's you.

## What it is
A project-based marketing OS for one consultant-builder running multiple campaigns
in parallel. The agent is the engine; files are memory; deliverable templates are
the product; the system is the training wheels.

## Who it's for
A single operator running their own marketing — typically a consultant, founder,
or builder publishing content under their own name. Resist generalizing: this
isn't an AI assistant for marketing teams, agencies, or multi-tenant SaaS.
Single-user system is a feature, not a limitation.

The original fork operator is Brandon Situ (consultant living in both worlds —
enterprise AI by day, builder by night). The manifesto pattern is the contribution;
the specific operator content is the operator's to own.

## What it does (the 4 jobs)
1. Thinking partner that's actually smarter — Tier-1 critique, recommendations
   with reasoning, push-back without being asked.
2. PMO — tracks all campaigns via campaign.md frontmatter, surfaces stale work,
   missed retros, drift. (Today: in-session daily check-in. Phase D: ambient
   cross-channel nudges via daemon.)
3. Knows the marketing process and deliverables, but stays flexible — campaign flows
   sequence with documented overrides; templates declare WHAT, agent figures HOW.
4. Self-improving — every campaign sharpens templates, voice, and agent
   behaviour through Phase-5 System Retros with smart routing
   (template patch vs voice patch vs positioning patch vs agent-rule patch).

## What's actually being built (the durable claim)
The deliverable templates are the product. The agent + harness + retros + audit
and telemetry layer are the training wheels that battle-test those templates over
real campaigns.
Once a template stabilizes (System Retro patches converge to
near-zero), it's done — and it's portable. Drop
`library/deliverables/post-copy/template-vF.md` into M365 Declarative Agents,
Azure AI Foundry, or Vertex AI Agent Builder, and it works because it carries
real campaign-tested constraints, not GPT-4-era prompt engineering.

This is the public claim the system has to credibly back.

## How responsibility is split (two-mode routing)
- CMO mode owns: drafting deliverables, running retros, posting copy, deliverable
  template patches, voice/positioning patches.
- Builder mode owns: AGENTS.*.md, system architecture, schema, machinery, process
  files, this manifesto.
- Mode boundary is enforced (no silent swaps). What flows where gets logged.
  Items surfaced in CMO that need Builder design get a tagged backlog entry in
  `system/builder-backlog.md`.

## What it is NOT
- Not a SaaS product. Not a multi-tenant system. Not portable wholesale (only
  templates are portable; the harness is Cursor-specific by design).
- Not an agent that does the operator's job. It does the project management around
  the work so one person can run more campaigns coherently.
- Not done. Phase D (daemon, Discord, MCP, ambient research) is queued.
  Cluster E in the roadmap.

## Inspiration cited
- Karpathy CLAUDE.md framing (problem-led principles): we borrow shape, not content.
- Hermes-style retro routing (Nous Research): the retro proposes where each
  feedback item routes; user approves.
- Anthropic skills architecture: skills as generic capabilities; orchestration
  lives in the agent + project context, not the skill.
