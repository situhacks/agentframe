# Code production

The runbook a `project/<slug>` chat loads for a project with `domain: code`. The chat plans and reviews; a builder writes the code.

## Where things are

- The code is in `workspace/projects/<slug>/code/`. The plan and decisions are in `project.md`; events are in `activity.md`.
- `code/` holds plain files and never a `.git`. Do not run `git init` or `git clone` there: `af save` would store the nested repository as one entry, and the diff in the loop would show nothing. To bring in an existing repository, copy its files without `.git`; its history stays where it was. `af doctor` reports a `.git` under `code/`.
- His life layer is read with `af search` (his notes, people, prior projects). It is changed only through `af-main`.

## The loop

1. Plan on Opus or Fable by [`models.md`](../../schema/models.md). Fill the four stubs in `project.md` at kickoff: Goal, Stack, Test command, Done when.
2. Run `af save`, so `git -C workspace diff -- projects/<slug>/code` shows exactly what the builder changes.
3. Start one builder: a general-purpose agent on Sonnet. Do not use the `builder` agent type; it is for tracked system files. Run one builder at a time.
4. The builder runs the project's Test command. It must pass before review.
5. The `reviewer` agent reviews the diff adversarially.
6. Review findings go back to the same builder.
7. The chat runs the Test command itself, then runs `af save`.

A build that fails its tests or its review is rebuilt on Opus, then on Fable ([`models.md`](../../schema/models.md)).

## Private by default

When the chat briefs the reviewer, it says the code is private. It adds "Hydrogen allowed" only if the chat's own brief carries that line. Only he decides it, project by project.

## Learnings

- Before changing a path, run `af search "<path or module>"`. It finds entries in both `workspace/learnings.md` and `system/learnings.md`. Read the entries whose `Watch:` line matches.
- A mistake that came back, or cost a real failure, gets an entry in `workspace/learnings.md`, in the shape of [`learnings.md`](../../schema/learnings.md), Guard first.
- Write each entry so any project can use it. Name the stack or the tool, not only this project.
- `Guard:` and `Watch:` paths in `workspace/learnings.md` start at the vault root, for example `workspace/projects/<slug>/code/tests/test_x.py`, because the harness review resolves them there.
- A lesson about AgentFrame itself goes to `system/backlog.md`, not to the learnings log.

## Starting the chat

Copy this brief into `system/sessions/project/<slug>/brief.md`. `desk` starts the chat.

```
---
goal: <one line: what this project builds>
done_when: <its Done when line>
write_surface: [workspace/projects/<slug>/]
model: opus
class: build
---
Project: <slug>
```

Add the line `Hydrogen allowed` to the body only if he said so for this project.
