---
name: deliverable-harvest
version: 0.1.0
description: |
  Extract deliverable-SHAPE feedback from campaign source material and route it
  to the right home: template-patch candidates, feedback-log paper trail, and
  builder-backlog recurrence watches. Input-agnostic: a deliverable's version
  trail, a session transcript, or the live chat. Mines the same sources as
  voice-harvest but with a structure lens — what the operator repeatedly added,
  removed, or restructured (sections, tables, what belongs in a v1, weight,
  format) rather than how sentences sound. Asks depth up front (diffs-only /
  +transcript / +chat) to respect token budget. Proposes findings
  first-pass-then-approve; never patches templates directly (routes to
  system-improvement).
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
  - AskUserQuestion
---

# Deliverable Harvest: Extract Shape Feedback Into the System

Deliverable templates only stay good if they absorb how the operator actually corrected the work. This skill reads a campaign's correction record and extracts the **shape signal**: structure, sections, format, weight, and what-belongs-where — the feedback that should become template patches instead of being re-taught every campaign.

The highest-signal sources are the operator's manual edits and the big structural redirects ("v1 copy should not contain imagery notes," "the table needs two aligned columns"). Voice-level signal (cadence, word choice) is NOT this skill's job — that's `voice-harvest`.

## Shared-read rule (mutual awareness with voice-harvest)

`voice-harvest` and this skill mine the **same sources with different lenses**. At Step 1, if the operator wants both (typical at a campaign retro), **read the sources once and run both extraction passes** — never re-read per skill. When invoked alone, offer the other lens in one line ("same sources carry voice signal — harvest that too?") and drop it if declined.

## The procedure

### Step 1 — Scope sources and depth (ASK)

Identify the material (which deliverables, which trails), then ask the operator how deep, same tiers as voice-harvest: **1 — diffs only** (cheap; consecutive `v{n}→v{n+1}`, weighted to operator-edit transitions and big early structural rewrites) · **2 — + session transcripts** (rich, token-heavy) · **3 — + live chat** (only valid in the session that did the work). Default Tier 1. In a possibly-fresh session (retro weeks later), disk sources only — don't claim chat memory you don't have.

### Step 2 — Read and extract shape deltas

Walk the sources for **structure changes**: sections added/removed/reordered, format conversions (prose→table, list→prose), weight/length corrections, content-placement rules ("X doesn't belong in this deliverable/version"), sequencing feedback ("lock skeleton before prose"), and any operator statement about what the deliverable SHOULD contain. Discard voice-level deltas (route those to voice-harvest) and one-off content fixes (typos, facts).

### Step 3 — Cluster and classify

Collapse repeats into one candidate each. Per distinct finding:

- **Generalizable template gap** (test: *would the next campaign hit this with the current template?*) → **template-patch candidate**, named to its target `library/deliverables/{type}/template.md` section.
- **Already covered by the template but violated anyway** → **recurrence signal**: the template rule didn't fire. Log/update a `BB-*` watch in `system/builder-backlog.md` (first time = watch; matches a prior watch = confirmed, needs a structural fix, not a sharper-worded template line).
- **Campaign-specific, not generalizable** → one line to the campaign's `feedback-log.md` (paper trail for the campaign retro), nothing else.

### Step 4 — Propose (FIRST-PASS-THEN-APPROVE)

Surface all candidates grouped by destination: template-patch candidates (with target file + section), backlog watches, feedback-log lines. The operator approves, edits, or drops each. **Approved template patches route to `system/skills/system-improvement/SKILL.md`** — that skill owns the patch discipline (earning citation = the diff you just mined); this skill never edits templates directly.

### Step 5 — Log

Append one `system_changes` row via `system/audit/writer.py`: findings count by destination, sources mined (tier), deliverables covered. Feedback-log lines and backlog entries were written in Step 3–4; the audit row records the harvest ran.

## Boundaries

- **No direct template edits** — system-improvement owns the patch loop.
- **No voice extraction** — voice-harvest owns it; share the read, split the lenses.
- **No strategy/performance judgments** — campaign-retro territory.
- Findings are proposals until the operator approves; silent pass is a valid outcome (say so).
