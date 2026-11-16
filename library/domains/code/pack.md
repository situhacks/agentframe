---
domain: code
prefix: code
extension_fields: []
verbs: [doctor]
dirs: [code]
learnings: true
---

# Code domain pack

The domain for a project whose deliverable is code. A code project is `domain: code`; its files live in `workspace/projects/<slug>/code/` as plain files, so they stay private and out of the search index, while its plan and decisions live in `project.md`.

This pack is the only artifact that knows code projects. `af new-project` reads the fields above; the router names no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Frontmatter extension | `extension_fields` (none) + `prefix` | The prefix is `code-`. |
| Scaffold skeleton | [`skeleton.md`](skeleton.md) | The `project.md` that `new-project` writes, with four stubs the kickoff fills: Goal, Stack, Test command, Done when. |
| Folders | `dirs` | Each name becomes a folder in the project. `code` holds the project's plain files and never a `.git`: `workspace/` is one git repository, and a nested one is stored as a single opaque entry. `af doctor` reports a `.git` under `code/`. |
| Scaffold files | [`scaffold/`](scaffold/) | Every file under it is copied into the project at the same path, never over an existing file. `scaffold/code/.gitignore` keeps dependencies, builds and secrets out of git. |
| Learnings log | `learnings: true` | `new-project` creates `workspace/learnings.md` when it is absent, in the shape of [`learnings.md`](../../schema/learnings.md). |
| Verb applicability | `verbs` above | Only `doctor`; code is not a versioned deliverable. |
| Production runbook | [`production.md`](production.md) | The loop a `project/<slug>` chat runs: plan, build, review, test, save. |
