# Builder backlog — completed

Append-only archive of resolved `BB-*` items. IDs stay stable for cross-references.

**Active / unresolved** items live in [`builder-backlog.md`](builder-backlog.md). When an item resolves, move its full YAML block from the active file to the end of this file (same date subsection if present; otherwise add a dated `##` heading).

---

## 2026-04-23

<!-- Seed entries from the state-machine inventory audit (Cluster B § B.2). Most are resolved inline by the same Cluster B execution; a few are deferred and remain unresolved as initial backlog. -->

- id: BB-2026-04-23-01
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 1.A
  one_line: |
    `LIFECYCLE.status: paused` defined in campaign-frontmatter.md but never set, never read, never transitioned to/from. Drop or define.
  affected_surfaces:
    - library/process/campaign-frontmatter.md
  estimated_effort: trivial
  priority: low
  earned_by: |
    state-machine inventory 2026-04-23 Finding 1.A — zero usage in current campaign or completed campaigns
  related_entries: [BB-2026-04-23-02, BB-2026-04-23-03]
  resolved: true
  resolved_at: 2026-04-23T02:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: Cluster B § B.2 follow-on — campaign-frontmatter.md "Process changes" 2026-04-23 entry
  resolution_note: |
    Dropped from `LIFECYCLE.status` enum (now `active | complete | cancelled`).

- id: BB-2026-04-23-02
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 1.B + operator decision on `complete` vs `archived`
  one_line: |
    `archived` is folder-location, not work-state. Conflates two concerns. Retire and let folder-move be a side-effect of `complete | cancelled`.
  affected_surfaces:
    - library/process/campaign-frontmatter.md
    - library/deliverables/campaign-retro/template-vF.md
    - library/process/conversational-routines.md
    - library/process/typical-flow.md
  estimated_effort: small
  priority: medium
  earned_by: |
    operator clarification 2026-04-23 — "archived tag adds nothing; the marketing process dictates completion, not the folder location"
  related_entries: [BB-2026-04-23-01, BB-2026-04-23-03]
  resolved: true
  resolved_at: 2026-04-23T02:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: Cluster B § B.2 follow-on — campaign-frontmatter.md, campaign-retro/template-vF.md, conversational-routines.md, typical-flow.md all patched
  resolution_note: |
    `archived` retired from enum. Folder move is a side-effect of `LIFECYCLE.status: complete | cancelled`.

- id: BB-2026-04-23-03
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding from operator decision on `killed_in_review`
  one_line: |
    `killed_in_review` is too narrow (external-review-only). Solo operators can't represent operator-initiated cancellation. Generalize.
  affected_surfaces:
    - library/process/campaign-frontmatter.md
    - library/process/conversational-routines.md
    - library/process/typical-flow.md
    - library/deliverables/business-brief/template-vF.md
  estimated_effort: small
  priority: medium
  earned_by: |
    operator clarification 2026-04-23 — "cancelled + the why, agent will ask when we say campaign is cancelled"
  related_entries: [BB-2026-04-23-01, BB-2026-04-23-02]
  resolved: true
  resolved_at: 2026-04-23T02:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: |
    Cluster B § B.2 follow-on — `cancelled` replaces `killed_in_review`; `cancelled_at` + `cancelled_reason` fields added; `cancel campaign` conversational routine added to conversational-routines.md
  resolution_note: |
    `killed_in_review` retired in favor of broader `cancelled`. Agent asks for one-line reason via conversational routine.

- id: BB-2026-04-23-04
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 1.C
  one_line: |
    Phase 4→5 transition rule undocumented in typical-flow.md (`current_phase: 5-launch-and-learn` advance is implicit). Make it explicit.
  affected_surfaces:
    - library/process/typical-flow.md
  estimated_effort: trivial
  priority: medium
  earned_by: |
    state-machine inventory 2026-04-23 Finding 1.C
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T02:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: typical-flow.md Phase 4 — added "Tracker update at end of Phase 4" rule
  resolution_note: |
    Explicit rule added: `current_phase: 5-launch-and-learn` flips when first `deliverables.post-{n}.status: shipped`.

- id: BB-2026-04-23-05
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.A
  one_line: |
    `status: in_review` (on briefs) vs `review: in-review` (on tracker mirror) conflates two orthogonal axes (operator working state vs external coordination state).
  affected_surfaces:
    - library/process/campaign-frontmatter.md
    - library/deliverables/business-brief/template-vF.md
    - library/deliverables/campaign-brief/template-vF.md
    - library/process/typical-flow.md
  estimated_effort: small
  priority: medium
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.A
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: Cluster B § B.2 follow-on — `status: in_review` retired; `status: drafting + review: pending` covers the case cleanly
  resolution_note: |
    Orthogonality codified in campaign-frontmatter.md "Orthogonality" callout + typical-flow.md review-path-defaults.

- id: BB-2026-04-23-06
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.B
  one_line: |
    `status: shipped` defined on `carousel-spec/template-vF.md` enum but no transition into it (post is the shipping unit, not the carousel-spec). Drop.
  affected_surfaces:
    - library/deliverables/carousel-spec/template-vF.md
  estimated_effort: trivial
  priority: low
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.B
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: carousel-spec/template-vF.md — `shipped` removed from enum + explanatory note added
  resolution_note: |
    `shipped` dropped; carousel-spec status tops out at `locked` (the post is the shipping unit).

