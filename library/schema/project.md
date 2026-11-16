# Project

Shapes `workspace/projects/<slug>/`, one bounded piece of work. `af new-project` creates it; the `af` buttons move it; [`project-flow.md`](../process/project-flow.md) runs it from kickoff to closeout.

```
workspace/projects/<slug>/
  project.md              # frontmatter index + plan body
  activity.md             # material events, one line each
  sources/                # immutable inputs
  knowledge/              # compiled pages
  <name>-v1.md            # prior versions, immutable
  <name>-v2.md            # head: the tracker row points here
  _local/<type>/<type>-v1.md   # a project-only deliverable type
```

- **Create** with `af new-project <slug> --domain <pack>` (`project-mgmt` is the neutral default). It writes `project.md`, `activity.md`, `sources/`, and `knowledge/`. Never make the folder by hand: a folder under `workspace/projects/` with no `project.md` is not a project, no button or guard can see it, and `af doctor` reports it.
- **A code project** is `af new-project <slug> --domain code`: the code lives in `code/`, and the [`code` pack's `production.md`](../domains/code/production.md) is the loop its chat runs.
- **`project.md`** frontmatter is the index: identity, lifecycle, and the `deliverables` tracker, one row per deliverable whose `file` points at its head. The field table lives in [`project-frontmatter.md`](../process/project-frontmatter.md) and nowhere else. **`activity.md`** follows [`project-activity.md`](../process/project-activity.md).
- **Start a deliverable** with `af draft <slug> <row> --file <name>-v1.md`; a draft already written elsewhere is registered with `af adopt`. Every content change cuts the next version first with `af version`, which copies the head to `-v{N+1}` and moves the row; an editable copy for the operator is the same move. Rules: [`deliverable-versioning.md`](../process/deliverable-versioning.md).
- **The chain is the history.** Each head after v1 carries `changes_from_v{N-1}`: the operator's feedback, the direction change, or what he edited by hand. A prior version is never edited. A rejected draft is an earlier version whose rejection the next one records; never a separately named file.
- Only scratchpads are throwaway, and they carry `scratchpad` in the filename. `af doctor` notes any other `status: drafting` file that is not named `{name}-v{N}.md` and that no row points at.
