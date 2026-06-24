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

## Template resolution

When drafting deliverable `X` for a project with `domain: D`, the template resolves in order:

1. **Domain pack** — `library/domains/{D}/deliverables/X/template.md` (the domain-specific template, if the pack ships one).
2. **Shared** — `library/deliverables/X/template.md` (a cross-domain deliverable, e.g. design-language, video-spec, image-prompts, closeout-retro, system-retro).
3. **This generic shape** — no type-specific template exists; author the instance ad-hoc against the shape above.

The pack's template (when present) layers over this shape; the shape itself is never forked per domain. A flow points to "draft the deliverable using its template"; resolution does the lookup — it is not the flow's or the persona's job to hard-code a path.
