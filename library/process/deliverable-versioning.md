# Deliverable Versioning

Owns the iteration shape for every deliverable instance under `workspace/campaigns/{slug}/` — from first draft through lock. Lazy-loaded when the agent is drafting or iterating any deliverable instance.

## Naming

Versioned files use `{name}-v{N}.md`: `copy-v1.md`, `copy-v2.md`, `draft-v1.md`, `carousel-spec-v1.md`. The highest `N` in the folder is the head version. The campaign tracker's `deliverables.{slug}.file` pointer names the head directly so a state-load reads the head without scanning the folder.

## Frontmatter

Versioned files carry the deliverable type's existing frontmatter shape (depends_on, voice_loading, shipped_at, published, shipped_media, etc.) plus:

```yaml
status: drafting | locked | shipped | deferred
last_updated: <ISO 8601 date>
```

No `current_version` field. No `version_history` array. The filename carries the version number, the directory listing carries the chain, and `git log` carries the audit detail when someone wants it.

## First draft (v1)

The agent writes `copy-v1.md` (or `{name}-v1.md` for non-post deliverables) with `status: drafting`. The campaign tracker `deliverables.{slug}.file` is set to that path in the same turn.

At the end of the drafting turn, the agent offers: *"Want an editable copy you can revise yourself before the next iteration?"* The offer is opt-in to avoid token cost and surprise files when the operator just wants to read the draft first.

## Editable copy (operator opt-in)

When the operator accepts the offer, the agent copies the current head to the next version (`copy-v1.md` → `copy-v2.md`), updates the tracker to point at the new head, and tells the operator the file is ready to edit. The operator edits the new head directly. The prior version stays in the folder, untouched, as the snapshot for that point.

## Iteration (agent applies operator feedback)

When the operator gives feedback and the agent applies it, the agent writes the next version (`copy-v{N}.md` → `copy-v{N+1}.md`) with the changes applied, updates the tracker to the new head. The prior version stays in the folder as the snapshot.

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

The agent writes the new content to `{name}-v{N+1}.md`, updates the tracker pointer, and leaves the prior version in the folder.

## Lock and ship

When the operator approves the current head, the agent flips that file's `status` to `locked` per [`lock-event.md`](lock-event.md). On publish, the same head file's `status` flips to `shipped` with the publish-record fields filled in, per the publish coordination procedure in [`post-copy/template.md`](../deliverables/post-copy/template.md).

## Edge cases

- **Multiple iterations in one session:** each replacement writes its own `v{N+1}`. No batching.
- **Operator says "just edit in place" on a substantive change:** honor the request. Note the override in `activity.md` if downstream work depends on the prior shape.
- **Locked deliverable that needs a substantive change:** unlock first (operator decision), then apply the replacement procedure. The unlock event lives in `activity.md`.

## Self-check

Before writing a new version, the agent confirms:

- The current head file exists and the tracker `file:` pointer names it.
- The new filename increments by exactly one (no gaps).
- The tracker `file:` pointer is updated to the new head in the same turn.

If any check fails, surface the gap to the operator before writing.

## Interaction with lock-event

Lock-event mechanics live in [`lock-event.md`](lock-event.md) and run after this rule. If a lock turn includes replacement-shaped changes, write the new version first, then flip its status to `locked`. The two procedures compose: this file owns version creation; lock-event owns the status flip and tracker sync.