- id: BB-2026-04-23-07
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.C + operator decision on `content_locked_design_pending`
  one_line: |
    `content_locked_design_pending` is a one-deliverable special case with no Lock-Event Trigger semantics. Retire.
  affected_surfaces:
    - library/deliverables/carousel-spec/template-vF.md
    - library/process/campaign-frontmatter.md
    - workspace/campaigns/agent-architecture-pov/phase-4-production/posts/post-5/carousel-spec-vF.md
    - workspace/campaigns/agent-architecture-pov/campaign.md
  estimated_effort: small
  priority: medium
  earned_by: |
    operator clarification 2026-04-23 — "in practice the carousel design always ships with the copy; the intermediate state is edge-case noise"
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: Cluster B § B.2 follow-on — value retired across all surfaces; live agent-architecture-pov post-5 reverted to `status: drafting`
  resolution_note: |
    Retired. The same finding earned C.3 Schema-Drift-Check Discipline (the live state having drifted unnoticed for weeks was the original surfacing).

- id: BB-2026-04-23-08
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.D
  one_line: |
    Design-language sub-artifacts (directions-vF.md + design-language-vF.md) had no documented frontmatter convention.
  affected_surfaces:
    - library/deliverables/carousel-spec/template-vF.md
  estimated_effort: small
  priority: medium
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.D
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: carousel-spec/template-vF.md — Draft frontmatter convention now covers all three sub-artifacts
  resolution_note: |
    All three vF files now have documented frontmatter shape + transition rules + tracker-mirror semantics.

- id: BB-2026-04-23-09
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.E
  one_line: |
    `status: in_review` (underscore) vs `review: in-review` (hyphen) cosmetic naming inconsistency.
  affected_surfaces:
    - library/process/campaign-frontmatter.md
    - library/process/typical-flow.md
  estimated_effort: trivial
  priority: low
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.E
  related_entries: [BB-2026-04-23-05]
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: Cluster B § B.2 follow-on
  resolution_note: |
    Resolved as a side-effect of BB-2026-04-23-05 (in-review and in_review both retired in the enum collapse).

