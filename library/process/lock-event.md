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
3. **Run the button.** `python system/af.py lock <campaign-slug> <deliverable-slug-or-path>` — it owns the mechanics atomically: frontmatter flip, post-FINAL assembly for manifest ingredients, tracker sync, activity event. Never hand-edit a terminal status.
4. **Work the printed checklist.** The button prints the judgment steps that remain (voice mini-retro eligibility per [`voice-mini-retro.md`](voice-mini-retro.md), remaining follow-ups). Surface the result to the operator.

---

## Scope boundary

Brief-specific export orchestration (`.docx`/`.pptx` discovery, render flow, and export artifacts) is owned by the brief templates (`business-brief`, `campaign-brief`) under their `Publish / Export Mechanics` sections.

---

## Defense in depth — state-load check

When opening campaign state, run `python system/af.py doctor <campaign-slug>` and surface any drift it reports; ask before fixing.
