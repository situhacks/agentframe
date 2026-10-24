# Authoring Standard

## Purpose

One standard for the runtime files Builder writes most: process files in `library/process/` and deliverable templates. It owns their shapes, where each lives, how a new deliverable type is born, and the size caps `af doctor` checks.

## When To Load

Creating, materially reshaping, or retiring a process file or a deliverable template, or adding a deliverable type. A one-line patch needs only the pre-write gate in `AGENTS.builder.md`.

## Owners

- `library/process/*.md`: reusable procedures, loaded when a situation needs them.
- `template.md` under a pack's or the shared `deliverables/`: artifact shape and hard constraints.
- A pack's `production.md`: that domain's phase checklist. [`project-flow.md`](project-flow.md): the one project shape.
- `AGENTS*.md`: routes and cross-cutting invariants only.
- `.claude/skills/`: a reusable technique that a second project reaches for.

Extend the file that already owns the job before adding another. A new file that would repeat most of an existing one becomes a section or a branch of it.

## Process Files

Sections, in order:

1. `Purpose`: the procedure this file owns.
2. `When To Load`: the state or intent that triggers it; quoted phrases only as secondary examples.
3. `Procedure`: ordered steps an agent can execute.
4. `Verification`: how the agent proves the outcome.
5. `Boundaries`: what this file does not own.

A new, reshaped, or retired process file updates its row in [`README.md`](README.md) (what it owns, when to load) in the same change, and names the parent that loads it at the right moment. A file nothing loads is dead.

## Deliverable Templates

The template sections and the instance shape live in [`deliverable-shape.md`](../deliverables/_meta/deliverable-shape.md); write against it, not against another template's headings. A section earns its place only when it changes what an agent loads, writes, verifies, or asks the operator. A section that invites factual claims names its source layer: the research artifact first, a light web check for a time-sensitive fact, model prior only for structural reasoning.

A template lives at the first level that fits, and resolution reads the levels in the same order: the pack (`library/domains/{pack}/deliverables/{type}/`), shared (`library/deliverables/{type}/`), the project's `_local/{type}/`, then the generic shape with no template.

## A New Deliverable Type

1. Check the pack and shared templates, and extend one that covers most of the job.
2. Make the `{type}/` folder at the level that fits and write its `template.md` from the shape.
3. Add the type to the pack's `pack.md` deliverable row where the pack keeps one.
4. Confirm the chain above resolves it from a project in that domain.

No skill is involved. The first real draft is the template's test; a template the draft ignores gets reshaped, not reworded.

## Size Caps

`af doctor` notes a file over its cap: 1,700 words for `AGENTS.builder.md` and `AGENTS.operator.md`, 1,500 for a process file, 900 for a template. Over the cap, cut before adding.

## Every Runtime File

- Runtime prose is present-tense instruction for an agent and carries no history, dates, or account of what changed or why.
- A change that retires, renames, or moves a name sweeps every reference to it in the same change.

## Boundaries

This file does not own a template's content, the project shape ([`project-flow.md`](project-flow.md)), or the frontmatter schema ([`project-frontmatter.md`](project-frontmatter.md)).
