# AgentFrame Marketing — Lock-Event Procedure

Generic lock mechanics for deliverables. Load on demand when a deliverable is being locked by state change or clear operator intent.

---

## Trigger (defense in depth)

Fire when either is true:

1. A deliverable `*-v{N}.md` frontmatter is set to any terminal state — `status: locked`, `status: shipped`, or `status: published` — (or being set in the current turn). A deliverable reaching a terminal state by any route must pass through this procedure; `shipped` and `published` are not a way around the lock gate.
2. The operator clearly signals lock intent ("lock this", "finalize this", "ship this deliverable", or close variants).

---

## On trigger

1. **Verify template lock criteria.** Read the deliverable template and confirm criteria pass; surface gaps before locking.
2. **Run declared pre-lock quality gates.** If the template includes `## Humanizer Pass` (or another explicit gate), run it using the referenced process.
3. **Update deliverable frontmatter.** Set `status: locked` and `last_updated` in the canonical `-v{N}.md`.
4. **Update campaign tracker in the same turn.** Sync `campaign.md` `deliverables.{slug}` state and any phase/counter implications from the active flow.
5. **Append activity event.** Record the lock in `workspace/campaigns/{slug}/activity.md`.
6. **Run voice mini-retro when applicable.** Follow [`library/process/voice-mini-retro.md`](voice-mini-retro.md) trigger/eligibility rules.
7. **Surface result to operator.** Confirm lock state, plus any remaining follow-ups (review, export, publish).

---

## Scope boundary

Brief-specific export orchestration (`.docx`/`.pptx` discovery, render flow, and export artifacts) is owned by the brief templates (`business-brief`, `campaign-brief`) under their `Publish / Export Mechanics` sections.

---

## Defense in depth — state-load check

When opening campaign state, surface drift where deliverable frontmatter and `campaign.md` tracker disagree, and ask before fixing.
