---
name: deliverable-harvest
version: 0.1.0
description: |
  Extract deliverable-SHAPE feedback from finished work into template patches,
  local-deliverable promotions, and backlog recurrence watches, at
  ready or publish time and at project closeout. Reads the version trail (v1
  to head, with the operator's hand edits), the published copy against the
  ready copy, and the operator's words in the session, with a structure lens:
  sections, tables, what belongs in a v1, weight, format. Proposes every
  finding in one batched review; applies the approved ones on the operator's yes.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
  - AskUserQuestion
disable-model-invocation: true
---

# Deliverable Harvest: Extract Shape Feedback Into the System

Deliverable templates only stay good if they absorb how the operator actually corrected the work. This skill reads a deliverable's correction record and extracts the **shape signal**: structure, sections, format, weight, and what-belongs-where — the feedback that should become template patches instead of being re-taught every project.

The highest-signal sources are the operator's manual edits and the big structural redirects ("v1 copy should not contain imagery notes," "the table needs two aligned columns"). Voice-level signal (cadence, word choice) is `voice-harvest`'s job.

It runs at `af ready` or `af publish` on a deliverable the operator shaped, and at project closeout over every published head not yet harvested ([`project-flow.md`](../../../library/process/project-flow.md)). The operator can also ask for it directly.

## Shared-read rule (mutual awareness with voice-harvest)

`voice-harvest` and this skill mine the **same sources with different lenses**. When both run, read the sources once, run both extraction passes, and present one combined review. When invoked alone, offer the other lens in one line ("same sources carry voice signal — harvest that too?") and drop it if declined.

## The procedure

### Step 1 — Read the sources

- **Version trail:** list `{name}-v1 … {name}-v{N}` and walk consecutive `v{n}→v{n+1}` diffs, weighted to the operator's hand-edit copies (a verbatim copy he edited in place, so its diff isolates his keystrokes) and the big early structural rewrites. Each head's `changes_from_v{N}` field carries the operator's feedback on that draft.
- **Published versus ready:** when the deliverable shipped, diff the live copy (the published URL or the platform draft) against the ready head. What changed between them is late operator correction.
- **The operator's words in the session:** structural direction he gave in this conversation, when the session did the work. In a fresh or compacted session, read disk only; don't claim chat memory you don't have.

### Step 2 — Extract shape deltas

Walk the sources for **structure changes**: sections added/removed/reordered, format conversions (prose→table, list→prose), weight/length corrections, content-placement rules ("X doesn't belong in this deliverable/version"), sequencing feedback ("settle the skeleton before prose"), and any operator statement about what the deliverable SHOULD contain. Discard voice-level deltas (voice-harvest's) and one-off content fixes (typos, facts).

### Step 3 — Cluster and classify

Collapse repeats into one candidate each. Per distinct finding:

- **Generalizable template gap** (test: *would the next project hit this with the current template?*) → **template-patch candidate**, named to its target `template.md` section.
- **Already covered by the template but violated anyway** → **recurrence signal**: the template rule didn't fire. Grep `system/backlog.md` for a prior watch: first time = a new open `BB-*` row asking whether it recurs; matches a prior watch = confirmed, needs a structural fix, not a sharper-worded template line.
- **Local deliverable promotion candidate** (scan the project's `_local/` folders; test: *is this type reusable across other projects in this domain?* and *does it cover less than 70% of any existing template?*) → **promotion candidate**, target `library/domains/{domain}/deliverables/{type}/template.md` (the active project's domain).
- **Project-specific, not generalizable** → nothing to route. A per-piece correction already lives in that version's `changes_from_vN`; a production note the project's next deliverable needs goes in the `project.md` body.

### Step 4 — One batched review, then apply

Surface every candidate in one review, grouped by destination: template patches (target file, section, proposed text), promotion candidates, backlog watches. The operator approves, edits, or drops each. On his yes:
- **Template patches:** apply under the Builder pre-write gate in `AGENTS.builder.md` — name the tendency the patch counters, find the prior rule on the topic (if it has patch history, name why the prior shape failed), and keep the template inside its size budget.
- **Local promotions:** write the template to the pack's `deliverables/` folder, applying [`authoring.md`](../../../library/process/authoring.md).
- **Backlog watches:** append or update the `BB-*` rows in `system/backlog.md`.

Report what landed where; a silent pass is a valid outcome.

## Boundaries

- **No edits before the operator's yes** — findings are proposals until the review approves them.
- **No voice extraction** — voice-harvest owns it; share the read, split the lenses.
- **No strategy or performance judgments** — those stay in the project.
