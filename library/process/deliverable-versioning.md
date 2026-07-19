# Deliverable Versioning

## Purpose

Own the iteration shape for versioned deliverables under `workspace/projects/{slug}/`: first-draft scaffolding, surgical edits, replacement versions, editable copies, and lock reconciliation. The CLI owns file and tracker mechanics; the agent and operator own change judgment and content.

## When To Load

Load before the first write or rewrite to a kept deliverable. Reload after context compaction or when resuming a drafting task in a new conversation.

Before mutation, classify the operation:

| Operation | New version? | Mechanism |
|---|---:|---|
| First draft | Create v1 | `af draft` |
| Existing authored draft | Register existing file | `af adopt` |
| Surgical edit | No | Edit current drafting head; update `last_updated` |
| Replacement | Yes | `af version` before editing |
| Editable operator copy | Yes | `af version`, then hand off the new head |
| Lock after replacement | Yes, then lock | `af version` -> edit -> `af lock` |
| Delivered copy materially differs | Yes | Version the named row/artifact -> reconcile -> re-lock |

## Address Model

Versioned files use `{name}-v{N}.md`. The highest numeric `N` is the head. Do not create `current_version` or `version_history` frontmatter; the filename and directory carry the chain.

There are two addresses:

1. **Tracker-owned deliverable.** Its `project.md` row points directly to the versioned head. Versioning moves that pointer.
2. **Nested artifact.** A parent tracker row points to an assembly record or folder while named artifacts inside that folder carry their own version chains. Versioning the artifact preserves the parent pointer.

Marketing posts use the nested form: the post row points at unversioned `post-FINAL.md`; `body-copy-v{N}.md`, `slide-copy-v{N}.md`, and other ingredients hold the snapshots.

## Procedure

### 1. First draft

For a tracker-owned deliverable:

```text
python system/af.py draft <project> <row> --file <project-relative-name-v1.md>
```

For a nested artifact:

```text
python system/af.py draft <project> <parent-row> --artifact <artifact-name>
```

The command creates a shared frontmatter container with `status: drafting` and `last_updated`, refuses an existing chain, and updates tracker state. A domain hook may create or reconcile a parent assembly record; for a marketing post, the first ingredient creates `post-FINAL.md` and keeps the post row pointed there.

After scaffolding, load the resolved template and add any template-specific frontmatter fields before writing content. `af draft` does not render deliverable prose or infer template-specific fields.

When a renderer or external authoring step already produced a valid `status: drafting` Markdown artifact, register it without overwriting the file:

```text
python system/af.py adopt <project> <row> --file <project-relative-name-v1.md>
```

`af adopt` creates the tracker row when absent, updates an empty placeholder row when present, and refuses an existing competing artifact.

### 2. Surgical edit

Edit the current drafting head in place only when the change is bounded and does not move the deliverable's shape or claims:

- typo, copyedit, or small wording swap inside a paragraph;
- CTA wording swap that keeps the same CTA role;
- one citation, link, or reference with the surrounding claim unchanged;
- formatting-only change;
- ordinary frontmatter maintenance.

Update `last_updated`. Do not create a cosmetic version.

### 3. Replacement or editable copy

For a tracker-owned head:

```text
python system/af.py version <project> <row>
```

For a nested artifact:

```text
python system/af.py version <project> <parent-row> --artifact <artifact-name>
```

Run the command before changing content. It resolves the exact numeric head, creates `N+1`, resets the new head to drafting, refuses malformed/missing/colliding addresses, and leaves the prior version untouched. The nested form updates parent drafting state but does not move the parent file pointer.

The new head already contains the prior version's full content. Apply the replacement as surgical edits to that copy; do not retype unchanged passages. A full-file rewrite of the new head is right only when the replacement is genuinely whole-body (new thesis, new arc).

Replacement-shaped changes include:

- full-body rewrite, new angle, or new thesis;
- adding, removing, reordering, or materially rewriting sections;
- new audience framing, goal, hook, or arc;
- changing the recommended option among variants;
- operator pushback that requires a fresh working copy;
- an explicit request to save the current state and make a copy.

When the operator requests an editable copy, version first and identify the new head. The snapshot they are protecting remains untouched.

### 4. Locked or delivered head

Direct edits to a locked or delivered head are not allowed. After the operator confirms substantive revision, `af version` is the explicit unlock/version event: it creates a drafting head and records the material event in `activity.md`. Re-lock or republish through the owning process after reconciliation.

If the operator explicitly overrides versioning and asks for a substantive in-place edit, surface the snapshot risk first. Record the override in `activity.md` when downstream work depends on the prior shape.

### 5. Lock

When the operator approves the current head, follow [`lock-event.md`](lock-event.md). If approval includes replacement-shaped changes, version and edit first, then lock. For post ingredients, lock assembly updates `post-FINAL.md`; publish state belongs to that assembly record.

## Verification Or Logging

After `af draft` or `af version`, verify the command receipt and filesystem:

- the named destination exists;
- the prior version is byte-unchanged;
- tracker pointer movement matches the address type;
- the new head has canonical shared drafting fields;
- any template-specific frontmatter is present before content drafting;
- no lower-numbered version was edited.

Routine iteration narration does not go to `activity.md`; the commands themselves append one terse work pulse per run (`artifact_drafted` on draft, `artifact_versioned` on a drafting-to-drafting version) so the calendar can derive worked time — never add pulse lines by hand. Per-version change narration belongs only in a template-declared `changes_from_v{N}` field. Lock is the ordinary activity roll-up; an unlock/version event from a locked or delivered source is material and is logged by the command.

## Boundaries

- The CLI does not decide surgical versus replacement.
- The CLI does not generate content, run voice/humanizer passes, or encode domain phase order.
- Templates own type-specific content and frontmatter requirements.
- Domain packs own assembly behavior.
- `post-FINAL.md` remains unversioned; its ingredients carry the version trail.
