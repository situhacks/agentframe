---
name: deliverable-scaffolding
version: 0.1.0
description: |
  Add a new deliverable type to the system. Use when an operator (or a forker
  adapting AgentFrame Marketing to a different marketing process) wants to introduce a
  deliverable type that does not exist in library/deliverables/ yet. Carries
  the canonical deliverable-template authoring standard, the
  locate-before-inventing duplication check, a
  deferred-validation framing for confirming the new template actually constrains over its first 1-2 real
  uses, and a `system_changes` audit row. Refuses to scaffold a
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

You are about to add a new entry to the system's deliverable taxonomy. The cost of getting this wrong is taxonomic drift — two templates that overlap silently, neither of which is canonical, both of which need to be patched on every system retro.

This skill carries the discipline. Run every step in order. Refuse to proceed past any gate that fails.

## When to load this skill

Load when the operator (or forker) wants to add a new deliverable type. Concrete signals:

- "I want to add a [thing] deliverable type" / "let's create a template for [thing]".
- An audit-history gap: an artifact has shipped 2+ times without a template (each instance was bespoke). The friction earns a template.
- A forker setting up AgentFrame Marketing for a different marketing process and asking "where do I put my own deliverable definitions?" — this skill is their entry point.

Do NOT load this skill for:

- Patching an existing deliverable type → that is `system/skills/system-improvement/SKILL.md`.
- Changing campaign flows, phase systems, default flow selection, or broad structure routing → start with `system/skills/agentframe-structure/SKILL.md`.
- One-off deliverable instances inside a campaign (a one-off slide, a one-off email) → those are produced inline, no template needed.
- Decommissioning a deliverable type → not yet a skill; bespoke under Builder.

## Architectural anchors (why this skill is shaped this way)

- **Architectural Truth #1** (`AGENTS.builder.md` + `AGENTS.cmo.md`) — agent + tools + constraints, separated. This skill scaffolds the *constraint* layer (template.md = declarative WHAT). It does NOT generate procedural skill code wrapped around the deliverable; the agent reads the template + context and writes the deliverable natively.
- **Architectural Truth #5 (templates are the product)** — the templates this skill creates are the durable artifact. They port to other agent runtimes. Ship them in the canonical shape.
- **Architectural Truth #6 (resist anticipating future needs)** — this skill itself is a deliberate anticipation exception, justified by the need to keep forked deliverable libraries consistent. The skill therefore enforces strict locate-before-inventing to prevent the anticipation cost from compounding into taxonomic bloat.
- **Cross-cutting earning rule** — the new deliverable type itself must earn its place, just like a new constraint must. The duplication check in Step 1 is this rule applied to deliverable taxonomy.
- **Prior-patch shape discipline** — a new template that does not constrain is a system regression. Validation is deferred to the first 1-2 real uses (see Step 5); the procedural check at scaffold time is the duplication / shape diagnosis (Steps 1 + 3), not a synthetic dry-run.

## The procedure

Six steps. Each gate is binary.

### Step 1 — Locate-before-inventing step (REFUSAL GATE)

Before scaffolding anything, survey existing types.

1. Read every existing `library/deliverables/*/template.md` (use Glob then Read; there are typically 8-12 files, fast).
2. For the proposed new type, write a one-paragraph "What this deliverable is" statement (Purpose + Reader + Author POV + the rough Sections list — 3-5 sentences).
3. Compare against existing types: for each existing type, estimate section overlap and intent overlap with the proposed type.
4. Decision:
   - **Overlap < 40%**: proceed to Step 2.
   - **Overlap 40-70%**: surface to user — *"The proposed `[new-type]` overlaps `[existing-type]` substantially: [name overlapping sections, name distinguishing sections]. Confirm: extend `[existing-type]` instead, or proceed because [distinguishing job is real]."*
   - **Overlap >= 70%**: refuse to scaffold. *"The proposed `[new-type]` is largely the same artifact as `[existing-type]`. Adding it would create taxonomic drift (two templates for one job). Either (a) extend `[existing-type]` with a new section or branch, (b) add a Deliverable subtype convention inside `[existing-type]/template.md` (we don't have one yet, but it earns its place if needed), or (c) override and proceed with explicit justification logged."*
5. Record the duplication-check result (which existing types you compared against, the overlap call) — this gets included in the `system_changes` row in Step 6.

**Pass criterion**: explicit overlap call made against every existing type, decision logged.

