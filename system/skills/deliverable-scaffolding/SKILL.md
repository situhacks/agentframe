---
name: deliverable-scaffolding
version: 0.1.0
description: |
  Add a new deliverable type to the system. Use when an operator (or a forker
  adapting AgentFrame to a different marketing process) wants to introduce a
  deliverable type that does not exist in library/deliverables/ yet. Carries
  the canonical deliverable-template authoring standard, the
  locate-before-inventing duplication check, and a
  deferred-validation framing for confirming the new template actually constrains over its first 1-2 real
  uses, recorded as a builder-backlog watch. Refuses to scaffold a
  deliverable type that overlaps an existing one by >=70% without explicit
  user justification.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - AskUserQuestion
---

# Deliverable Scaffolding: Add a New Deliverable Type

You are about to add a new entry to the system's deliverable taxonomy. The cost of getting this wrong is taxonomic drift — two templates that overlap silently, neither of which is canonical, both of which need to be patched on every harvest.

This skill carries the discipline. Run every step in order. Refuse to proceed past any gate that fails.

## When to load this skill

Load when the operator (or forker) wants to add a new deliverable type. Concrete signals:

- "I want to add a [thing] deliverable type" / "let's create a template for [thing]".
- A repeat gap: an artifact has reached ready/published in 2+ instances without a template (each instance was bespoke). The friction earns a template.
- A forker setting up AgentFrame for a different marketing process and asking "where do I put my own deliverable definitions?" — this skill is their entry point.

Do NOT load this skill for:

- Patching an existing deliverable type → edit the template under the Builder pre-write gate, or let `deliverable-harvest` propose the patch.
- Changing phase checklists or broad structure routing → start with `.claude/skills/agentframe-structure/SKILL.md`.
- One-off deliverable instances inside a campaign (a one-off slide, a one-off email) → those are produced inline, no template needed.
- Decommissioning a deliverable type → not yet a skill; bespoke under Builder.

## Architectural anchors (why this skill is shaped this way)

- **Architectural Truth #1** (`AGENTS.builder.md` + `AGENTS.operator.md`) — agent + tools + constraints, separated. This skill scaffolds the *constraint* layer (template.md = declarative WHAT). It does NOT generate procedural skill code wrapped around the deliverable; the agent reads the template + context and writes the deliverable natively.
- **Architectural Truth #5 (templates are the product)** — the templates this skill creates are the durable artifact. They port to other agent runtimes. Ship them in the canonical shape.
- **Architectural Truth #6 (resist anticipating future needs)** — this skill itself is a deliberate anticipation exception, justified by the need to keep forked deliverable libraries consistent. The skill therefore enforces strict locate-before-inventing to prevent the anticipation cost from compounding into taxonomic bloat.
- **Locate before inventing** — the new deliverable type must justify its place against the existing taxonomy. The duplication check in Step 1 applies that to deliverable taxonomy.
- **Prior-patch shape discipline** — a new template that does not constrain is a system regression. Validation is deferred to the first 1-2 real uses (see Step 5); the procedural check at scaffold time is the duplication / shape diagnosis (Steps 1 + 3), not a synthetic dry-run.

## The procedure

Five steps. Each gate is binary.

### Step 1 — Locate-before-inventing step (REFUSAL GATE)

Before scaffolding anything, survey existing types.

1. Read every existing `library/deliverables/*/template.md` and `library/domains/*/deliverables/*/template.md` (use Glob then Read).
2. For the proposed new type, write a one-paragraph "What this deliverable is" statement (Purpose + Reader + Author POV + the rough Sections list — 3-5 sentences).
3. Compare against existing types: for each existing type, estimate section overlap and intent overlap with the proposed type.
4. Decision:
   - **Overlap < 40%**: proceed to Step 2.
   - **Overlap 40-70%**: surface to user — *"The proposed `[new-type]` overlaps `[existing-type]` substantially: [name overlapping sections, name distinguishing sections]. Confirm: extend `[existing-type]` instead, or proceed because [distinguishing job is real]."*
   - **Overlap >= 70%**: refuse to scaffold. *"The proposed `[new-type]` is largely the same artifact as `[existing-type]`. Adding it would create taxonomic drift (two templates for one job). Either (a) extend `[existing-type]` with a new section or branch, (b) add a Deliverable subtype convention inside `[existing-type]/template.md` (we don't have one yet, but it earns its place if needed), or (c) override and proceed with explicit justification logged."*
5. State the duplication-check result (which existing types you compared against, the overlap call) to the operator before scaffolding.

**Pass criterion**: explicit overlap call made against every existing type, decision stated.

### Step 2 — Reference-shape step

Pick a stable reference template from existing types — one that matches the new type's general shape (stakeholder-facing? user-voiced? analytical?).

Recommended baselines:

- **Stakeholder-facing analytical doc** (briefs, strategies): `library/domains/marketing/deliverables/business-brief/template.md`.
- **User-voiced creative output** (social posts, scripts): `library/domains/marketing/deliverables/body-copy/template.md`.
- **Internal operational doc** (reviews, plans): the generic shape in `library/deliverables/_meta/deliverable-shape.md`.

Read the chosen reference end-to-end. The new template will use its section order and conventions; the user fills in content.

### Step 3 — Scaffold step

Read `library/deliverables/_meta/template-authoring.md`, then generate `library/deliverables/{new-type}/template.md` using that standard.

The scaffolded template must include only sections that help a future agent decide, execute, compare, or verify. Do not copy a long skeleton into the skill or preserve empty slots for their own sake.

### Step 4 — Wire-up step

Identify and surface all upstream files that need updating to wire the new deliverable type into the system. Do NOT apply these wire-ups silently — surface each to the user with a yes/no, because each wire-up is itself a system patch:

