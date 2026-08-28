# Deliverable Versioning

## Purpose

Own the iteration shape for versioned deliverables under `workspace/projects/{slug}/`: first-draft scaffolding, versioned changes, editable copies, readiness, and immutable publication. The CLI owns file and tracker mechanics; the agent and operator own content.

## When To Load

Load before the first write or rewrite to a kept deliverable. Reload after context compaction or when resuming a drafting task in a new conversation.

**Every content change to a head cuts a new version first.** There is no size threshold and no in-place exception: a typo fix versions, a copyedit versions, a full rewrite versions. Workspace files have no git history, so an in-place edit destroys the only snapshot of what the head said before, and the judgment call about which changes are "small enough" is the thing that keeps going wrong. A spare version costs nothing; a lost snapshot is unrecoverable.

| Operation | Mechanism |
|---|---|
| First draft | `af draft` |
| Existing authored draft | `af adopt` |
| Any content change to a head | `af version` before editing |
| Editable operator copy | `af version`, then hand off the new head |
| Ready after a change | `af version` -> edit -> `af ready` |
| Published copy needs change | Open a new version/edition; never edit the published artifact |

Frontmatter bookkeeping is not a content change: `status`, `last_updated`, and export/tracker fields are stamped in place, usually by a button.

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

### 2. Any content change, and editable copies

For a tracker-owned head:

```text
python system/af.py version <project> <row>
```

For a nested artifact:

```text
python system/af.py version <project> <parent-row> --artifact <artifact-name>
```

Run the command before changing content. It resolves the exact numeric head, creates `N+1`, resets the new head to drafting, refuses malformed/missing/colliding addresses, and leaves the prior version untouched. The nested form updates parent drafting state but does not move the parent file pointer.

The new head already contains the prior version's full content. Write the change as targeted `Edit` calls against that copy rather than retyping unchanged passages; that is the cheap path, and it is now editing a fresh snapshot rather than the only one. A full-file rewrite is right only when the change is genuinely whole-body (new thesis, new arc).

An operator asking for "a copy to edit" or "a hand copy" is making this request, not a judgment call: version, then hand off the new head. The snapshot they are protecting stays untouched.

### 3. Ready or published head

A ready head takes changes the same way: `af version` preserves the ready snapshot and creates a drafting head. A published head is immutable, and `af version` opens a new drafting head for ordinary versioned deliverables. An unversioned published assembly record cannot be reopened; create a new tracked edition.

If the operator explicitly overrides versioning and asks for an in-place edit, surface the snapshot risk first, then do as asked. Record the override in `activity.md` when downstream work depends on the prior shape.

### 4. Ready and publish

When the current head is good enough to use or share, follow [`ready-event.md`](ready-event.md). If approval includes any content change, version and edit first, then mark ready. For post ingredients, readiness updates `post-FINAL.md`. Run `af publish` only when the artifact is being issued as an immutable record.

## Verification Or Logging

After `af draft` or `af version`, verify the command receipt and filesystem:

- the named destination exists;
- the prior version is byte-unchanged;
- tracker pointer movement matches the address type;
- the new head has canonical shared drafting fields;
- any template-specific frontmatter is present before content drafting;
- no lower-numbered version was edited.

Routine iteration narration does not go to `activity.md`; the commands themselves append one terse work pulse per run (`artifact_drafted` on draft, `artifact_versioned` on version) so the calendar can derive worked time—never add pulse lines by hand. Per-version change narration belongs only in a template-declared `changes_from_v{N}` field. `ready` and `publish` each write their own transition receipt.

## Boundaries

- The CLI does not decide what a change means; it only makes the snapshot safe.
- The CLI does not generate content, run voice/humanizer passes, or encode domain phase order.
- Templates own type-specific content and frontmatter requirements.
- Domain packs own assembly behavior.
- `post-FINAL.md` remains unversioned; its ingredients carry the version trail.