### Step 2 — Reference-shape step

Pick a stable reference template from existing types — one that matches the new type's general shape (stakeholder-facing? user-voiced? analytical?).

Recommended baselines:

- **Stakeholder-facing analytical doc** (briefs, strategies): `library/deliverables/business-brief/template.md`.
- **User-voiced creative output** (social posts, scripts): `library/deliverables/post-copy/template.md`.
- **Internal operational doc** (retros, reviews): `library/deliverables/system-retro/template.md`.

Read the chosen reference end-to-end. The new template will use its section order and conventions; the user fills in content.

### Step 3 — Scaffold step

Read `library/deliverables/_meta/template-authoring.md`, then generate `library/deliverables/{new-type}/template.md` using that standard.

The scaffolded template must include only sections that help a future agent decide, execute, compare, or verify. Do not copy a long skeleton into the skill or preserve empty slots for their own sake.

### Step 4 — Wire-up step

Identify and surface all upstream files that need updating to wire the new deliverable type into the system. Do NOT apply these wire-ups silently — surface each to the user with a yes/no, because each wire-up is itself a system patch:

| Wire-up | When needed | If needed, route through |
|---|---|---|
| Add letter (i+) to System Retro smart-routing options | If patches to this new deliverable will need their own routing letter (rare — most new deliverable types use existing route (c) "deliverables/{type}/template.md hard constraints") | `system/skills/system-improvement/SKILL.md` (since it's a template patch to `system-retro/template.md`) |
| Add deliverable to a campaign flow | If this deliverable belongs in one or more named flows | `system/skills/agentframe-structure/SKILL.md` (campaign-flow change, then `system-improvement` for the file patch if needed) |
| Add `current_phase` enum value to `library/process/campaign-frontmatter.md` | Only if this deliverable triggers a brand-new campaign phase (very rare; almost always slots into existing phases) | `system/skills/system-improvement/SKILL.md` |
| Confirm `status` enum on the new deliverable matches the canonical vocabulary in `library/process/campaign-frontmatter.md` | Always (no-op when scaffold uses the default `drafting | locked | deferred` from Step 3) — surface only if the operator wants a different enum (e.g. adding `shipped` for a deliverable that publishes externally, or proposing a brand-new value) | `system/skills/system-improvement/SKILL.md` (any new value is a `library/process/campaign-frontmatter.md` schema change and must pass the prior-patch shape-failure check when prior history exists) |
| Add campaign-local export template convention at `workspace/campaigns/{slug}/exports/templates/{new-type}.{docx,pptx}` | If this deliverable exports to Word/PPT | Surface to user; they create optional templates (this skill does not author binary export templates) |

For each wire-up: ask user "do this now, or defer?" Defer is fine — the deliverable can exist without being wired into a campaign flow. Many deliverables are situational and shouldn't be in the default flow.

### Step 5 — Deferred-validation framing

The scaffolded template will be validated by its first 1-2 real uses, not by a synthetic dry-run. The protective work happens in two real-world places: the next time the scaffolded type is drafted in a campaign, and the next System Retro that touches that campaign.

**This step's job**: write the validation expectation, then stop and move to Step 6.

Write a one-paragraph **validation expectation**:

> **Validation expectation**: the first real use of `library/deliverables/{new-type}/template.md` in a campaign is the validation point. The campaign's deliverable should observably reflect the template's hard constraints + section structure. If the first draft from the new template produces output that visibly hits the constraints (e.g. a Hard Constraint of "audience named at the person level" produces drafts naming a specific person, not "marketers in general"), mark `validation_resolved: pass` in a paired `system_changes` row. If the first draft ignores the template's constraints (output looks the same as a bespoke one would have), the template has the wrong shape — surface to the user, redesign the shape, and append the redesigned scaffold via this skill.

**Common shape rescues** (apply if validation comes back FAIL after first real use):

- The template is too generic. Tighten Hard constraints with earned, specific rules.
- The template has too many soft "consider X" prompts and not enough hard rules. Convert soft prompts to hard constraints, or remove them.
- The template duplicates an existing type. Re-run Step 1 — the duplication check may have been too lenient.

**No subagent dispatch.** The Step 6 `system_changes` row carries `validation_pending: true`; the System Retro after the first real use flips it with a paired `validation_resolved` row.

**Optional human-in-the-loop dry-run (rare)**: if the operator wants extra confidence before committing the scaffold to a campaign-flow wire-up, the operator may manually draft a sample deliverable from the template themselves and judge whether it constrained their drafting. Operator-driven, opt-in, not a default gate.

### Step 6 — Log step

Append a `system_changes` row via `system/audit/writer.py` for the new deliverable scaffold:

```
change_type: deliverable_scaffolded
target_kind: deliverable_template
target_path: library/deliverables/{new-type}/template.md
summary: Scaffolded new deliverable type `{new-type}` from `{reference-type}`.
payload_json:
  duplication_check: {compared types + overlap + decision}
  wire_ups_applied: {list OR "None"}
  validation_expectation: {one paragraph from Step 5}
  validation_pending: true
  principle_for_future_self: {optional one paragraph}
```

After the first real use of this deliverable type, the System Retro for that campaign appends a paired `validation_resolved` `system_changes` row linked to this scaffold (see `system/skills/system-improvement/SKILL.md` Step 6 for the schema).

If the new deliverable was wired into a campaign flow or `campaign-frontmatter.md`, those wire-ups produce their own `process_patch` / `schema_change` `system_changes` rows via `system/skills/agentframe-structure/SKILL.md` and `system/skills/system-improvement/SKILL.md` — link them from this scaffold row's payload.

## What this skill does NOT do

- **Does not author the new template's content.** The skill provides the canonical shape (sections in order, conventions, format). The user fills in Purpose, POV specifics, Section descriptions, Hard constraints, Tone notes, Edge cases — that is the domain knowledge that earns the deliverable's place.
- **Does not author binary export templates.** If the deliverable exports to Word or PPT, the user owns any optional campaign template files at `workspace/campaigns/{slug}/exports/templates/{new-type}.{docx,pptx}`. This skill notes the wire-up; the user owns the visual design.
- **Does not own routing taxonomy.** Whether the new deliverable needs a new smart-routing letter in the System Retro template is a System Retro template patch — routes through `system/skills/system-improvement/SKILL.md`.
- **Does not own the deliverable's lifecycle in a campaign.** When the deliverable is drafted, who reviews it, when it locks — that lives in the template the user fills in (Lock criteria section). This skill installs the slot; the user fills the slot.
- **Does not decommission deliverable types.** Removing a deliverable type is bespoke under Builder; no skill earns its place yet. When the first removal-with-friction surfaces, `system/skills/deliverable-decommission/SKILL.md` lands alongside this one.

## Edge cases

- **The new deliverable is actually a phase, not a deliverable** (e.g. user says "let's add a discovery phase deliverable"): surface the category confusion. Phases are in named campaign flows under `library/process/campaign-flows/`; deliverables produced within phases are here. If the user wants a new phase, that is a Builder decision routed through `system/skills/agentframe-structure/SKILL.md`.
- **The new deliverable is a one-off for a single campaign**: refuse to scaffold. Templates earn their place from repeat use. *"This sounds like a one-off artifact for `[campaign-slug]`. Templates earn their place from 2+ uses. Draft this inline in the campaign for now; if it shows up again in a future campaign, scaffold then."*
- **User wants to scaffold from a template that doesn't exist yet**: ask which existing type is closest. Use that as the reference baseline. The skill cannot bootstrap a deliverable type from nothing — there must always be a reference shape.
- **Forker is scaffolding a deliverable for a marketing process that has zero overlap with this fork's existing types** (e.g. forker is in B2B SaaS, fork ships consumer-content templates): the locate-before-inventing check will return ~0% overlap on every comparison, which is the correct signal. Proceed; the new template anchors a new family within this fork's `library/deliverables/`. Their next deliverable type compares against the new entry too.

## How this skill is invoked

- **Operator-initiated**: "Let's add a `[type]` deliverable" → load this skill.
- **Forker first-touch**: forker reads [`README.md`](../../../README.md) (product orientation) and [`AGENTS.cmo.md`](../../../AGENTS.cmo.md), then comes here to add their own deliverable types matching their marketing process.
- **From `system/skills/system-improvement/SKILL.md`**: when a System Retro identifies a friction pattern that is "we have no template for this artifact and we keep producing it bespoke", system-improvement may surface "this looks like an earned new deliverable type — load `deliverable-scaffolding`".

## Forker note

If you are reading this as a fork operator setting up your own deliverable taxonomy: `library/deliverables/_meta/template-authoring.md` is the canonical shape. Keep sections only when they help a future agent decide, execute, compare, or verify.
