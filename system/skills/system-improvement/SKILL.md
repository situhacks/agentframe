---
name: system-improvement
version: 0.1.0
description: |
  Patch a system file (deliverable template, voice.md, profile.md, positioning.md,
  or library/process/*.md) when friction is observed. Use during System Retro,
  Campaign Retro, or any turn where CMO/Builder is about to apply a patch to
  agent-facing rule content. Carries the cross-cutting earning rule, the
  prior-patch shape-failure check, the deferred-validation framing, and the SQLite audit append
  cadence baked in. Refuses to draft a patch that cannot cite a
  feedback-log.md line, a `system_changes` audit row, a v1→head
  deliverable diff, or a frozen historical log entry where the absence of the
  constraint caused an observable stray. Refuses to draft a patch on a topic
  with prior-patch history without first naming the shape failure that made
  the prior patch insufficient.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - AskUserQuestion
---

# System Improvement: Patch a System File Safely

You are about to modify a file that other agent turns will load. The cost of getting this wrong is silent — a rule that doesn't constrain looks identical to a rule that does, until the next stray ships in production.

This skill carries the discipline. Run every step in order. Refuse to proceed past any gate that fails.

## When to load this skill

Load when you are about to patch any of these target file classes:

| Target class | Examples | Owner mode |
|---|---|---|
| Deliverable template | `library/deliverables/{type}/template.md` | CMO (System Retro) or Builder |
| Voice system | `library/context/operator/voice/` (target the specific file: `anti-patterns.md` for rules, `voice-profile.md` for the prior, `pairs/` for examples) | CMO |
| Profile (operator-self) | `library/context/operator/profile.md` | CMO (rare; usually Second Brain sync, not pattern-scan) |
| Positioning (outward-stance) | `library/context/operator/positioning.md` | CMO |
| Process file | `library/process/project-frontmatter.md`, `library/process/flows/{flow}.md`, other `library/process/*.md` | Builder |
| Always-loaded persona | `AGENTS.cmo.md`, `AGENTS.builder.md` | Builder |

If you are about to edit anything else (a campaign deliverable, a daemon script, infra), this skill is not the right reference — it is scoped to system-level rule content.

If the change adds, renames, defaults, or retires a campaign flow; adds or retires a deliverable type; or moves ownership between flows/templates/process files/personas, start with `system/skills/agentframe-structure/SKILL.md`. Return here only for the ordinary file patch once the structural owner is clear.

## Architectural anchors (why this skill is shaped this way)

This skill operationalizes principles that already exist in the agent personas:

- **Cross-cutting earning rule** (mirrored to `AGENTS.cmo.md` cross-cutting discipline + `AGENTS.builder.md` cross-cutting discipline) — every constraint earns its place from observed strays.
- **Prior-patch shape check** (`AGENTS.builder.md` + `AGENTS.cmo.md`) — patches on a topic with prior-patch history require an explicit prior-patch shape-failure diagnosis before drafting; validation of the new patch is deferred to real-world stray-recurrence over the next 1-2 campaign cycles.
- **Constraint shape discipline** — when a rule does not constrain, redesign its shape; do not re-write the same shape with sharper words.
- **Architectural Truth #1** — skills are generic capabilities; AgentFrame Marketing-specific routing taxonomy lives in `library/deliverables/system-retro/template.md`, NOT in this skill.
- **SQLite audit log** — every live patch writes a `system_changes` row in `system/audit/agentframe.db`. Historical markdown logs are browse-only backfill sources. This skill's Step 6 carries the write path.

This skill does not own routing taxonomy, pattern-strength threshold, override scrutiny, or decisions about whether a given target file class should accept patches at all. Those live in the orchestrating template (System Retro / Campaign Retro) or in the agent persona. This skill is the patch-loop procedure only.

## The procedure

Six steps. Each gate is binary — pass to the next step, or refuse and surface why.

### Step 1 — Earn step (REFUSAL GATE)

Before any other work, verify the patch is earned.

**Required citation** (one of four classes): (a) a `feedback-log.md` entry (campaign-scoped, file path + line number); (b) a recent `system_changes` row in `system/audit/agentframe.db` for this target/topic; (c) a `v1→head` diff between deliverable snapshots (the diff IS the citation for deliverable-template patches — see Step 2); (d) a frozen historical log entry where the absence of this constraint caused an observable stray.

**Required statement**: name the LLM tendency the patch counters in one line. (e.g. "agent enumerated phrases as triggers", "agent over-asked instead of actioning", "agent collapsed marketing logic into a generic skill".)

**Refusal conditions**:

- Cannot cite either source → surface the absence: *"This patch has no earning evidence. The cross-cutting earning rule refuses unearned constraints. Either (a) downgrade to a process observation in System Retro Section 3, (b) defer until evidence accrues, or (c) cite a stray I missed."* Do not draft.
- Citation exists but the cited entry does not actually demonstrate the failure mode the patch counters → surface the mismatch and refuse. (Citing a different stray than the patch counters is unearned.)
- Citation is "I have a hunch this might happen" or "to be safe" → unearned. Refuse.

**Pass criterion**: a one-line LLM-tendency statement + a real source citation that demonstrates the named tendency.

### Step 2 — Read step

Load all of the following before drafting:

- The target file (full content, not skim).
- Recent `system_changes` rows for the target/topic in `system/audit/agentframe.db` (the live canonical patch log).
- The earning citation source. Three citation classes:
  - `feedback-log.md` line — read the cited line + 5 lines of context above and below.
  - `system_changes` row — read the cited row + any adjacent rows on the same target/topic if the prior-patch shape failure is unclear.
  - For deliverable-template patches: the v1→head diff for the deliverable that surfaced the friction. Read **both** `{name}-v1.md` AND `{name}-v{N}.md`, then read the section that changed. The diff IS the citation — what the operator changed is the literal evidence the template did not produce what the operator wanted.
- The relevant always-loaded persona section that governs this target class:
  - For deliverable templates / voice.md / profile.md / positioning.md / process files → `AGENTS.cmo.md` Agent-Design Principles.
  - For `AGENTS.cmo.md` / `AGENTS.builder.md` itself → `AGENTS.builder.md` Core Design Principles.
- Any prior patch to this same target file in the last 30 days (query recent `system_changes` rows for the target). If a prior patch exists, the prior-patch shape-failure check applies.

**Pass criterion**: full content loaded, citation context loaded, prior-patch history surveyed.

### Step 3 — Diff step

Compute the gap between observed friction (cited in Step 1) and current target content.

State the gap explicitly in this shape:

> **Friction observed**: [one sentence from the citation]
> **Current target says**: [quote the target file's current language on this topic, OR state explicitly "the target file does not address this"]
> **Gap**: [what the agent would need loaded to avoid the friction]

If "current target says" matches "friction observed" closely (i.e. the rule already exists and was not followed), the rule has the wrong shape, not the wrong wording. Do not draft a sharper-worded version of the existing rule. Surface to the user: *"A rule on this topic already exists at [location]. The fact that the cited stray happened anyway means the rule did not constrain. Adding a second rule in the same shape is the failure mode. Diagnose the shape failure first: describes desired state instead of failure mode / has more exceptions than rule / not loaded when needed / wrong file."*

**Pass criterion**: gap explicitly stated; if a prior rule on this topic exists, shape-failure diagnosis explicitly stated before proposing a patch shape.

### Step 4 — Propose step (USER DECISION GATE)

Draft the patch as before/after, with internal branching by target class:

- **Deliverable template patch**: name which section is affected (Sections / Hard constraints / Critique-before-draft prompts / Tone notes / Lock criteria / Edge cases / etc.).
- **Voice patch**: specify Writing Style Examples vs mechanical rules vs banned-words section.
- **Positioning patch (outward-stance)**: specify Narrative / Content Pillars / Audience / POV Stances / Angles / Current Quarter Goals. If Narrative or Audience (campaign-spanning structural anchors), surface that the patch reshapes every upcoming user-voiced or strategic deliverable and ask for extra confirmation.
- **Profile patch (operator-self)**: rare-cadence — Identity / Primary source of truth / Active Projects. Profile patches usually fire from Second Brain sync events or operator-stated identity changes, NOT from campaign pattern-scan. If a pattern-scan surfaces a profile.md candidate, surface explicitly: *"Profile is the slowest-cadence operator file. This patch is firing from pattern-scan rather than a sync event — confirm this is an actual identity change vs an outward-stance refinement (which would route to positioning.md instead)."*
- **Process file patch**: flag scope — flow sequencing, shared process procedure, override-handling, schema/frontmatter behavior, or deliverable-existence check. If the patch changes the selected/default flow model, start with `agentframe-structure`.
- **Always-loaded persona patch (`AGENTS.cmo.md` / `AGENTS.builder.md`)**: flag explicitly that this is paid for in tokens on every turn forever (Architectural Truth #2 / Lazy Loading is the law). Justify why this rule must fire on every turn vs lazy-loading.

Surface the proposal to the user with this exact shape:

> **Proposed patch to**: [file path]
> **Section**: [section name]
> **LLM tendency it counters**: [one line, from Step 1]
> **Earning citation**: [source + line number]
> **Before**:
> [quote current text, or "(section does not exist)"]
> **After**:
> [proposed new text]
> **Shape**: [for behavioral principles only — confirm Karpathy LLM-tendency / Counter / Self-check shape]
>
> Approve as-is / approve with edits / reject / convert to one-off (no patch)?

Wait for explicit user decision. Do not apply without approval.

### Step 5 — Deferred-validation framing

Validation work is split across the procedural check that already happened in Step 3 (prior-patch shape-failure diagnosis when the topic has prior-patch history) and the deferred check that happens in real campaign work over the next 1-2 cycles.

**This step's job**: name what re-encounter would prove the patch worked, and what re-encounter would prove it failed. Then move to Step 6 and apply.

Write a one-paragraph **validation expectation** in this shape:

> **Validation expectation**: the next System Retro that surfaces a stray of the same family as `[earning citation]` is the validation point. If the agent's behavior on that stray family demonstrably matches the patched rule (the patched constraint fired), mark `validation_resolved: pass`. If the same stray family recurs unchanged (the patched constraint did not fire), mark `validation_resolved: fail`, redesign the shape (do NOT write a sharper-worded version of the same shape), and append the redesigned patch via this skill.

**Common shape rescues** (redesign options when validation comes back FAIL in a future cycle):

- Move the rule from a lazy-loaded file to an always-loaded file (or vice versa).
- Reshape from "describe desired state" → "name failure mode + counter + self-check" (Karpathy form).
- Move from agent persona → deliverable template (rule needs to load only when the deliverable is being drafted).
- Replace prose enumeration with a structural change (state-anchored trigger instead of phrase-list).

**No subagent dispatch.** The `system_changes` row in Step 6 carries `validation_pending: true`; a future System Retro flips it via a paired `validation_resolved` `system_changes` row.

**Optional human-in-the-loop dry-run (rare)**: for patches the operator subjectively flags as high-stakes (e.g. always-loaded persona changes that ship to all forkers, or rules where a wrong shape would be expensive to roll back), the operator may request a real-conversation dry-run — *operator opens chat, runs the original stray scenario manually against the agent loaded with the patched file, observes whether the constraint fires*. This is the artifact-smoke-test version of validation: human runs, human observes, human judges. Optional, opt-in, and operator-driven; not a default gate.

### Step 6 — Apply + log step

After Step 5's validation expectation is written:

1. **Apply** the patch to the target file using Edit tool (preserve all whitespace and formatting).
2. **Append** a `system_changes` row via [`system/audit/writer.py`](../../audit/writer.py) (the live canonical patch log). Use:
   - `change_type`: the patch class (`template_patch`, `context_edit`, `principle_refinement`, `process_patch`, `schema_change`, `skill_patch`, `validation_resolved`)
   - `target_kind`: the target class (`deliverable_template`, `operator_context`, `process_file`, `persona_file`, `skill_file`)
   - `target_path`: repo-relative path to the patched file
   - `reason` / `summary`: the human-readable why
   - `payload_json`: the structured fields below

   Payload shape:

```
{
  "section_patched": "...",
  "llm_tendency_countered": "...",
  "shape": "...",
  "before": "...",
  "after": "...",
  "earning_citation": "...",
  "prior_patch_shape_failure": "...",
  "validation_pending": true,
  "validation_expectation": "...",
  "principle_for_future_self": "..."
}
```

   Later, when a System Retro re-encounters the stray family named in `validation_expectation`, append a paired `system_changes` row with `change_type: validation_resolved` and a payload linking back to the original patch row:

```
{
  "original_change_id": "...",
  "resolution": "pass | fail | pass-by-absence",
  "evidence": "...",
  "next_action": "..."
}
```

3. **Mode-sync surfacing.** If the target file is `AGENTS.cmo.md` or `AGENTS.builder.md`, surface the mode-sync question: *"This patches an always-loaded persona. The mirror file (`AGENTS.md`) needs a sync via `Copy-Item {AGENTS.cmo.md | AGENTS.builder.md} AGENTS.md -Force` to take effect. Want me to run that now, or are you mid-mode-execution and want to defer the sync?"* If the sync changes system behavior, append a `system_changes` row via `system/audit/writer.py`.

## What this skill does NOT do

- **Does not own routing taxonomy.** Smart-routing options (a/b/c/d/e/f/g/h) are AgentFrame Marketing product taxonomy and live in `library/deliverables/system-retro/template.md`. The orchestrating template decides where a patch routes; this skill executes the patch.
- **Does not own pattern-strength threshold.** "3+ instances = pattern" is AgentFrame Marketing cadence and lives in System Retro template Section 1. This skill assumes the orchestrator already cleared the pattern bar.
- **Does not own override scrutiny.** Section 0 of the System Retro template runs first; this skill is invoked per-patch after that scrutiny has run.
- **Does not own deliverable creation or removal.** Adding a new deliverable type is `system/skills/deliverable-scaffolding/SKILL.md`. Removing one is currently bespoke under Builder (no skill earned its place yet).
- **Does not own structural flow edits.** Editing a named campaign flow is in scope only as a process_patch after `agentframe-structure` has established the structural owner. The deeper question "should this flow/phase exist at all" is Builder judgment carried by `agentframe-structure`, not this patch-loop procedure.
- **Does not write the deliverable content itself.** This skill patches templates (the constraints layer), not deliverables (the output layer). Deliverable production is the agent reading template + context and writing markdown natively, no skill in between.

## Edge cases

- **Patch is to this skill itself (`system/skills/system-improvement/SKILL.md`)**: run the procedure on the skill file. Yes, it can patch itself. The earning citation is what keeps recursion safe.
- **Topic with prior-patch history but you can't find the prior shape failure**: stop. The procedural check in Step 3 exists exactly because writing a sharper-worded version of an unknown-shape-failure is the failure mode. Either find the prior patch and diagnose it from `system_changes` rows or relevant frozen historical logs, or surface to the user that this patch should not be drafted until the prior shape failure is named.
- **Multiple patches proposed in the same turn** (System Retro produces 3+ patterns): run the procedure per patch, sequentially. The deferred-validation framing means each patch ships with its own validation expectation — multiple patches on the same target file in the same turn share a validation window (next 1-2 cycles) but are tracked individually.
- **User wants to skip the prior-patch shape-failure check** ("just apply it, I know the prior one"): the topic has prior-patch history, the procedural check fires. Surface: *"If you can name the shape failure of the prior patch in one sentence, that's the check passing — paste it into the `system_changes.payload_json.prior_patch_shape_failure` field and I'll continue. The check is not a delay; it's the claim that the new patch comes from a different shape family."*
- **Citation source is gitignored** (e.g. `feedback-log.md` in a private campaign): cite the path + line number anyway in the `system_changes` row. Future operators reading the audit history will see "citation lives in [private path]" — that is the correct behavior.
- **Validation cycle never recurs the stray** (3+ campaigns pass without the stray family appearing in any retro): this is informational, not failure. Either the patched constraint is firing silently (pass) or the stray family was situational and won't recur (no signal). Surface in the System Retro that names the milestone: *"Patch {system_changes link or id} has been live for {N} campaigns without re-encountering the stray family. Marking `validation_resolved: pass-by-absence` with a note that the signal is silence, not observed firing."*

## How this skill is invoked

- **From System Retro template** (Section 2 routing): orchestrator says "load `system/skills/system-improvement/SKILL.md` for this patch".
- **From Campaign Retro template**: same delegation pattern where the retro overlaps system-improvement work (override scrutiny + SQLite audit appends).
- **Ad-hoc by Builder or CMO**: when an agent turn surfaces a stray and the operator says "patch that" outside a retro window. Same procedure runs.
- **From `system/skills/deliverable-scaffolding/SKILL.md`**: when scaffolding a new deliverable wires up routing in `system-retro/template.md` or adds a `current_phase` enum value, that wire-up is itself a patch and goes through this skill.

## Forker note

If you are reading this as a fork operator: this skill's discipline is the meaningful part. The AgentFrame Marketing-specific bits (which files are templates, which routing letters exist, what `feedback-log.md` looks like) live in the orchestrating templates, not here. Swap your own deliverable types and your own retro template; the earn → read → diff → propose → defer-validation → apply → log loop ports unchanged.
