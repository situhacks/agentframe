# Voice Mini-Retro

Called by [`ready-event.md`](ready-event.md) when a deliverable becomes ready. The harvest procedure itself lives in the [`voice-harvest`](../../system/skills/voice-harvest/SKILL.md) skill; this file owns only the ready-event eligibility gate and the invocation.

## Eligibility (skip-when)

- **No `{name}-v1.md` snapshot** (back-fills, hand-written one-offs) → non-applicable; skip silently, log nothing.
- **Small correction to an already-ready head** → already ran; do not re-fire.
- **Otherwise** → run the harvest.

## Invocation

Load and run [`system/skills/voice-harvest/SKILL.md`](../../system/skills/voice-harvest/SKILL.md) on the deliverable that just became ready.

- **Source tier:** this is a ready-event invocation, possibly in a fresh/compacted session. Default to **Tier 1 (disk diffs only)** — do not rely on chat memory that may be gone. The skill offers deeper tiers (transcript / chat) only when the operator has budget and the session holds the drafting context.
- **Outputs:** the skill proposes pairs (operator approves), writes approved ones to `pairs/`, proposes corpus promotion when the ready/published final is user-voiced (full piece → `voice/corpus/{register}/`), and logs a backlog recurrence-watch if a voice issue recurred despite an existing pair. It logs its own `system_changes` row.

## Notes

- **Voice-only.** Structure/content/strategy learnings route to the System Retro, not here.
- **On-demand harvest** (operator says "update the voice" outside readiness) invokes the same skill directly—see the skill's "How this skill is invoked." This file is just the ready-event entry point.
