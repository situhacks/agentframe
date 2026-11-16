# Re-Distill

## Purpose

Owns rebuilding a written rule, skill, router section or model card from the material it was first drawn from, when a newer model makes the old text a weaker guide than a fresh one. His rule: "you should never be married to one or two to a process because in a few months, when new models get released and models get smarter, your written prompts and skills and stuff become more and more useless." The written rules are derived; the origin material is the truth.

## When To Load

- The `checked` date of [`models.md`](../schema/models.md) is over 60 days old, or a new model has landed.
- He says to re-distill an area.
- A harness review flags a skill, router section or card nothing needs any more.

## Procedure

The origin material:

- His orders and instructions: `workspace/life/sources/orders/`, verbatim and immutable. Other inputs: `workspace/life/sources/`.
- Chat files: `workspace/_archive/transcripts/<Claude Code project folder>/<path>.gz`, gzip copies of everything in the vault's Claude Code project folders (transcripts, `subagents/`, the saved `tool-results/` a transcript points at) that `system/tools/transcript_archive.py` makes each morning before Claude Code deletes the originals. A source that shrank keeps its old copy, with the new content beside it as `<name>.<yyyymmddHHMMSS>.gz`. The batch's own temp-folder runs (`...Temp-agentframe-*`) are left out on purpose. `af search` does not index the archive. Read one file through a `reader` agent or `gzip -dc`.
- Deliverables: every `af version` of a project file under `workspace/projects/<slug>/`.
- His voice: the corpus under `workspace/life/voice/`.

To re-distill one area at a time (voice, a model card, a skill, a router section):

1. Name the area and what it must do for him.
2. A builder reads the origin material for that area and drafts the rule or skill fresh, not by editing the old text.
3. Compare old and new. Keep each line only if the new model needs it.
4. He judges anything in his voice.
5. Delete what the new model no longer needs; the harness review's job too.

## Verification

- The new text passes the checks of the file it replaces (`af doctor`, the size caps in [`authoring.md`](authoring.md), the tests).
- Each rule it keeps traces to an order, a transcript or a version, not to the old text.
- Nothing in the origin material was edited or deleted.

## Boundaries

- This file does not change Claude Code's clean-up period; raising `cleanupPeriodDays` is his option, in his own settings.
- It does not index or summarize transcripts.
- The model rule and the model facts stay in [`models.md`](../schema/models.md).
