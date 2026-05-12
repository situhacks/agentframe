# AgentFrame Marketing — Humanizer Integration

This file owns gate timing, scope, and logging for `## Humanizer Pass` sections in public-facing deliverables. The skill at [`system/skills/humanizer/SKILL.md`](../../system/skills/humanizer/SKILL.md) owns rewrite technique.

Loaded on demand only when a template declares `## Humanizer Pass` with non-empty guidance.

---

## Default scope

Apply to prose paragraphs only. Skip code blocks, structured tables, frontmatter, and labels. Output is a rewrite of the prose, not a critique.

Templates can narrow scope in their `## Humanizer Pass` guidance (for example slide text only, or `video/SCRIPT.md` only).

---

## Gates

Three trigger points. Run automatically; don't ask permission.

| Gate | Trigger | Scope |
|---|---|---|
| **First draft** | Before surfacing the initial draft to the user | Full pass on the prose-carrying artifact (or scoped regions per template guidance) |
| **Iteration** | Before a "ready for review" surface where prose has changed since the last pass | Delta pass on regions you rewrote. Skip if the turn was structural-only (table reorderings, frontmatter edits). If the delta is murky after a long session, do a full pass and say so. |
| **Lock** | Wired into lock-event pre-lock quality gates | Full pass on the prose-carrying artifact before lock/publish handoff. |

If a template names a sub-artifact as prose owner (for example `video/SCRIPT.md`), map first-draft/iteration/lock to that sub-artifact rather than the parent `-vF.md`.

---

## User notification

At first-draft and lock gates, surface a one-line note that humanizer ran and what artifact/scope it covered; iteration-gate passes stay silent unless asked.

---

## Logging

Append one `humanizer_pass` event to `workspace/campaigns/{slug}/activity.md` per pass with `deliverable_type`, `artifact`, `gate`, `scope`, and `regions_changed`.