| Wire-up | When needed | If needed, route through |
|---|---|---|
| List the type in its domain pack's `pack.md` deliverable row | When the template lives under `library/domains/{domain}/deliverables/` | Direct edit |
| Add the type to a pack's optional phase checklist | If a domain checklist (marketing: `production.md`) should name it | `.claude/skills/agentframe-structure/SKILL.md` |
| Confirm `status` enum on the new deliverable matches the canonical vocabulary in `library/process/project-frontmatter.md` | Always (no-op when scaffold uses the default `drafting | ready | deferred`; `published` is already available when the artifact is issued immutably) — surface only if the operator proposes a brand-new value | A `project-frontmatter.md` schema change under the Builder pre-write gate, with a `MIGRATION:` line in the commit |
| Add campaign-local export template convention at `workspace/projects/{slug}/exports/templates/{new-type}.{docx,pptx}` | If this deliverable exports to Word/PPT | Surface to user; they create optional templates (this skill does not author binary export templates) |

For each wire-up: ask user "do this now, or defer?" Defer is fine — the deliverable can exist without being named in a phase checklist. Many deliverables are situational.

### Step 5 — Deferred-validation framing

The scaffolded template will be validated by its first 1-2 real uses, not by a synthetic dry-run. The protective work happens in two real-world places: the next time the scaffolded type is drafted in a project, and the `deliverable-harvest` pass at its ready or publish.

**This step's job**: write the validation expectation as a `BB-*` watch row in `system/builder-backlog.md`, then stop.

Write a one-paragraph **validation expectation**:

> **Validation expectation**: the first real use of `library/deliverables/{new-type}/template.md` in a campaign is the validation point. The campaign's deliverable should observably reflect the template's hard constraints + section structure. If the first draft from the new template produces output that visibly hits the constraints (e.g. a Hard Constraint of "audience named at the person level" produces drafts naming a specific person, not "marketers in general"), resolve the watch row as passed. If the first draft ignores the template's constraints (output looks the same as a bespoke one would have), the template has the wrong shape — surface to the user, redesign the shape, and append the redesigned scaffold via this skill.

**Common shape rescues** (apply if validation comes back FAIL after first real use):

- The template is too generic. Tighten Hard constraints with earned, specific rules.
- The template has too many soft "consider X" prompts and not enough hard rules. Convert soft prompts to hard constraints, or remove them.
- The template duplicates an existing type. Re-run Step 1 — the duplication check may have been too lenient.

**No subagent dispatch.** The watch row stays open until the harvest after the first real use resolves it.

**Optional human-in-the-loop dry-run (rare)**: if the operator wants extra confidence before relying on the scaffold, the operator may manually draft a sample deliverable from the template themselves and judge whether it constrained their drafting. Operator-driven, opt-in, not a default gate.

## What this skill does NOT do

- **Does not author the new template's content.** The skill provides the canonical shape (sections in order, conventions, format). The user fills in Purpose, POV specifics, Section descriptions, Hard constraints, Tone notes, Edge cases — that is the domain knowledge that earns the deliverable's place.
- **Does not author binary export templates.** If the deliverable exports to Word or PPT, the user owns any optional campaign template files at `workspace/projects/{slug}/exports/templates/{new-type}.{docx,pptx}`. This skill notes the wire-up; the user owns the visual design.
- **Does not own the deliverable's lifecycle in a campaign.** When the deliverable is drafted, how feedback works, and when it becomes ready live in the template the user fills in (Readiness Criteria). This skill installs the slot; the user fills the slot.
- **Does not decommission deliverable types.** Removing a deliverable type is bespoke under Builder; no skill earns its place yet. When the first removal-with-friction surfaces, `system/skills/deliverable-decommission/SKILL.md` lands alongside this one.

## Edge cases

- **The new deliverable is actually a phase, not a deliverable** (e.g. user says "let's add a discovery phase deliverable"): surface the category confusion. Phases belong to the project plan ([`project-flow.md`](../../../library/process/project-flow.md)) or a pack's optional phase checklist; deliverables produced within phases are here. A new checklist phase is a Builder decision routed through `.claude/skills/agentframe-structure/SKILL.md`.
- **The new deliverable is a one-off for a single campaign**: refuse to scaffold. Templates earn their place from repeat use. *"This sounds like a one-off artifact for `[campaign-slug]`. Templates earn their place from 2+ uses. Draft this inline in the campaign for now; if it shows up again in a future campaign, scaffold then."*
- **User wants to scaffold from a template that doesn't exist yet**: ask which existing type is closest. Use that as the reference baseline. The skill cannot bootstrap a deliverable type from nothing — there must always be a reference shape.
- **Forker is scaffolding a deliverable for a marketing process that has zero overlap with this fork's existing types** (e.g. forker is in B2B SaaS, fork ships consumer-content templates): the locate-before-inventing check will return ~0% overlap on every comparison, which is the correct signal. Proceed; the new template anchors a new family within this fork's `library/deliverables/`. Their next deliverable type compares against the new entry too.

## How this skill is invoked

- **Operator-initiated**: "Let's add a `[type]` deliverable" → load this skill.
- **Forker first-touch**: forker reads [`README.md`](../../../README.md) (product orientation) and [`AGENTS.operator.md`](../../../AGENTS.operator.md), then comes here to add their own deliverable types matching their marketing process.
- **From `deliverable-harvest`**: when a harvest finds "we have no template for this artifact and we keep producing it bespoke", it may surface "this looks like a new deliverable type — load `deliverable-scaffolding`".

## Forker note

If you are reading this as a fork operator setting up your own deliverable taxonomy: `library/deliverables/_meta/template-authoring.md` is the canonical shape. Keep sections only when they help a future agent decide, execute, compare, or verify.