- id: BB-2026-04-23-10
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.F
  one_line: |
    `not_started` defined in campaign-tracker enum but never appears in per-deliverable file frontmatter (the file doesn't exist yet). Document the asymmetry.
  affected_surfaces:
    - library/deliverables/business-brief/template-vF.md
    - library/deliverables/campaign-brief/template-vF.md
    - library/deliverables/audience-strategy/template-vF.md
    - library/deliverables/messaging-architecture/template-vF.md
    - library/deliverables/image-prompt/template-vF.md
    - library/deliverables/carousel-spec/template-vF.md
    - library/deliverables/post-copy/template-vF.md
    - library/process/campaign-frontmatter.md
  estimated_effort: small
  priority: low
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.F
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:00:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: per-template tracker-only notes added across all 7 affected templates + canonical schema annotation
  resolution_note: |
    Asymmetry now explicit; tracker can hold `not_started`, file frontmatter never does.

- id: BB-2026-04-23-11
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 2.G
  one_line: |
    Spec-doc enum drift in `runtime/review-coordination.md` (still references v1 review enum: `pending | sent | in-review | returned | applied | waived`). Realign with v2 schema.
  affected_surfaces:
    - docs/superpowers/specs/2026-04-16-marketing-os-v3/runtime/review-coordination.md
  estimated_effort: trivial
  priority: low
  earned_by: |
    state-machine inventory 2026-04-23 Finding 2.G
  related_entries: [BB-2026-04-23-05]
  resolved: true
  resolved_at: 2026-04-23T03:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: schema-drift banner added to runtime/review-coordination.md pointing at canonical v2 schema
  resolution_note: |
    Banner added rather than full rewrite (this spec doc is frozen seed-content; live source of truth is library/process/campaign-frontmatter.md).

- id: BB-2026-04-23-12
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 3.A
  one_line: |
    Back-fill state-load surface — `back_filled: true` on a deliverable's frontmatter is invisible at routine state-load time; only surfaces at System Retro Section 0 Override scrutiny.
  affected_surfaces:
    - library/process/campaign-frontmatter.md
  estimated_effort: small
  priority: medium
  earned_by: |
    state-machine inventory 2026-04-23 Finding 3.A — `agent-architecture-pov` business-brief was back-filled 2026-04-19 with no surface at routine state-load time
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: campaign-frontmatter.md schema-drift-check section now includes step 7 (back-fill peek for locked deliverables)
  resolution_note: |
    Cheap frontmatter-only peek added to the always-on schema-drift-check.

- id: BB-2026-04-23-13
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 4.A
  one_line: |
    `system/skills/deliverable-scaffolding/SKILL.md` template skeleton uses pre-v2 schema (`status: drafting | in_review | locked` enum, no current_version, no version_history). Realign with v2.
  affected_surfaces:
    - system/skills/deliverable-scaffolding/SKILL.md
  estimated_effort: small
  priority: medium
  earned_by: |
    state-machine inventory 2026-04-23 Finding 4.A
  related_entries: []
  resolved: true
  resolved_at: 2026-04-23T03:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: deliverable-scaffolding SKILL.md Step 3 template skeleton patched + Step 4 wire-up table now includes status-enum check
  resolution_note: |
    Template skeleton now matches the v2 schema vocabulary and includes orthogonality + tracker-mirror reminders.

- id: BB-2026-04-23-14
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: agent
  surfaced_in_mode: cmo
  surfaced_during: |
    state-machine inventory audit (Cluster B § B.2) — Finding 4.B
  one_line: |
    Schema-drift check was framed as defense-in-depth (opt-in), causing live frontmatter (post-5 `content_locked_design_pending`) to drift unrecognized for weeks. Promote to Behavioral Principle.
  affected_surfaces:
    - AGENTS.cmo.md
    - AGENTS.md
    - library/process/campaign-frontmatter.md
  estimated_effort: small
  priority: high
  earned_by: |
    state-machine inventory 2026-04-23 Finding 4.B — `agent-architecture-pov` post-5 carousel-spec held `status: content_locked_design_pending` for ~3 weeks despite the value not being in the canonical schema
  related_entries: [BB-2026-04-23-07]
  resolved: true
  resolved_at: 2026-04-23T03:30:00+00:00
  resolved_by: agent (Builder mode, Cluster B execution)
  resolved_in: |
    AGENTS.cmo.md C.3 Schema-Drift-Check Discipline added; Lazy-Loading Rules state-question entry reframed; campaign-frontmatter.md schema-drift section renamed "Schema-drift check (the always-on guarantee)"; AGENTS.md resynced
  resolution_note: |
    Schema-drift check is now a precondition for any frontmatter-derived reasoning, not an opt-in defensive gate.

- id: BB-2026-04-23-15
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    Cluster B scope clarification 2026-04-23
  one_line: |
    Spec-drift audit — proactively diff every spec doc in docs/superpowers/specs/ against its live counterpart in library/system/AGENTS.* to find frozen-seed drift before it bites. Deferred 2026-04-23 pending more concrete definition.
  affected_surfaces:
    - docs/superpowers/specs/2026-04-16-marketing-os-v3/*
  estimated_effort: medium
  priority: low
  earned_by: |
    operator scope-clarification 2026-04-23 — "i dunno about spec drift need more definition"
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction: old/deferred spec-drift audit is not worth carrying as active backlog unless concrete drift recurs.

## 2026-05-03

- id: BB-2026-05-03-01
  surfaced_at: 2026-05-03T08:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    agent-architecture-pov Post 4 shipped back-fill and Post 5 carousel voice calibration
  one_line: |
    Voice mini-retro did not fire for Posts 1-4 because the trigger is shaped around `drafting` -> `locked` deliverable transitions with both `{name}-v1.md` and `{name}-vF.md` present, while shipped/back-filled post-copy work often moves directly to `shipped` or is reconciled from public LinkedIn copy. Diagnose whether publish/back-fill flows need their own voice-retro trigger, whether `voice-mini-retro.md` should include a shipped/back-fill fallback, or whether the publish coordination rule in `AGENTS.cmo.md` should explicitly load the mini-retro after final copy reconciliation.
  affected_surfaces:
    - AGENTS.cmo.md
    - AGENTS.md
    - library/process/voice-mini-retro.md
    - library/deliverables/post-copy/template-vF.md
    - library/process/campaign-frontmatter.md
  estimated_effort: small
  priority: medium
  earned_by: |
    Operator noticed after Post 4 back-fill: "my i think in our new kinda of cmo agents directive or one of hte campaign related process files that should be loadeded in - says to at each post compelteion step we should take learning sand update our voice.md file. lets do that now if we havent yuet.. from post 1-4, with a focus on the text i type within the linkedin post copy" followed by "why was vopice mini retro.md missed? was it not loaded in context? lets do a diagnosis and write a builder log todo so we can explore and fix it later".

    Diagnostic evidence from the live files:
      - `AGENTS.md` On-Demand Procedures only loads `library/process/voice-mini-retro.md` for "Any deliverable lock" when frontmatter `status: locked` is set, or the operator says a lock/finalize/ship phrase on a deliverable that has both `{name}-v1.md` and `{name}-vF.md`.
      - `library/process/voice-mini-retro.md` defines the canonical state trigger as `{name}-vF.md` transitioning `drafting` -> `locked`, then reads `{name}-v1.md` and `{name}-vF.md`.
      - Posts 1-3 were shipped/back-filled with public LinkedIn copy after the fact; Post 4 was created as a shipped back-fill (`copy-vF.md` status `shipped`) from the public LinkedIn post. That path had no live `drafting` -> `locked` moment for the agent to observe, and in Post 4's case no first AI `{name}-v1.md` snapshot to diff.

    Net pattern: the process exists, but the trigger shape is too narrow for real post-completion states. It protects normal lock events, not publish/back-fill reconciliation.
  proposed_shape_for_builder_session: |
    Keep the voice mini-retro narrow and state-anchored; do not turn it into a broad "after every post, rewrite voice.md" rule.

    Recommended investigation path:
      1. Separate three states: lock-time diff (`v1` -> locked `vF`), publish-time reconciliation (locked/drafting `vF` -> actually shipped copy), and back-fill (public shipped copy creates or replaces `vF` after the fact).
      2. For lock-time diff, keep the current `voice-mini-retro.md` trigger.
      3. For publish-time reconciliation, add a small trigger to the post publish coordination flow: after final shipped copy is reconciled into `copy-vF.md`, compare the immediately previous in-repo copy state to shipped copy and run the voice-only earning filter.
      4. For back-fill with no prior AI draft, do not pretend a mini-retro "passed." Instead, capture the shipped text as a voice calibration candidate and allow patching `voice.md` only when the operator explicitly asks or when 3+ shipped posts show the same pattern.
      5. Decide where this belongs: likely `library/process/voice-mini-retro.md` gets a "Publish/back-fill fallback" section, while `AGENTS.cmo.md` / `AGENTS.md` On-Demand Procedures and `post-copy/template-vF.md` publish coordination get one-line pointers.

    Validation expectation: the next time a post is published or back-filled, the agent should either (a) load the voice mini-retro fallback and surface a voice-learning candidate when the shipped copy differs materially in voice, or (b) explicitly state that no diffable AI draft exists and therefore the shipped text is calibration-only, not a mini-retro pass.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T22:00:53-07:00
  resolved_by: agent
  resolved_in: system_changes rows 65 + 67
  resolution_note: |
    Added publish/back-fill fallback to `voice-mini-retro.md` and wired post-copy publish coordination to run it after shipped-copy reconciliation. No-prior back-fills are calibration input, not mini-retro passes.

- id: BB-2026-05-03-02
  surfaced_at: 2026-05-03T11:51:00-07:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    agent-architecture-pov Post 5 carousel-spec net-new rewrite
  one_line: |
    Versioning self-check did not reliably fire before full-content rewrites of `carousel-spec-vF.md`: the agent previously changed the canonical `vF` through multiple conceptual versions while only updating `version_history`, without creating physical `carousel-spec-v1.md` / `v2.md` snapshots. This recurred despite an existing `AGENTS.md` rule saying Write-tool replacement of any `{name}-vF.md` must first snapshot the prior `vF`.
  affected_surfaces:
    - AGENTS.cmo.md
    - AGENTS.md
    - library/deliverables/carousel-spec/template-vF.md
    - library/deliverables/post-copy/template-vF.md
    - library/process/typical-flow.md
  estimated_effort: small-to-medium
  priority: high
  earned_by: |
    Operator correction during Post 5 carousel rewrite: "i want you to create a net new, so archive this current version as v1, then create th new one as vf ... i thought that was how you were supposed to do it? also do a diagnosis on why that process was not followed - how your are supposed to track versions by moving the old vf to a new v1/2/n and have the new copy as vf. then log for builder log so we can fix later"

    Diagnostic evidence:
      - `AGENTS.md` Memory and Continuity says deliverable `{name}-v{N}.md` files are immutable snapshots created automatically on every Write-tool replacement of `vF`; line-level rule says before a Write-tool replacement, snapshot the prior `vF` to `{name}-v{N+1}.md`.
      - The prior Post 5 carousel `vF` frontmatter had `current_version: 3` and three `version_history` entries, but the folder contained only `carousel-spec-vF.md` and no `carousel-spec-v1.md`/`v2.md` physical snapshots.
      - Root cause is enforcement shape, not absence of a rule: the rule is present, but it is framed as a self-check near the file-role table and does not force an operator-visible decision before full conceptual rewrites.
  proposed_shape_for_builder_session: |
    This likely consolidates with BB-2026-04-23-19. Do not create a separate duplicate patch if that entry already solves the same failure family.

    Builder should evaluate whether the fix needs to:
      1. Promote the `Versioning self-check` into a stronger CMO Behavioral Principle or edit-time decision protocol.
      2. Add deliverable-template-specific examples for carousel specs: slide wording tweak = in-place edit; net-new structure/name/story rewrite = snapshot old `vF` first, then replace `vF`.
      3. Require the agent to state "snapshot or surgical edit?" before applying any broad rewrite to a `*-vF.md` file.
      4. Clarify numbering when `version_history` exists but physical snapshots do not, so the agent does not invent inconsistent `current_version` semantics.

    Validation expectation: next time the operator requests a net-new rewrite of any deliverable `vF`, the agent snapshots the prior `vF` to the next physical `vN` file before replacing `vF`, or explicitly asks whether the change should be treated as surgical in-place vs a version bump.
  related_entries: [BB-2026-04-23-19]
  resolved: true
  resolved_at: 2026-05-04T22:00:53-07:00
  resolved_by: agent
  resolved_in: system_changes rows 64 + 65
  resolution_note: |
    Reshaped `AGENTS.cmo.md` into an edit-shape/versioning decision rule and added local post-copy/carousel examples. Ambiguous `*-vF.md` edits now require naming StrReplace vs Write consequences before editing.

- id: BB-2026-05-03-03
  surfaced_at: 2026-05-03T12:08:00-07:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    agent-architecture-pov Post 5 carousel-spec v3 rewrite
  one_line: |
    Carousel-spec template/process is ambiguous when the artifact also carries LinkedIn post copy. The template says `Voice loading: No` because carousel-spec is treated as a visual deliverable, but the live Post 5 carousel spec contains `Post Copy`, hook variants, CTA, and slide copy. This caused unclear ownership: hook variants were not labelled as post-copy openings vs Slide 1 title options, CTA sat outside the recommended copy, and Brandon-specific voice rules were easy to miss.
  affected_surfaces:
    - library/deliverables/carousel-spec/template-vF.md
    - library/deliverables/post-copy/template-vF.md
    - AGENTS.cmo.md
    - AGENTS.md
  estimated_effort: small
  priority: medium
  earned_by: |
    Operator feedback during Post 5 carousel spec review: "Where do those hook variants live? Is that living within technically the recommended draft and the carousel content? Or is it like where is it supposed to be living? If these are variants for the first slide, then let's make it clear that that is. And let's group up the CTA under the recommended copy draft because the CTA is run under it." In the same turn, operator asked whether voice/humanizer had been applied because the draft used short impact-story lines like "Not perfect. Very useful."

    Diagnostic evidence:
      - `library/deliverables/carousel-spec/template-vF.md` currently says `Voice loading: No` and describes carousel-spec as structure + tokens + HTML, while Post 5's live `carousel-spec-vF.md` includes user-voiced LinkedIn post copy and CTA.
      - The live artifact's mixed ownership made the hook bank ambiguous and let voice-sensitive post copy sit inside a file type whose template says voice rules do not apply.
  proposed_shape_for_builder_session: |
    Decide whether carousel specs should continue to include LinkedIn post copy at all.

    Likely fixes:
      1. Prefer separating `copy-vF.md` from `carousel-spec-vF.md` earlier, so post copy loads `post-copy/template-vF.md` + `voice.md`, while carousel specs stay visual/content slide specs.
      2. If combined carousel+caption specs remain allowed, update `carousel-spec/template-vF.md` with explicit sections: `Recommended LinkedIn Copy` (hook/body/CTA), `Alternate Post-Copy Opening Hooks`, and `Slide 1 Title Options`, plus a conditional voice-loading note for user-voiced regions.
      3. Add a carousel visual-variety requirement: reuse the campaign design language, but do not make every slide a top-left headline clone.

    Validation expectation: the next carousel spec with accompanying LinkedIn copy should make it obvious where the post hook, CTA, slide title, and visual spec live, and the agent should load voice guidance for the post-copy region even if the broader carousel artifact is visual.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T22:00:53-07:00
  resolved_by: agent
  resolved_in: system_changes rows 65 + 66
  resolution_note: |
    Clarified reciprocal ownership: `copy-vF.md` owns the LinkedIn caption once it exists; `carousel-spec-vF.md` owns slide text, cover-title options, visual direction, slide jobs, and carousel-specific CTA placement. Voice loading remains conditional for user-facing carousel prose.

- id: BB-2026-04-26-02
  surfaced_at: 2026-04-26T21:35:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    iterating agent-architecture-pov Post 4 video/SCRIPT.md after operator gave top-down script feedback
  one_line: |
    Video script iteration lacks an explicit CMO discussion gate. The agent treated a long creative feedback note as authorization to directly rewrite `video/SCRIPT.md`, instead of first discussing alternatives, tradeoffs, and a recommended restructuring with the operator.
  affected_surfaces:
    - library/process/video-production.md
    - library/deliverables/video-spec/template-vF.md
    - AGENTS.cmo.md
    - AGENTS.md
  estimated_effort: small
  priority: medium
  earned_by: |
    Operator feedback on 2026-04-26 after second draft: "why are you just implementing it straight to the script without pushing back discussing alternatives and options with me?" Root cause appears to be that `AGENTS.md` has a generic "confirm, then draft" rule, but the video-production process describes when to use `SCRIPT.md` / `STORYBOARD.md` without defining a review protocol for long creative notes during script/storyboard iteration.
  related_entries: [BB-2026-04-26-01]
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction as old/completed enough; no active patch needed.

- id: BB-2026-04-23-16
  surfaced_at: 2026-04-23T01:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    Cluster B scope clarification 2026-04-23 — original "in-flight capture" Behavioral Principle from earlier scoping
  one_line: |
    In-flight capture Behavioral Principle — codify the discipline of capturing CMO-mode-surfaced Builder tasks into builder-backlog rather than swapping modes mid-campaign. Deferred pending evidence the capture surface itself is being underused.
  affected_surfaces:
    - AGENTS.cmo.md
  estimated_effort: small
  priority: low
  earned_by: |
    operator scope-clarification 2026-04-23 — "defer until further evidence or definition emerges"
  related_entries: [BB-2026-04-23-14]
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction as old/completed enough; no active patch needed.

- id: BB-2026-04-23-17
  surfaced_at: 2026-04-23T20:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    post-daemon-implementation handoff; operator swapping back to CMO to finish agent-architecture-pov post 3; queue next Builder session work
  one_line: |
    Daemon v1 follow-through — run a live shakedown (real system/daemon/.env + config.yaml, Gmail/Calendar + user_google_email + workspace MCP URL), run `py -3 system/daemon/run.py` without --simulate, then wire a weekday Windows Task Scheduler job; decide Discord digest policy (webhook on vs dry-run) and add optional repo hygiene (e.g. .gitignore rules for system/memory if generated files should not track).
  affected_surfaces:
    - system/daemon/*
    - system/memory/*
    - .gitignore
  estimated_effort: medium
  priority: medium
  earned_by: |
    operator 2026-04-23 — "append next steps for the next builder run" after daemon + digest + warm-start work landed
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction as old/completed enough; no active patch needed.

- id: BB-2026-04-23-18
  surfaced_at: 2026-04-23T21:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    agent-architecture-pov Post 3 pre-edit diff pass — comparing copy-vF claims to shipped MarketingOS daemon implementation
  one_line: |
    Daemon agents (researcher + pm) need a Hermes-style self-improvement loop on their own AGENT.md / prompts — currently only CMO-side templates patch via Loop 1/2 retros; the daemon's prompts don't evolve against what worked. Post 3 copy implicitly promises this with "the skill file itself gets patched when outputs start drifting," and the operator is staking that publicly. Apply the same retro-loop discipline (Hermes pattern Post 2 named) to system/daemon/agents/{researcher,pm}/AGENT.md so the daemon's own scaffolding sharpens with each run, not just the templates the CMO uses.
  affected_surfaces:
    - system/daemon/agents/researcher/AGENT.md
    - system/daemon/agents/pm/AGENT.md
    - system/daemon/lib/* (likely a new retro/self-patch module)
    - system/audit/marketingos.db (new event type? daemon_self_patch in system_changes, or extend daemon_runs payload)
    - library/process/* (if the daemon-side retro-loop discipline gets a process surface)
  estimated_effort: medium
  priority: medium
  earned_by: |
    Post 3 copy-vF.md (line: "the skill file itself gets patched when outputs start drifting, so 'trending AI topics' this month means something different than last month") + operator confirmation 2026-04-23 ("on the second part about hermes style skill improvement - yes heard loud and clear. that's on the todo")
  related_entries: [BB-2026-04-23-17]
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction as old/completed enough; no active patch needed.

- id: BB-2026-04-23-19
  surfaced_at: 2026-04-23T22:00:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    agent-architecture-pov Post 3 vF edit pass — operator noticed agent used Write (full-rewrite + auto-snapshot) for an edit that should have been multiple StrReplace calls (no snapshot, no current_version bump). Operator asked whether the agent was aware of the existing versioning practice; diagnostic walk found the rule exists in two places but the enforcement surface is too thin.
  one_line: |
    Versioning self-check is mechanically correct in the system (AGENTS.cmo.md line 229 + library/deliverables/post-copy/template-vF.md line 53) but operationally under-enforced — the agent picks Write or StrReplace by content-delta intuition rather than by the documented rule (Write=snapshot+bump, StrReplace=in-place no snapshot). Need a stronger operator-facing decision surface: an Edit-Tool Self-Check Behavioral Principle in AGENTS.cmo.md with a worked decision tree, the same decision tree mirrored into every deliverable template that has a vF/snapshot pattern, and a default behavior that the agent proposes BOTH the edit shape AND the tool choice (StrReplace vs Write) in the same turn whenever an edit is genuinely ambiguous. The cost of asking is one extra question per ambiguous edit; the cost of NOT asking is full-file Write churn (3+ file ops per edit + regenerated frontmatter + a snapshot the operator didn't ask for) plus snapshot pollution that makes the version_history harder to read.
  affected_surfaces:
    - AGENTS.cmo.md (Versioning self-check section near line 229 — promote from one-line self-check to Behavioral Principle with worked decision tree + ambiguous-case operator-question default)
    - AGENTS.md (resync from AGENTS.cmo.md per Cluster F Phase 3 open-source-default convention)
    - library/deliverables/post-copy/template-vF.md (Snapshot trigger section near line 53 — add the same decision tree, post-copy-specific examples)
    - library/deliverables/carousel-spec/template-vF.md (mirror — same vF/snapshot pattern)
    - library/deliverables/image-prompt/template-vF.md (mirror — same vF/snapshot pattern)
    - library/deliverables/business-brief/template-vF.md (mirror)
    - library/deliverables/campaign-brief/template-vF.md (mirror)
    - library/deliverables/audience-strategy/template-vF.md (mirror)
    - library/deliverables/messaging-architecture/template-vF.md (mirror)
    - library/deliverables/system-retro/template-vF.md (mirror — if applicable)
    - library/deliverables/campaign-retro/template-vF.md (mirror — if applicable)
    - possibly library/process/typical-flow.md (if the decision tree wants a process-level home rather than only persona+template)
  estimated_effort: small-to-medium
  priority: medium
  earned_by: |
    agent-architecture-pov Post 3 vF edit session 2026-04-23 — operator quote verbatim: "is it not faster to copy the file, rename then do the small inline edits afterwards? why did you basically rewrite? this will likely happen a few more times - can you check your cmo/instructions (where does this logic lives) because i thought you are aware of the versioning practice and to recommend to either make small edits in line on the vf, or create a new version and promote it - i didn't see that behaviour from you (you just asked if we want to update vf)". Specific failure modes the agent walked back through after the operator surfaced this:
      (1) During the v4→vF readability+humanizer pass, the agent used Write (snapshotted the entire prior body to copy-v4.md, regenerated the entire copy-vF.md including frontmatter and changelog) when the actual content delta was bounded enough for ~6-8 StrReplace calls (body paragraph swaps + bolded-spec block replacement + frontmatter humanizer_pass field flip + changelog append).
      (2) The operator-facing turn never surfaced the choice: the agent asked "Want me to apply this to copy-vF.md?" and on confirmation went straight to Write without proposing StrReplace as an option or naming the snapshot consequence.
      (3) Diagnostic shows the rule itself is correct and lives in AGENTS.cmo.md line 229 + library/deliverables/post-copy/template-vF.md line 53 — both rules say "Write triggers snapshot, StrReplace does not." Neither surface tells the agent HOW to choose between them, and neither carries an operator-facing "ask when ambiguous" default.
      (4) Operator instruction confirming this is genuine cross-campaign Builder-mode work: "we are trying to diagnose this and possibly add to builder todo for next run".
    Net pattern: rule is well-formed, enforcement surface is missing.
  proposed_shape_for_builder_session: |
    When this entry is picked up in a Builder-mode session, the patch likely takes this shape (not binding — Builder session decides the final form):

    1. Promote the AGENTS.cmo.md "Versioning self-check" one-liner to a named Behavioral Principle (sibling of "Builder-mode tasks surfaced from CMO mode capture into builder-backlog.md, not into a mode swap"). Inline a worked decision tree:

       - Inline edit (1-N localized swaps that don't change document shape) → StrReplace, no snapshot, current_version does NOT bump.
       - Full-content replacement OR shape-changing rewrite (re-architected sections, prose-to-list pivot, structural reordering) → Write with prior-vF snapshot to {name}-v{N+1}.md, bump current_version, append version_history entry.
       - Ambiguous → ask the operator in the same turn the edit is proposed: "This is ~N changes. Inline as StrReplace (no snapshot, current_version stays at F), or treat as a version bump (snapshot the current vF to {name}-v{N+1}.md and Write the new vF)?"

    2. Mirror the decision tree into every deliverable template that has a vF/snapshot pattern (see affected_surfaces). Each template should add the deliverable-specific examples — post-copy gets "typo fix = StrReplace; full-body rewrite = Write"; carousel-spec gets "single slide copy tweak = StrReplace; slide reorder = Write"; etc.

    3. Make "propose-the-tool-with-the-edit" a default-on behavior in CMO mode until the agent demonstrates reliably correct tool choice across N campaigns. Cost: one extra question per ambiguous edit. Benefit: zero unnecessary snapshots, version_history stays meaningful.

    4. Optional consolidation check: if this principle should ALSO live at library/process/typical-flow.md (process-level rather than persona+template-level), evaluate at Builder-session time whether the rule is genuinely cross-deliverable enough to warrant a process-level home. Risk of over-placing: same rule restated in 3+ surfaces is the kind of duplication this codebase deliberately avoids.

    5. After patching, run a test edit on a real campaign deliverable (or a sandbox copy) to confirm the agent now proposes the tool choice at edit time. If the test fires the new behavior reliably, mark resolved. If it doesn't, the patch went into a surface that doesn't load at the right moment — re-route to whatever surface the agent actually reads when about to edit a vF file.

    Out-of-scope for this entry (don't expand the patch surface mid-session):
      - Don't redesign the snapshot mechanism itself (Write-trigger semantics are correct; this is a surface/enforcement patch, not a mechanism patch).
      - Don't auto-detect "this should be StrReplace" via static analysis or pre-commit hook in V1 — the operator-facing question is the right surface for V1; automation can come if the question gets answered the same way 20+ times in a row.
      - Don't backfill historical version_history entries to remove prior over-snapshotted edits — the snapshots that exist are real prior shapes worth diffing against; the violation was unnecessary churn, not incorrect data.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T22:00:53-07:00
  resolved_by: agent
  resolved_in: system_changes row 64
  resolution_note: |
    Closed with BB-2026-05-03-02. The fix is not a full decision tree copied into every template; `AGENTS.cmo.md` carries the canonical edit-shape rule, with local examples only in the templates that failed in live work.

## 2026-04-26

- id: BB-2026-04-26-01
  surfaced_at: 2026-04-26T08:20:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    brainstorming agent-architecture-pov Post 4 video planning scratchpad
  one_line: |
    Deliverable-planning lazy-load path did not naturally produce a Cursor Plan Mode artifact for a long, multi-turn video planning session; the agent fell back to a custom `plan-scratchpad.md`. Diagnose whether `library/process/deliverable-planning.md` should better support durable campaign-local planning scratchpads or whether the CMO routing should trigger Plan Mode earlier.
  affected_surfaces:
    - library/process/deliverable-planning.md
    - AGENTS.cmo.md
    - library/deliverables/video-spec/template-vF.md
  estimated_effort: medium
  priority: medium
  earned_by: |
    Operator note during Post 4 video planning: "I know we didn't actually use the plan feature, we ended up using a custom plan scratchpad, but that's something we'll log over to the builder log to diagnose later why didn't the lazy loaded plan feature work."
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Builder backlog cleanup 2026-05-04
  resolution_note: |
    Closed per operator direction as old/completed enough; no active patch needed.

## 2026-05-04

- id: BB-2026-05-04-01
  surfaced_at: 2026-05-04T20:23:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    diagnosing why a simple Outlook email request used local Outlook COM automation instead of the already-built Edge + Playwright browser fallback workflow
  one_line: |
    Agent tool-routing context is too thin: the agent should know the approved integration ladder before choosing execution surfaces — use provided/approved APIs, MCPs, connectors, or CLIs first; when unavailable or not worth wiring, route to the documented browser fallback workflows. The Outlook browser workflow existed, but it was not in always-loaded or naturally loaded context, so the agent chose an ad hoc COM path.
  affected_surfaces:
    - AGENTS.cmo.md
    - AGENTS.builder.md
    - AGENTS.md
    - library/process/browser-fallback.md
    - system/browser/README.md
    - system/browser/workflows/*/recipe.md
  estimated_effort: small
  priority: high
  earned_by: |
    2026-05-04 live failure: operator asked the agent to send a simple Outlook email to Kaustubh; agent attempted PowerShell Outlook COM recipient resolution instead of loading `library/process/browser-fallback.md` and `system/browser/workflows/outlook-draft-email/recipe.md`. Operator diagnosis: the system should always use approved APIs/MCPs/connectors first, then browser fallback when no approved native path exists; that routing context was not present in loaded agent context.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: operator
  resolved_in: Browser routing cleanup 2026-05-04
  resolution_note: |
    Closed as "watch for recurrence." Browser fallback docs/workflow routing were cleaned up; if the agent chooses an unapproved path again, reopen with fresh evidence.

- id: BB-2026-05-04-02
  surfaced_at: 2026-05-04T20:39:00+00:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    first Workfront browser-fallback workflow demo for creating a Post 5 campaign request
  one_line: |
    Browser demo recorder only attaches listeners to the first page in the browser context, so workflows that navigate through Experience Cloud into Workfront in a second tab/window lose the important click/input events. Recorder should track new pages/tabs in the owned context, install listeners on each active page, and record enough page URL evidence to distinguish the URL the operator actually used from the agent-provided start URL.
  affected_surfaces:
    - system/browser/src/demo-recorder.js
    - system/browser/src/demo-recorder-lifecycle.js
    - system/browser/src/demo-compactor.js
    - system/browser/tests/*
    - library/process/browser-fallback.md
  estimated_effort: medium
  priority: high
  earned_by: |
    2026-05-04 Workfront demo: recorder run `workfront-campaign-request-post-5` captured Adobe sign-in, Experience Cloud home, and profile-menu clicks only. Live Work Browser observation after the run showed a second Workfront page at `deloittedigitalus.my.workfront.com` with a submitted request (`AgentFrame Marketing Post 5`) and custom form values, proving the operator completed the workflow but the recorder missed the form field interactions. Code inspection showed `demo-recorder.js` selects `context.pages()[0]` and installs listeners only on that page.
  related_entries: [BB-2026-05-04-01]
  resolved: true
  resolved_at: 2026-05-04T21:10:00-07:00
  resolved_by: agent
  resolved_in: system/browser/src/demo-recorder.js + system/browser/tests/demo-recorder.test.js
  resolution_note: |
    Recorder now installs listeners on all existing pages in the owned browser context, subscribes to future `page` events, avoids duplicate installation, and observes the last active recorded page at stop time.

## 2026-05-09

- id: BB-2026-05-09-01
  surfaced_at: 2026-05-09T16:08:00-07:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    starting a new campaign after swapping from Builder to CMO mode
  one_line: |
    New-campaign intake should make the live workspace-context check explicit and cite candidate-recommendation provenance before asking the operator to choose a direction, including a clear fallback when connector tools are unavailable.
  affected_surfaces:
    - library/process/typical-flow.md
    - AGENTS.cmo.md
  estimated_effort: small
  priority: medium
  earned_by: |
    Operator asked why CMO did not offer to check Composio/workspace signals first, why it went straight to campaign ideas, and where the recommended campaign directions came from.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-09T18:38:00-07:00
  resolved_by: agent
  resolved_in: |
    library/process/typical-flow.md Phase 1 + AGENTS.builder.md lazy-loaded workflow ownership principle
  resolution_note: |
    Kept the new-campaign workspace/provenance gate in the lazy-loaded Phase 1 process file and removed the duplicated CMO persona prose. Added a Builder principle that AGENTS files route/guard while process and template files own workflow steps.

- id: BB-2026-05-09-02
  surfaced_at: 2026-05-09T17:04:00-07:00
  surfaced_by: operator
  surfaced_in_mode: cmo
  surfaced_during: |
    running Gemini Deep Research collaborative planning for the agents-move-work-up campaign
  one_line: |
    Harden the Gemini Deep Research API path for detailed Phase 1: plan-first runs, full research runs, response-shape extraction, usage/cost capture, artifact saving, and clear campaign-state updates.
  affected_surfaces:
    - library/process/typical-flow.md
    - library/deliverables/research-artifact/template-vF.md
    - system/exports or system/runtime Gemini runner if one is added
  estimated_effort: medium
  priority: high
  earned_by: |
    First live Interactions API run completed, but the plan appeared under `outputs[1].text` rather than the documented `steps[-1].content[0].text`. Operator explicitly asked to harden this new process after completing the campaign research flow.
  related_entries:
    - BB-2026-05-09-01
  resolved: true
  resolved_at: 2026-05-09T18:25:00-07:00
  resolved_by: agent
  resolved_in: system/research/gemini_deep_research.py
  resolution_note: |
    Added a deterministic Gemini Deep Research runner with schema-tolerant extraction, raw JSON preservation, media extraction, focused tests, and short Phase 1/template instructions.

## 2026-05-10

- id: BB-2026-05-10-01
  surfaced_at: 2026-05-10T12:58:00-07:00
  surfaced_by: agent
  surfaced_in_mode: builder
  surfaced_during: |
    system folder inventory and cleanup pass
  one_line: |
    Preview-server hub currently surfaces intermediate render artifacts (history folders, working assets, scratch outputs) and watch globs miss some newly authored HTML locations. Add explicit exclusion controls and broaden watch coverage so the dashboard defaults to final-facing previews.
  affected_surfaces:
    - system/server/config.yaml
    - system/server/lib/hub.py
    - system/server/README.md
    - system/server/static/demo/
  estimated_effort: medium
  priority: high
  earned_by: |
    Operator reported the preview hub becoming noisy with video-preview screenshots, short clips, and intermediate images. Current discovery rules index all media under phase folders without exclusions, and LiveReload watch globs are narrower than discovery, so newly created HTML may require manual refresh behavior that feels inconsistent.
  related_entries: []
  resolved: true
  resolved_at: 2026-05-10T13:24:00-07:00
  resolved_by: agent
  resolved_in: preview-server-noise-and-trigger execution
  resolution_note: |
    Added config-driven hub exclusion + `.preview-hide` marker support, broadened watch globs, and added the preview-offer process trigger path.

