# Deliverable Versioning

Trigger: agent is about to write replacement body content into a `*-vF.md` deliverable that already exists with `current_version >= 1`. Lazy-loaded from the persona routing index when this state is reached. Not loaded for net-new drafts (no prior content to snapshot).

This file owns the surgical-vs-replacement rule for every deliverable instance under `workspace/campaigns/{slug}/`. Templates do not restate it; deliverable-specific edit-shape examples may live in the relevant `library/deliverables/{type}/template-vF.md` when the type earns it.

## Surgical edit (no version bump)

Apply when the change is bounded and does not move the deliverable's shape or claims:

- Typo fixes, copyedits, small wording swaps inside a paragraph.
- CTA wording swaps that keep the same CTA role.
- Frontmatter updates: `last_updated`, `status`, `published.*`, `shipped_media`, `exports[]`, `review`, `expected_feedback_by`.
- Adding a single citation, link, or reference where the surrounding claim is unchanged.
- Reformatting (heading level, list to prose, etc.) that does not change content.

For a surgical edit, the agent writes directly to `*-vF.md`. Update `last_updated`. Do not bump `current_version`. Do not append to `version_history`.

## Replacement (snapshot + version bump)

Apply when the change is structural or substantive:

- Full-body rewrite, new angle, or new thesis.
- Section restructure (adding, removing, or reordering top-level sections).
- Materially new audience framing, goal, hook, or arc.
- Replacing the recommended option among variants.
- Operator pushback that requires "new version" rather than "edit in place."

For a replacement:

1. Copy the current `*-vF.md` to `*-v{N}.md` where `N` is the existing `current_version`. The snapshot is immutable — set `status: snapshot` in its frontmatter and stop touching it.
2. Write the new body into `*-vF.md`.
3. Bump `current_version` to `N + 1`.
4. Append one entry to `version_history` naming the reason in one line:
   ```yaml
   version_history:
     - {v: 2, date: 2026-05-11, note: "Operator pushback: too announcy; reworked hook and tightened body"}
   ```
5. Update `last_updated`.

The snapshot file (`*-v{N}.md`) is the immutable prior; the canonical current file is always `*-vF.md`.

## Edge cases

- **First draft (current_version: 1, never operator-touched):** keep editing `*-vF.md` directly. No snapshot. The first replacement that follows operator review is what triggers the snapshot of v1 ? `*-v1.md` and the bump to v2.
- **Multiple consecutive replacements in one session:** snapshot once per version bump. Do not snapshot intermediate states.
- **Operator says "new version" explicitly:** treat as replacement even if the change looks small.
- **Operator says "just edit in place" on a structural change:** honor the request, log the override decision in `activity.md` if downstream work depends on the prior shape.
- **Locked deliverable that needs a substantive change:** unlock first (operator decision), then apply the replacement procedure. The unlock event lives in `activity.md`.

## Interaction with lock-event

Lock-event mechanics live in [`lock-event.md`](lock-event.md) and run after this versioning rule. If a lock turn includes replacement-shaped changes, snapshot first, then lock the new `*-vF.md`. The two procedures compose: versioning owns the snapshot/bump; lock-event owns the status flip and tracker sync.

## Self-check

Before writing replacement content, the agent confirms:

- The prior `*-vF.md` exists and has `current_version >= 1`.
- A snapshot file `*-v{N}.md` does not yet exist for the current `N` (or is the same file we are about to create).
- The new `version_history` entry names the reason in one line.

If any check fails, surface the gap to the operator before writing.
