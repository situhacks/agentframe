---
name: voice-harvest
version: 0.1.0
description: |
  Extract a person's voice signal from source material and fit it into the
  voice system as annotated contrastive pairs (ACPs). Input-agnostic: point it
  at a deliverable's version trail, a session transcript, a fresh artifact
  (article/post/email), or the live chat — any material that shows how the
  person actually writes or how they corrected AI drafts into their voice.
  Use when the operator says "update/harvest the voice", "do a voice pass",
  "fold this into the voice system", or when a lock-event / voice-mini-retro
  delegates a harvest. Asks the operator how deep to mine (diffs-only vs.
  transcript vs. full) up front, because transcript deep-dives are token-heavy.
  Proposes pairs (first-pass-then-approve), writes approved ones to
  library/context/operator/voice/pairs/, and logs a recurrence-watch entry to
  the builder backlog when a voice issue recurs despite an existing rule/pair.
  Scope is harvest→pairs+backlog-watch ONLY; anti-patterns/profile/identity
  edits route to system-improvement.
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
  - AskUserQuestion
---

# Voice Harvest: Extract Voice Signal Into the Pairs Library

The voice system generates from **annotated contrastive pairs** (ACPs): a generic BASE → the person's REWRITE → a MOVE note on what changed. The pairs only stay good if they keep absorbing real corrections. This skill is the harvester: read source material, find the deltas that teach voice, and fold the generalizable ones into `pairs/`.

The richest signal is **what the human changed** — the operator's manual edits to an AI draft, and the late fine-tuning passes — not the AI's own drafts. Weight toward those. But the early big-rewrite diffs (where the operator gave AI structural direction) also carry voice. Read both.

## Shared-read rule (mutual awareness with deliverable-harvest)

`deliverable-harvest` and this skill mine the **same sources with different lenses** (shape/structure vs. voice). At Step 1, if the operator wants both (typical at a campaign retro), **read the sources once and run both extraction passes** — never re-read per skill. When invoked alone, offer the other lens in one line and drop it if declined.

Out of scope: patching `anti-patterns.md` / `voice-profile.md` / `identity.md` (→ `system-improvement`); writing deliverable copy in-voice (the agent reads `voice/` and drafts natively — no skill between).

## What this skill owns vs. routes

