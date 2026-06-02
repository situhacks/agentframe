# Voice Mini-Retro

Called by [`lock-event.md`](lock-event.md) step 6 at every deliverable lock. This file owns the eligibility check and the procedure. It does not self-trigger — it runs only when lock-event delegates to it.

Harvests the voice corrections made across a deliverable's version trail into the voice system — primarily as new annotated contrastive pairs in `library/context/operator/voice/pairs/` (the generative core), secondarily as rule patches. Voice-only; structure/content/strategy route to the System Retro.

## Eligibility (skip-when)

- **No `{name}-v1.md` snapshot** (back-fills, hand-written one-offs) → non-applicable; skip silently, log nothing.
- **Re-lock with no new versions** (unlock-revise-relock) → already ran; do not re-fire.
- **Otherwise** → run the procedure.

## Procedure — reconstruct from disk, no memory

Reads only files on disk (the version chain). Never relies on the agent remembering what it did while drafting. Works in a fresh session, after compaction, any time.

1. **List the version chain.** All `{name}-v1.md … {name}-vF.md`.
2. **Walk consecutive diffs** (`v1→v2 … v(n)→vF`). Skip diffs with no prose change (frontmatter, structure, table reorders). Deep-read only prose-changed diffs.
3. **Classify each prose change by reasoning over the text:**
   - **Voice** — cadence, word choice, staccato→flowing, fancy→plain, tone, hedge, a banned pattern removed, an operator rewrite into their voice. KEEP.
   - **Non-voice** — content, facts, structure, hook/CTA swap, typo. DISCARD.
4. **Cluster.** Collapse the same move repeated across versions into ONE candidate (19 staccato fixes → one).
5. **Per distinct move:**
   - **Generalizable move** (test: *would this teaching note help write a DIFFERENT post better?*) → candidate PAIR: BASE = AI version, BRANDON = corrected version, MOVE = what changed + why, register tag. One-offs → discard.
   - **A pattern that already has a rule, recurred anyway** → the rule didn't fire; flag for rule diagnosis (Rules below).
   - **Nothing earned** → silent pass.
6. **Propose to operator:** "This trail's corrections teach [move(s)]. Add as pair(s)? [if over cap: replaces weakest — name it]." Only approved candidates get written to `pairs/`.

## Pairs hygiene (prevent bloat)

- **Cap ~20–30, grouped by register.** Over cap → new pair REPLACES the weakest/most-redundant; the library swaps, never just grows.
- **Dedup by move.** One pair per move; a new example replaces the old or is skipped.
- **Recency-weighted.** Newest approved work wins; authentic Brandon writing outranks aspirational seeds over time.
- **Earn test gates every add:** generalizable, reusable move only.

## Rules (secondary path)

A correction becomes a rule only when it's a hard ban or an existing rule failed:
- **Rule existed, pattern recurred** → surface: "rule on [X] didn't constrain — diagnose now (system-improvement skill, target the relevant `voice/` file) or carry to System Retro?"
- **3+ new hard-ban instances in one deliverable** → surface: "patch `voice/anti-patterns.md` now or carry forward?"

Patch-now delegates to [`system/skills/system-improvement/SKILL.md`](../../system/skills/system-improvement/SKILL.md) targeting the specific file in `library/context/operator/voice/` (anti-patterns.md / identity.md / voice-profile.md — there is no monolithic voice.md). Earning citation = the version-chain diff.

## Durable record

- Pair added / rule patched → `system_changes` row (`validation_pending: true`).
- Candidate deferred → keep the diff snippet in campaign context for the System Retro.
- Silent pass → nothing.

## Publish/back-fill fallback (post-copy)

After `copy-v{N}.md` is reconciled to what shipped:
1. **Prior in-repo copy exists** → diff the chain, voice-only.
2. **No prior AI draft** → shipped copy is calibration only; harvest a pair only if the operator asks or the pattern repeats across 3+ posts. No "pass" claim (nothing to prove the voice held against).
3. **Shipped matches prior** → skip silently.

## Anti-patterns

- **Relying on memory** instead of the on-disk version chain. If it's not on disk, it doesn't count.
- **Dumping every version** instead of clustering deduped candidates.
- **Adding pairs past the cap or without the earn test.**
- **Patching a monolithic voice.md** — gone; target the specific `voice/` file.
- **Expanding beyond voice** — structure/content route to System Retro.
