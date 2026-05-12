# Builder Backlog

> **Unresolved Builder-mode tasks only.** When CMO mode hits system friction, it appends here instead of swapping modes mid-campaign. **Resolved** items are **moved** to [`builder-backlog-completed.md`](builder-backlog-completed.md) so this file stays a short queue. When several items accumulate, run a focused Builder session and batch-address them.

## What appends here

Tasks that:
- **Are Builder-mode work.** Patches to `library/process/*.md`, `library/deliverables/*/template-vF.md`, `system/skills/*`, `AGENTS.cmo.md`, `AGENTS.builder.md`, `AGENTS.md`, `library/context/operator/*` (process/template/persona surfaces), `system/server/*`, `docs/superpowers/specs/*`. Anything that lives outside `workspace/` and changes how the system itself behaves.
- **Are not yet actioned.** Append new entries here. When an entry is resolved, **cut the full YAML block** from this file and **append** it to [`builder-backlog-completed.md`](builder-backlog-completed.md) under the same date heading (add a `## {YYYY-MM-DD}` section if missing).

## What does NOT append here

- **Within-campaign deliverable or system-level feedback** — that goes into `workspace/campaigns/{slug}/feedback-log.md` (Loop 1 within-campaign feedback signal).
- **Patch trail for shipped fixes** — deliverable-template patches keep their per-target `library/deliverables/{type}/evolution.md`; non-deliverable system patches log to `system/audit/agentframe.db` `system_changes`. The Builder backlog is for tasks **before** they become patches.
- **Phase overrides / deferrals / retro-skips** — those go into the campaign's `activity.md`.

## Entry format

```yaml
- id: BB-{YYYY-MM-DD}-{NN}                      # year-month-day + sequence; 01, 02, …
  surfaced_at: null                             # ISO-8601 datetime
  surfaced_by: null                             # agent | operator
  surfaced_in_mode: cmo                         # almost always cmo; Builder-sourced backlog elsewhere
  surfaced_during: ""                           # one line — context when this surfaced (use `|` for multi-line)
  one_line: ""                                  # 1–2 sentences; operator-grade task statement (use `|` if longer)
  affected_surfaces: []                         # repo paths the patch will likely touch
  estimated_effort: null                        # trivial | small | medium | large
  priority: null                                # low | medium | high | blocker (blocker = next campaign blocked)
  earned_by: ""                                 # evolution / feedback-log evidence (use `|` if longer)
  related_entries: []                           # other BB-* ids this consolidates with
  resolved: false                               # set true when moved to builder-backlog-completed.md
  resolved_at: null
  resolved_by: null                             # operator | agent
  resolved_in: null                             # cluster / PR / commit / spec that addressed it
  resolution_note: null                         # what shipped, or deferred — {reason}
```

The `id` doubles as a stable handle for cross-references (the System Retro Phase 5 backlog-check counts unactioned rows **in this file**; the operator can refer to a row by `BB-*` id). Historical resolved items live in [`builder-backlog-completed.md`](builder-backlog-completed.md).

## Backlog-check trigger (CMO surface)

The System Retro template's Phase 5 step includes a backlog-check: count unactioned entries in **this file** (`resolved: false`). **If count ≥ 10,** the System Retro nudges the operator: "`builder-backlog.md` has reached {count} unactioned entries — recommend a Builder-mode session to address them as a batch." Threshold is 10 (chosen by operator 2026-04-23 — empirically 5 felt too eager for a 1-2 campaigns/quarter cadence; 10 represents enough accumulation to find consolidation overlap). The nudge is conversational; the operator decides whether to swap into Builder mode after the campaign closes or defer further.

## Rotation rule (yearly archive)

If [`builder-backlog-completed.md`](builder-backlog-completed.md) exceeds a comfortable size, optionally roll **oldest** resolved years into `system/builder-backlog/archive/builder-backlog-{YYYY}.md` at quarterly meta-retro time. **Unresolved** entries never roll; they stay in this file. Until volume warrants that, the completed file can grow as one append-only history.

## How CMO surfaces a backlog entry

When CMO mode encounters a Builder-mode task during normal work, it surfaces inline rather than swapping modes (short paraphrase of the friction, then ask whether to append a `BB-*` row here).

On operator confirmation (or operator-initiated capture: "add to builder backlog: ..."), append the entry per the format above. Then continue the CMO turn — do not swap modes.

---

## Active entries

_Queue empty — public fork reset. Append new `BB-*` YAML blocks below as CMO or Builder surfaces friction._