| In scope | Routes elsewhere |
|---|---|
| Read sources → extract voice deltas → propose/write **pairs** | Patching anti-patterns / profile / identity → `system-improvement` |
| Log a **recurrence-watch** to `builder-backlog.md` | Structure/content/strategy learnings → System Retro / Campaign Retro |
| | The pairs-format spec itself → `pairs/README.md` (read it, don't redefine it) |

## The procedure

### Step 1 — Scope the sources (ASK; respect the token budget)

Identify what material exists for this harvest, then **ask the operator how deep to go** — transcript mining is token-heavy and the operator may be low on budget. Offer the tiers:

| Tier | Sources read | Cost | When |
|---|---|---|---|
| **1 — Diffs only** | The deliverable's version trail: consecutive `v{n}→v{n+1}` diffs, **weighted to operator-edit transitions** (`*-operator-edits.md`, `edited_by: operator`) and the big early rewrites | Cheap | Default / low budget |
| **2 — + Transcript** | Tier 1 + the session transcript(s) where this work was discussed (`~/.claude/projects/<project>/<session>.jsonl`) — the discussion, feedback, and evolution, which is often richer than the diffs | Heavy | Operator has budget + wants depth |
| **3 — + Live chat** | Tier 2 + the current session's in-chat corrections (only valid in the live session that did the work) | Variable | Harvesting work just done this session |

Ask with `AskUserQuestion`: *"How deep should I mine — diffs only (cheap), + transcript (rich, token-heavy), or full (chat too)?"* Default to Tier 1 if the operator doesn't care.

**Mode note (fidelity guard):** if invoked by lock-event / voice-mini-retro in a possibly-fresh session, **Tier 1 (disk diffs) is the safe default** — do not rely on chat memory that may be gone after compaction. The "use memory/chat" tiers are opt-in and only valid when the session actually holds the drafting context. Reconstruct from disk when in doubt.

### Step 2 — Read the sources

- **Version trail (Tier 1):** list the chain (`{name}-v1 … {name}-v{N}`). Walk **consecutive** diffs, not just v1→v{N} — the v(n)→v(n+1) transitions are where individual moves live, and the **operator-edit handoffs** (manual rewrites) + the **final smoothing passes** are the highest-signal. Read the prose-changed diffs; skip pure frontmatter/structure/table-reorder diffs.
- **Transcript (Tier 2):** read the session JSONL for the stretch where this work evolved — the operator's dictated direction, the "too fancy / fix it" feedback, the reject→accept exchanges. This captures corrections that never hit a version file (in-chat iteration).
- **Fresh artifact:** if the input is a net-new piece (an article, a sent email, a post the person wrote), read it as a positive exemplar — extract BRANDON-side lines directly; the BASE is the generic version you reconstruct.
- Always read the current `pairs/*.md` (all register files) so you know what's already covered and don't duplicate. Read `pairs/README.md` for the format.

### Step 3 — Extract and classify deltas

For each prose change, classify:

- **Voice** — cadence (staccato↔flowing), word choice (fancy↔plain), tone, hedge, a banned pattern removed, an operator rewrite into their own voice. KEEP.
- **Non-voice** — content, facts, structure, hook/CTA swap, typo. DISCARD (route structural learnings to the relevant retro, not here).

**Cluster:** collapse the same move repeated across many versions into ONE candidate (nineteen staccato fixes → one pair).

### Step 4 — Per distinct move, decide its destination

- **Generalizable, net-new move** (test: *would this MOVE note help write a DIFFERENT future piece better?*) → candidate **PAIR**. One-offs → discard.
- **A move that already has a pair/rule, but recurred anyway** → the existing pair didn't constrain. This is a **recurrence signal**, not a new pair. Go to Step 6 (backlog-watch).
- **Nothing earned** → silent pass; say so.

### Step 5 — Propose pairs (FIRST-PASS-THEN-APPROVE)

Do the first pass yourself, then surface candidates for the operator to approve — the retro rhythm the operator likes. For each candidate:

```
### tag — one-line label
BASE: <generic / pre-edit version>
BRANDON: <the operator's actual rewrite>
MOVE: <what changed + why — generatively useful; cite the source delta, e.g. "Post 5 v9→v10op">
register: builder-pov | market-signal | slide | cover
```

**Pairs hygiene** (from `pairs/README.md` / voice-mini-retro): cap ~30 total grouped by register; over cap → a new pair REPLACES the weakest/most-redundant (name it); dedup by move; recency-weighted (newest approved work wins). Surface candidates; write only approved ones to the matching `pairs/{register}.md`.

### Step 6 — Recurrence-watch (the cross-retro memory)

The builder backlog doubles as the voice system's "is this a real pattern?" tracker. Two cases:

- **Suspected-but-unconfirmed** (a voice issue appeared, but it's the first time and you're not certain it's systemic): log a `BB-*` **watch entry** — one line naming the issue + "watch: does this recur next harvest?" This costs almost nothing and creates the memory.
- **Confirmed** (this harvest's issue matches a prior watch entry — it recurred): the watch is now proven. Update that `BB-*` entry from watch → confirmed pattern, and note it likely needs a structural fix (a rule reshape, a mechanism change) — route that fix to `system-improvement` / a Builder session, not another pair. A pattern that recurs despite a pair means the pair isn't firing; more pairs won't fix it.

Before logging, grep `builder-backlog.md` for a prior matching watch entry. Backlog format lives in `system/builder-backlog.md`.

### Step 7 — Log the harvest

Append a `system_changes` row via `system/audit/writer.py` recording: pairs added (count + tags), pairs replaced (if over cap), backlog-watch entries logged/confirmed, sources mined (tier), and the deliverable/artifact harvested. One row per harvest run.

## What this skill does NOT do

- **Does not patch `anti-patterns.md` / `voice-profile.md` / `identity.md`.** A recurring hard-ban or a profile shift routes to `system-improvement`. This skill only writes `pairs/` + backlog-watch.
- **Does not own the pairs format or the cap.** Those live in `pairs/README.md`. Read them; don't redefine them here.
- **Does not write deliverable copy.** It harvests voice FROM finished/edited copy; it doesn't produce new copy.
- **Does not decide structure/content learnings.** Those route to System Retro / Campaign Retro.

## Edge cases

- **No version trail and no transcript** (a hand-written one-off, a back-fill): only a fresh-artifact harvest is possible — mine the artifact as a positive exemplar. If there's nothing to diff against, harvest a pair only if the operator asks or the move repeats across 3+ pieces (no reject→accept delta means weaker evidence).
- **Operator is low on tokens:** stay Tier 1 (diffs only). Surface that a deeper transcript mine is available later if the diff pass looks thin.
- **Fresh session, lock-event invocation:** Tier 1 disk-only is the default; do not claim chat-derived pairs you can't reconstruct from disk.
- **Over the pairs cap with several strong candidates:** propose the swaps (new replaces weakest, named) rather than growing past ~30. If the operator wants them all, that's a signal the cap should move — surface it, don't silently exceed.
- **The same move shows up as both a new-pair candidate AND a recurrence:** it recurred → treat as recurrence (Step 6), not a fresh pair. A duplicate pair is the wrong fix.

## Forker note

The discipline is the portable part: scope sources by budget → walk consecutive diffs weighted to human edits → cluster → generalizable-move-becomes-pair / recurrence-becomes-backlog-watch → approve → log. Swap your own pairs location, your own backlog, and your own audit sink; the harvest loop ports unchanged.
