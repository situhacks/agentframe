# Voice Mini-Retro (per-deliverable lock event)

## What this is

A fast-cadence, voice-only mini-retro that fires automatically on every deliverable lock event, using the same defense-in-depth trigger shape as the [Lock-Event Trigger](../../AGENTS.builder.md).

## Why this exists

`voice.md` evolution can't wait for the campaign-end System Retro. If the operator rewrites the AI's voice in deliverable #1 to soften ranges, that fix must influence deliverable #2's first AI draft — not deliverable #2's *locked* draft after the operator rewrites the same softening pass again. Without this loop, the same voice stray repeats across every deliverable in the campaign, the operator absorbs the rewrite cost N times, and the System Retro at campaign end sees N instances of the same friction it could have prevented after instance 1.

**Scope is voice-only by design.** Profile / process / persona / template patches stay slow-loop (campaign-end System Retro). Mixing them into the per-lock cadence would make every lock a full retro, defeat the purpose, and surface premature patches before the campaign has enough cycles to cross the 3+ pattern threshold for non-voice patches.

## Trigger (defense in depth)

Same shape as the Lock-Event Trigger — fire on either:

1. **Frontmatter state (canonical):** any deliverable file `{name}-vF.md` transitions `status: drafting` → `status: locked` — OR is being set to `locked` by the operator in the current turn.
2. **Phrase trigger:** operator says "lock this" / "ship it" / "we're done with [this deliverable]" / "finalize" (or close variants) on a deliverable that has both a `{name}-v1.md` snapshot and a `{name}-vF.md` working file.

The voice mini-retro fires AFTER the Lock-Event Trigger procedure completes (state update + tracker update + exports). The lock event is the gating dependency — voice mini-retro reads the locked file.

## Procedure

When the trigger fires:

1. **Load the diff inputs.** Read the deliverable's `{name}-v1.md` (first AI draft) and `{name}-vF.md` (locked operator-approved version) end-to-end.

2. **Earning filter (voice scope only).** Compute the diff. Filter for changes that match voice patterns:
   - Banned-word recurrence (the operator removed a word listed in `voice.md` banned words — the AI used it anyway).
   - Mechanical tic recurrence (em-dash chains, parallelism stacks, hedge phrases — anything `voice.md` mechanical-rules section names).
   - Writing-style example mismatch (the operator's rewrite is in a clearly different voice than the AI's draft, in a way that doesn't match the writing-style examples in `voice.md`).
   
   Filter out everything else (structural changes, content additions/removals, factual corrections, hook variant swaps, CTA wording — those are template-shape or strategy-shape, not voice-shape, and route to System Retro).
   
   **If filtered diff is empty:** silent pass. Do NOT surface to operator and do NOT append a dedicated audit row — the silence IS the signal that voice held.

3. **Pattern strength check.** For each voice-pattern stray surfaced in step 2:
   - **Single clear recurrence of a pattern that already has a `voice.md` rule** (e.g. `voice.md` bans em-dash chains; the AI used em-dash chains; the operator removed them in vF) → the rule existed and did not constrain. Surface to operator: "voice rule on em-dash chains existed and did not fire. Want to diagnose the shape failure now (load the system-improvement skill, run prior-patch shape-failure check) or carry forward to System Retro for batch diagnosis?"
   - **Single recurrence of a NEW voice-pattern stray** (no existing rule) → carry forward. Keep the diff snippet in the current campaign context and surface it at System Retro; if 3+ deliverables in the same campaign surface the same candidate, the System Retro promotes it to a patch proposal.
   - **3+ instances of the same voice-pattern stray within this single deliverable's diff** → cross the 3+ threshold inside one lock. Surface to operator: "this deliverable's vF removed [pattern] in 3+ places. Patch `voice.md` now (load the system-improvement skill targeting voice.md only) or carry forward to System Retro?"

4. **If operator chose patch-now in step 3:** delegate to [`system/skills/system-improvement/SKILL.md`](../../system/skills/system-improvement/SKILL.md) with target `library/context/operator/voice.md`. The skill handles the full earn → read → diff → propose → defer-validation → apply → log loop. The earning citation is the deliverable's v1→vF diff (named explicitly per Step 2 of the skill — the v1→vF diff is one of the three citation classes).

5. **Durable record policy.**

- If the outcome is `patched-now`, the delegated `system-improvement` skill writes the canonical `system_changes` row for the `voice.md` patch, including `validation_pending: true`.
- If the outcome is `candidate` or `carry-to-system-retro`, keep the signal in the current campaign context (deliverable diff + retro narrative). Do not append a dedicated markdown log row just for the mini-retro.
- If the outcome is `pass`, stay silent.

## Publish/back-fill fallback

This fallback is for post-copy only, after `copy-vF.md` has been reconciled to the copy that actually shipped.

1. **If a prior in-repo copy exists:** compare the immediately prior `copy-vF.md` body to the shipped/reconciled body. Run the same voice-only earning filter above, but treat content/strategy changes as out of scope.
2. **If no prior AI draft exists:** do not claim a mini-retro pass. The shipped copy is calibration input only; surface a voice-learning candidate only when the operator asks or when the same shipped-copy pattern appears across 3+ posts.
3. **If shipped copy matches the prior in-repo copy:** skip silently. No voice signal changed at publish time.

## Anti-patterns

- **Expanding scope beyond voice.** If you're tempted to surface a template-shape stray (e.g. "the hook structure was wrong") inside the mini-retro, stop — that routes to System Retro. Mini-retro stays voice-only by design (see "Why this exists").
- **Surfacing every silent pass.** Silent pass = no surface to operator. Logging happens; surfacing does not. Surface only when the operator must decide (existing rule did not constrain, 3+ within-deliverable threshold, or new-pattern candidate that the operator might want to flag preemptively).
- **Skipping when v1 doesn't exist.** Some deliverables (back-fills, hand-written one-offs) won't have a `{name}-v1.md` snapshot — there's no AI draft to diff against. In that case the mini-retro is non-applicable; skip silently and do NOT log a `pass` event (a pass implies the diff was checked).
- **Calling back-fill calibration a pass.** A no-prior shipped post can teach the voice file, but it cannot prove the AI held voice because there was no AI draft to compare.
- **Running the mini-retro twice on the same lock.** The mini-retro fires once per `drafting → locked` transition. Re-locking after a brief unlock-revise-relock cycle does NOT re-fire (no new v1 was generated; the diff is unchanged). The Lock-Event Trigger's audit-event append is the dedup signal.

## Defense in depth — state-load checks

When state-loading a campaign or answering a state question, scan the campaign's deliverables block in `campaign.md`:
- For each `deliverables.{slug}` row at `status: locked`, confirm the deliverable still has both a `{name}-v1.md` snapshot and a `{name}-vF.md` working file. If both exist and the operator asks whether the voice loop ran, rerun the mini-retro from the diff directly; there is no separate live markdown log to consult anymore.
