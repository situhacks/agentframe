# Deliverable Versioning

Owns the iteration shape for every deliverable instance under `workspace/projects/{slug}/` — from first draft through lock. Lazy-loaded when the agent is drafting or iterating any deliverable instance.

## Naming

Versioned files use `{name}-v{N}.md`: `slide-copy-v1.md`, `body-copy-v2.md`, `draft-v1.md`. The highest `N` in the folder is the head version. The campaign tracker's `deliverables.{slug}.file` pointer names the head directly so a state-load reads the head without scanning the folder.

One exception per post folder: `post-FINAL.md` is unversioned. It is the post's assembly record — locked ingredient content accumulates there per [`post-final/template.md`](../deliverables/post-final/template.md) — and the ingredient files around it carry the version trails. Post rows in the tracker point at it rather than at an ingredient head.

## Frontmatter

Versioned files carry the deliverable type's existing frontmatter shape plus:

```yaml
status: drafting | locked | delivered | deferred
last_updated: <ISO 8601 date>
```

No `current_version` field. No `version_history` array. The filename carries the version number, the directory listing carries the chain, and `git log` carries the audit detail when someone wants it.

## First draft (v1)

The agent writes `{name}-v1.md` with `status: drafting`. The campaign tracker `deliverables.{slug}.file` is set to that path in the same turn (post-ingredient drafts don't move the post row — it points at `post-FINAL.md`, created with the first ingredient).

At the end of the drafting turn, the agent offers: *"Want an editable copy you can revise yourself before the next iteration?"* The offer is opt-in to avoid token cost and surprise files when the operator just wants to read the draft first.

## Editable copy (operator opt-in)

When the operator accepts the offer, run `python system/af.py version <campaign> <deliverable>` — it copies the head to the next version and moves the tracker pointer atomically — then tell the operator the new head is ready to edit. The prior version stays in the folder, untouched, as the snapshot for that point.

## Iteration (agent applies operator feedback)

When the feedback is replacement-shaped (see below), run `af version` first, then write the changes into the new head. The prior version stays in the folder as the snapshot.

If the feedback criticizes the deliverable's SHAPE or the agent's process (not just this draft's content — e.g. "v1 copy should never contain imagery notes," "the table format is wrong for this deliverable"), also append one line to the campaign's `feedback-log.md` in the same turn. That line is the paper trail the Phase-5 harvest retro reads; without it the correction lives only in chat.

## Surgical edit (no new version)

Apply when the change is bounded and does not move the deliverable's shape or claims:

- Typo fixes, copyedits, small wording swaps inside a paragraph.
- CTA wording swaps that keep the same CTA role.
- Frontmatter updates: `last_updated`, `status`, `published.*`, `shipped_media`, `exports[]`, `review`, `expected_feedback_by`.
- Adding a single citation, link, or reference where the surrounding claim is unchanged.
- Reformatting (heading level, list to prose, etc.) that does not change content.

The agent writes directly to the current head file. Update `last_updated`.

## Replacement (new version)

Apply when the change is structural or substantive:

- Full-body rewrite, new angle, or new thesis.
- Section restructure (adding, removing, or reordering top-level sections).
- Materially new audience framing, goal, hook, or arc.
- Replacing the recommended option among variants.
- Operator pushback that requires a fresh copy rather than an in-place edit.
- Operator request to archive the current head and start a new working copy ("make a copy", "save this and start a new version").

Run `af version`, then write the new content into the new head. The prior version stays in the folder.

## Lock and ship

When the operator approves the current head, follow [`lock-event.md`](lock-event.md) — `python system/af.py lock` owns the mechanics, including landing post-ingredient content in `post-FINAL.md`. Publish state lives on `post-FINAL.md` via `af publish`, per [`post-final/template.md`](../deliverables/post-final/template.md).

## Edge cases

- **Multiple iterations in one session:** each replacement gets its own `af version` call. No batching.
- **Operator says "just edit in place" on a substantive change:** honor the request. Note the override in `activity.md` if downstream work depends on the prior shape.
- **Locked deliverable that needs a substantive change:** unlock first (operator decision), then `af version`. The unlock event lives in `activity.md`.

## Interaction with lock-event

If a lock turn includes replacement-shaped changes, run `af version` and write the new content first, then lock per [`lock-event.md`](lock-event.md). The two compose: this file owns when a new version is earned; the buttons own the file/tracker mechanics.
