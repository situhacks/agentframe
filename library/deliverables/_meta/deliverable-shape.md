# The Deliverable Shape

The generic, domain-neutral shape every deliverable instance takes on disk, before any type-specific template. A specific deliverable (a findings report, a post-final) is an *instance* of this shape; its template adds the content sections — this defines the container.

## The shape

A deliverable is a **folder** (mirroring how skills and projects are folders):

```
<deliverable-slug>/
  <deliverable-slug>-v{N}.md   # canonical, versioned; the head (highest N) is tracked in project.md
  notes.md                     # OPTIONAL — deliverable-local working scratch
  assets/                      # OPTIONAL — deliverable-specific generated media
```

- **Canonical doc** — `<slug>-v{N}.md`, versioned per [`deliverable-versioning.md`](../../process/deliverable-versioning.md). The filename carries the version; the highest `v{N}` is the head. Frontmatter is `status` + `last_updated` only (plus any field the deliverable itself owns).
- **Raw inputs** live once in the project-level `sources/`; deliverables **link** to them, never copy.
- **Cross-deliverable context** (entities, decisions) lives in the project's `knowledge/`; deliverables **link**.
- Only deliverable-*specific* scratch/media sits in the deliverable folder.

## Template sections

A type's `template.md` uses this order unless the deliverable earns a different one:

1. `Purpose`: the decision, output, or handoff the deliverable creates.
2. `Inputs`: what is loaded or ready before drafting; name the source, never restate it.
3. `Output Shape`: the artifact the agent writes.
4. `Hard Constraints`: the non-negotiable rules.
5. `Draft Frontmatter Convention`: only fields this deliverable carries beyond `status` and `last_updated`.
6. `Readiness Criteria`: what is true before `status: ready`.

Optional, only when it changes what an agent does: `Before Writing` (the button and load steps for a draft or a change), `Clean Pass` (operator-voiced prose: the voice clean pass and `af voice lint <head>`), `Feedback / Handoff`, `Publish / Export Mechanics`, `Exceptions / Branches`. No placeholder section says a thing does not apply.

## Template resolution

When drafting deliverable `X` for a project with `domain: D`, the template resolves in order:

1. **Domain pack** — `library/domains/{D}/deliverables/X/template.md` (the domain-specific template, if the pack ships one).
2. **Shared** — `library/deliverables/X/template.md` (a cross-domain deliverable, e.g. design-language, video-spec, image-prompts).
3. **Local template** — `_local/X/` (if the operator wants a kept, versioned, project-scoped artifact that doesn't exist in the library; author the instance ad-hoc against the generic shape).
4. **This generic shape** — no type-specific template exists; author the instance ad-hoc against the shape above.

The pack's template (when present) layers over this shape; the shape itself is never forked per domain. A project plan points to "draft the deliverable using its template"; resolution does the lookup — it is not the plan's or the persona's job to hard-code a path.
