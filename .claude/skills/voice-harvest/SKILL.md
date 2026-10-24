---
name: voice-harvest
version: 0.1.0
description: |
  Extract the operator's voice signal from finished work and fit it into the
  voice system: annotated contrastive pairs (ACPs) in
  library/context/operator/voice/pairs/, corpus promotion of user-voiced finals
  into library/context/operator/voice/corpus/, voice-file patches
  (anti-patterns, profile, identity, register overlays), and backlog
  recurrence watches when a voice issue recurs despite an existing rule or
  pair. Runs at ready or publish time on a user-voiced deliverable, at project
  closeout, or when the operator says "update/harvest the voice", "do a voice
  pass", or "fold this into the voice system". Sources: the deliverable's
  version trail (v1 to head, including the operator's hand edits), the
  published copy against the ready copy, and the operator's words in the
  session. Proposes every finding in one batched review and applies the
  approved ones on the operator's yes.
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

# Voice Harvest: Extract Voice Signal Into the Voice System

The voice system generates by imitating **full-piece exemplars** (`corpus/`) and steering with **annotated contrastive pairs** (ACPs): a generic BASE → the person's REWRITE → a MOVE note on what changed. Both layers only stay good if they keep absorbing real work. This skill is the harvester: read source material, find the deltas that teach voice, fold the generalizable ones into `pairs/` or the voice files, and promote finished user-voiced finals into `corpus/`.

The richest signal is **what the human changed** — the operator's manual edits to an AI draft, and the late fine-tuning passes — not the AI's own drafts. Weight toward those. But the early big-rewrite diffs (where the operator gave AI structural direction) also carry voice. Read both.

## When it runs

- `af ready` on a user-voiced deliverable, under the gate in [`ready-event.md`](../../../library/process/ready-event.md).
- `af publish` on a user-voiced final; corpus promotion becomes eligible once it is live.
- Project closeout, over every published head not yet harvested ([`project-flow.md`](../../../library/process/project-flow.md)).
- The operator asks for a voice pass. If the request already scopes the pass ("just the pairs", "only post 3"), run at that scope.

## Shared-read rule (mutual awareness with deliverable-harvest)

`deliverable-harvest` and this skill mine the **same sources with different lenses** (shape/structure vs. voice). When both run, read the sources once, run both extraction passes, and present one combined review. When invoked alone, offer the other lens in one line and drop it if declined.

Out of scope: writing deliverable copy in-voice (the agent reads `voice/` and drafts natively — no skill between).

## What this skill owns vs. routes

| In scope | Routes elsewhere |
|---|---|
| Read sources → extract voice deltas → propose/write **pairs** | Structure/content learnings → `deliverable-harvest` |
| Propose/apply **voice-file patches**: `anti-patterns.md`, `voice-profile.md`, `identity.md`, register overlays | The pairs-format spec → `pairs/README.md`; the corpus rules → `corpus/README.md` (read them, don't redefine them) |
| Propose/write **corpus promotion** of published/ready finals | A structural fix for a confirmed recurrence → a Builder session |
| Log a **recurrence-watch** to `system/backlog.md` | |

## The procedure

### Step 1 — Read the sources

- **Version trail:** list the chain (`{name}-v1 … {name}-v{N}`). Walk **consecutive** diffs, not just v1→v{N} — the v(n)→v(n+1) transitions are where individual moves live, and the **operator-edit handoffs** (manual rewrites) + the **final smoothing passes** are the highest-signal. Read the prose-changed diffs; skip pure frontmatter/structure/table-reorder diffs. Each head's `changes_from_v{N}` field carries the operator's feedback on that draft.
- **The hand-edit-copy convention (how to isolate operator keystrokes):** operator-edit versions are created as a verbatim copy ("Operator hand-edit copy — edit directly here") and edited in place, so `diff v(N-1)→v(N)` on that copy isolates the operator's changes exactly. Typos in these diffs mark authentic operator text — signal, not noise. The operator may also edit *other* versions in place; those edits only surface inside the next agent-cleanup diff, mixed with agent changes — use the version notes ("cleanup of the operator's vN edits") to attribute. A line-level change you can't attribute from disk gets reported as unattributed, never assumed.
- **Published versus ready:** when the final shipped, diff the live copy (the published URL or the platform draft) against the ready head. Changes made between ready and live are the operator's last-mile voice edits; weight them like hand edits.
- **The operator's words in the session:** in-chat corrections, the "too fancy / fix it" feedback, the reject→accept exchanges — valid only when this session did the work. In a fresh or compacted session (a ready-event or closeout run), read disk only; never claim chat-derived pairs you can't reconstruct from disk.
- Always read the current `pairs/*.md` so you know what's already covered across register- and context-grouped files, and `pairs/README.md` for the format.

### Step 2 — Extract and classify deltas

For each prose change, classify:

- **Voice** — cadence (staccato↔flowing), word choice (fancy↔plain), tone, hedge, **stance** (the writer's relationship to the reader: pundit↔peer, swagger↔confession, observation↔direct address), a banned pattern removed, an operator rewrite into their own voice. KEEP.
- **Non-voice** — content, facts, structure, hook/CTA swap, typo. DISCARD (structural learnings belong to deliverable-harvest).

**Cluster:** collapse the same move repeated across many versions into ONE candidate (nineteen staccato fixes → one pair).

### Step 3 — Per distinct move, decide its destination

- **Generalizable, net-new move** (test: *would this MOVE note help write a DIFFERENT future piece better?*) → candidate **PAIR**. One-offs → discard.
- **A move that already has a pair/rule, but recurred anyway** → the existing pair didn't constrain. This is a **recurrence signal**, not a new pair. Go to Step 6.
- **Most clusters land at profile/register level** (stance, header shape, a licensed move) rather than sentence level → the harvest's main output is ONE bundled voice-file patch, not a pile of pairs. Write pairs only for the sentence-level moves that anchor it.
- **Nothing earned** → silent pass; say so.

### Step 4 — Draft pairs and voice-file patches

Do the first pass yourself. For each pair candidate:

```
### tag — one-line label
BASE: <generic / pre-edit version>
BRANDON: <the operator's actual rewrite>
MOVE: <what changed + why — generatively useful; cite the source delta, e.g. "Post 5 v9→v10op">
register: formal | informal | both
context: builder-pov | market-signal | slide | cover | email | long-form | <other task context>
```

A MOVE note states the move in the present tense and may end with a short source tag, as in the example above; it never narrates what happened in the session. A rule patched into a voice file carries no source tag at all.

**Pairs hygiene** (from `pairs/README.md`): keep register and task context separate; never use a channel as either one. There is no numeric cap; a pair that teaches a live move earns its context. Every harvest prunes before it adds, on three tests: a pair that duplicates another pair's move, a pair that only restates a rule already in `anti-patterns.md` or `voice-profile.md`, and a pair whose BRANDON side shows a move the current corpus no longer makes. Name each retirement and what absorbs it.

**Voice-file patches:** name the target file, the section, and the proposed text. Apply the Builder pre-write gate in `AGENTS.builder.md`: name the tendency the rule counters, find the prior rule on the topic (if it has patch history, name why the prior shape failed), and cut before adding.

### Step 5 — Corpus promotion (full-piece exemplars)

When the harvested source includes a **published or operator-approved ready user-voiced final** (an essay that went live, a ready LinkedIn body + carousel copy), propose promoting it into the register's corpus:

1. Read `library/context/operator/voice/corpus/README.md` (the rules: 3–5 per register, topical diversity, verbatim finals only, one provenance line, recency-weighted pruning) and the target register folder.
2. Propose: the piece verbatim (reader-facing prose only—strip production notes, char targets, changelogs) + one provenance line (source path/URL + ready/publish date). If the register is at cap, name the piece it replaces and why (oldest / most topic-redundant).

Agent drafts and cleaned intermediates never promote—only text the operator wrote or explicitly marked ready as final.

### Step 6 — Recurrence-watch

The backlog doubles as the voice system's "is this a real pattern?" tracker. Grep `system/backlog.md` for a prior matching watch row first. Two cases:

- **Suspected-but-unconfirmed** (a voice issue appeared, but it's the first time and you're not certain it's systemic): propose a `BB-*` **watch entry** — one line naming the issue + "watch: does this recur next harvest?"
- **Confirmed** (this harvest's issue matches a prior watch entry — it recurred): propose updating that `BB-*` entry from watch → confirmed pattern, noting it likely needs a structural fix (a rule reshape, a mechanism change) in a Builder session, not another pair. A pattern that recurs despite a pair means the pair isn't firing; more pairs won't fix it.

### Step 7 — One batched review, then apply

Present every candidate in one review: pairs (with their prunes), voice-file patches, corpus promotions, backlog watches. The operator approves, edits, or drops each. On his yes, write approved pairs to the matching register- or context-grouped pair file, apply approved voice-file patches, write approved promotions to `corpus/{register}/`, and append or update the backlog entries. A declined promotion is dropped. Report what landed where and the pair count of each file touched, so the operator can see the context cost.

## What this skill does NOT do

- **Does not edit anything before the operator's yes.** Every output waits for the batched review.
- **Does not own the pairs format or the corpus rules.** Those live in `pairs/README.md` and `corpus/README.md`. Read them; don't redefine them here.
- **Does not write deliverable copy.** It harvests voice FROM finished/edited copy; it doesn't produce new copy.
- **Does not decide structure/content learnings.** Those belong to deliverable-harvest.

## Edge cases

- **No version trail** (a hand-written one-off, a back-fill): the ready-event gate skips it. When the operator asks anyway, the piece is a corpus-promotion candidate; harvest a pair from it only if the move repeats across 3+ pieces (no reject→accept delta means weaker evidence).
- **Many strong candidates at once:** propose them all; the prune tests above, not a count, decide what leaves.
- **The same move shows up as both a new-pair candidate AND a recurrence:** it recurred → treat as recurrence (Step 6), not a fresh pair. A duplicate pair is the wrong fix.

## Forker note

The discipline is the portable part: walk consecutive diffs weighted to human edits → cluster → generalizable move becomes a pair or voice-file patch / recurrence becomes a backlog watch → one batched review → apply. Swap your own pairs location and your own backlog; the harvest loop ports unchanged.
