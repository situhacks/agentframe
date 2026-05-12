# Builder Backlog

> **Unresolved Builder-mode tasks only.** When CMO mode hits system friction, it appends here instead of swapping modes mid-campaign. **Resolved** items are **moved** to [`builder-backlog-completed.md`](builder-backlog-completed.md) so this file stays a short queue. When several items accumulate, run a focused Builder session and batch-address them.

## What appends here

Tasks that:
- **Are Builder-mode work.** Patches to `library/process/*.md`, `library/deliverables/*/template-vF.md`, `system/skills/*`, `AGENTS.cmo.md`, `AGENTS.builder.md`, `AGENTS.md`, `library/context/operator/*` (process/template/persona surfaces), `system/server/*`, `system/skills/export-assets/*`, `docs/superpowers/specs/*`. Anything that lives outside `workspace/` and changes how the system itself behaves.
- **Are surfaced by CMO mode** — encountered while running a campaign, drafting a deliverable, running a retro, or doing state work. Builder-mode-internal work (e.g. while doing a Cluster execution, you find a downstream task) goes directly into the cluster execution flow or a `system_changes` row in `system/audit/agentframe.db`, not here.
- **Are not yet actioned.** Append new entries here. When an entry is resolved, **cut the full YAML block** from this file and **append** it to [`builder-backlog-completed.md`](builder-backlog-completed.md) under the same date heading (add a `## {YYYY-MM-DD}` section if missing).

## What does NOT append here

- **Within-campaign deliverable feedback** — that goes into the v1→vF diff (Loop 2 evolution signal) for the deliverable.
- **Within-campaign system-level feedback** — that goes into `workspace/campaigns/{slug}/feedback-log.md` (Loop 1 within-campaign feedback signal).
- **Patch trail for shipped fixes** — deliverable-template patches keep their per-target `library/deliverables/{type}/evolution.md`; non-deliverable system patches log to `system/audit/agentframe.db` `system_changes`. The Builder backlog is for tasks **before** they become patches.
- **Mode-swap or daily-checkin events** — those go into [`system/logs/system-events.md`](logs/system-events.md).
- **Phase overrides / deferrals / retro-skips** — those go into the campaign's `activity.md`.

## Entry format

```yaml
- id: BB-{YYYY-MM-DD}-{NN}                     # year-month-day + sequence; sequence is 01, 02, ...
  surfaced_at: {ISO-8601 datetime}
  surfaced_by: {agent | operator}              # who noticed it
  surfaced_in_mode: cmo                        # almost always cmo (Builder-mode work surfacing more Builder work goes elsewhere)
  surfaced_during: |                           # one line — what was happening when this came up
    {e.g. "drafting agent-architecture-pov post-3 carousel-spec"}
  one_line: |                                  # 1-2 sentences — the task, in operator-grade prose
    {e.g. "Carousel-spec deliverable conflates content + design lock states; needs split or status simplification."}
  affected_surfaces:                           # files/areas the patch will likely touch
    - library/deliverables/carousel-spec/template-vF.md
    - library/process/campaign-frontmatter.md
  estimated_effort: {trivial | small | medium | large}
  priority: {low | medium | high | blocker}    # blocker = next campaign cannot proceed without this
  earned_by: |                                 # the per-target evolution / feedback-log entry that made this earn its place
    {e.g. "agent-architecture-pov post-5 sat at content_locked_design_pending for weeks; agent treated unrecognized state as valid"}
  related_entries: []                          # other BB-* ids this consolidates with (filled at Builder-mode session time)
  resolved: false                              # flips to true when moved to builder-backlog-completed.md
  resolved_at: null
  resolved_by: null                            # operator | agent
  resolved_in: null                            # link to the cluster/PR/commit/spec that addressed it
  resolution_note: null                        # one-line — what shipped, or "deferred — {reason}"
```

The `id` doubles as a stable handle for cross-references (the System Retro Phase 5 backlog-check counts unactioned rows **in this file**, the operator can say "let's tackle BB-2026-04-23-03," etc.). Historical resolved items live in [`builder-backlog-completed.md`](builder-backlog-completed.md).

## Backlog-check trigger (CMO surface)

The System Retro template's Phase 5 step includes a backlog-check: count unactioned entries in **this file** (`resolved: false`). **If count ≥ 10,** the System Retro nudges the operator: "`builder-backlog.md` has reached {count} unactioned entries — recommend a Builder-mode session to address them as a batch." Threshold is 10 (chosen by operator 2026-04-23 — empirically 5 felt too eager for a 1-2 campaigns/quarter cadence; 10 represents enough accumulation to find consolidation overlap). The nudge is conversational; the operator decides whether to swap into Builder mode after the campaign closes or defer further.

## Rotation rule (yearly archive)

