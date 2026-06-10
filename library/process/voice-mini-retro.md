# Voice Mini-Retro

Called by [`lock-event.md`](lock-event.md) step 6 at every deliverable lock. The harvest procedure itself lives in the [`voice-harvest`](../../system/skills/voice-harvest/SKILL.md) skill; this file owns only the lock-event eligibility gate and the invocation.

## Eligibility (skip-when)

- **No `{name}-v1.md` snapshot** (back-fills, hand-written one-offs) → non-applicable; skip silently, log nothing.
- **Re-lock with no new versions** (unlock-revise-relock) → already ran; do not re-fire.
- **Otherwise** → run the harvest.

## Invocation

Load and run [`system/skills/voice-harvest/SKILL.md`](../../system/skills/voice-harvest/SKILL.md) on the deliverable that just locked.

- **Source tier:** this is a lock-event invocation, possibly in a fresh/compacted session. Default to **Tier 1 (disk diffs only)** — do not rely on chat memory that may be gone. The skill offers deeper tiers (transcript / chat) only when the operator has budget and the session holds the drafting context.
- **Outputs:** the skill proposes pairs (operator approves), writes approved ones to `pairs/`, and logs a backlog recurrence-watch if a voice issue recurred despite an existing pair. It logs its own `system_changes` row.

## Notes

- **Voice-only.** Structure/content/strategy learnings route to the System Retro, not here.
- **On-demand harvest** (operator says "update the voice" outside a lock) invokes the same skill directly — see the skill's "How this skill is invoked." This file is just the lock-event entry point.