If [`builder-backlog-completed.md`](builder-backlog-completed.md) exceeds a comfortable size, optionally roll **oldest** resolved years into `system/builder-backlog/archive/builder-backlog-{YYYY}.md` at quarterly meta-retro time. **Unresolved** entries never roll; they stay in this file. Until volume warrants that, the completed file can grow as one append-only history.

## How CMO surfaces a backlog entry

When CMO mode encounters a Builder-mode task during normal work, it surfaces inline rather than swapping modes:

> "Heads up — `carousel-spec` template's `status: content_locked_design_pending` value isn't in the canonical campaign-frontmatter schema. That's Builder-mode work to reconcile. Append to builder-backlog?"

On operator confirmation (or operator-initiated capture: "add to builder backlog: ..."), append the entry per the format above. Then continue the CMO turn — do not swap modes.

---

## Active entries

## 2026-05-11

- id: BB-2026-05-11-02
  surfaced_at: 2026-05-11T20:50:00-07:00
  surfaced_by: agent
  surfaced_in_mode: builder
  surfaced_during: |
    planning the Open Design smart handoff from AgentFrame deliverables
  one_line: |
    Patch OD daemon design-system loading so a campaign DL written into `.od/projects/<id>/DESIGN.md` can be read as the active design system, eliminating the need to stage campaign design systems in the global picker.
  affected_surfaces:
    - system/skills/open-design/source/apps/daemon/src/design-systems.ts
    - system/skills/open-design/source/apps/daemon/src/project-routes.ts
  estimated_effort: medium
  priority: medium
  earned_by: |
    The current bundled OD loader scans only `source/design-systems/<id>/DESIGN.md`, so AgentFrame must temporarily write `agentframe-<campaign-slug>/DESIGN.md` into the global design-system catalog to make the campaign DL selectable for staged projects.
  related_entries: []
  resolved: false
  resolved_at: null
  resolved_by: null
  resolved_in: null
  resolution_note: null

- id: BB-2026-05-11-01
  surfaced_at: 2026-05-11T14:30:00-07:00
  surfaced_by: agent
  surfaced_in_mode: builder
  surfaced_during: |
    auditing the Open Design vendor integration and mode audit trail
  one_line: |
    Add a mode-swap wrapper command/script that atomically performs `Copy-Item AGENTS.{builder|cmo}.md AGENTS.md -Force` and appends the matching `mode_swap` row through `system/audit/writer.py`, so swap logging is enforceable rather than manual.
  affected_surfaces:
    - AGENTS.builder.md
    - AGENTS.cmo.md
    - system/audit/writer.py
    - system/builder-backlog.md
  estimated_effort: small
  priority: high
  earned_by: |
    Open Design integration and adjacent Builder patches ran while AGENTS.md still contained CMO content, and no historical `mode_swap` rows existed before corrective row 147; current manual swap flow is too easy to execute without audit logging.
  related_entries: []
  resolved: false
  resolved_at: null
  resolved_by: null
  resolved_in: null
  resolution_note: null

## 2026-05-07

- id: BB-2026-05-07-01
  surfaced_at: 2026-05-07T14:27:00-07:00
  surfaced_by: operator
  surfaced_in_mode: builder
  surfaced_during: |
    planning the UC1 AgentFrame remap and Business Frame review/iteration flow
  one_line: |
    Add a canonical per-deliverable `feedback/` folder convention for externally collected review inputs, meeting transcripts, and manager/service-line notes so revision evidence is not lost to chat context windows.
  affected_surfaces:
    - library/deliverables/*/template-vF.md
    - library/process/campaign-flows/*.md
    - library/process/campaign-frontmatter.md
  estimated_effort: medium
  priority: high
  earned_by: |
    UC1 planning exposed that current review state tracks `review: pending|complete|waived`, but does not define where manager feedback, service-line comments, or transcript-derived revision inputs are saved before being applied to the next deliverable version. Operator explicitly wants collected feedback persisted somewhere durable rather than only living in chat context.
  related_entries: []
  resolved: false
  resolved_at: null
  resolved_by: null
  resolved_in: null
  resolution_note: null

## 2026-05-04

- id: BB-2026-05-04-03
  surfaced_at: 2026-05-04T22:00:53-07:00
  surfaced_by: operator
  surfaced_in_mode: builder
  surfaced_during: |
    high-value backlog closeout after adding the Reader-use contract
  one_line: |
    Run a later template bloat pass using the Reader-use contract as the filter: sections should exist only when a human or execution agent needs them to decide, approve, execute, compare, or reuse.
  affected_surfaces:
    - library/deliverables/*/template-vF.md
    - library/process/*.md
  estimated_effort: medium
  priority: medium
  earned_by: |
    Operator concern 2026-05-04 that many agent-produced files include audit/AI slop, one-line form-fill sections, duplicative context, and instructional leakage. System-change row 63 added the principle; this entry captures the later sweep rather than expanding the current high-value closeout.
  related_entries: []
  resolved: false
  resolved_at: null
  resolved_by: null
  resolved_in: null
  resolution_note: null
