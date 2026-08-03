# `library/lenses/`

Private, source-backed advisory models live here. A lens represents an attributed way of interpreting or advising; it is not operator context, a generic skill, or a verified account of the world.

Lens instances are local/private. This README is the committed contract; each `{slug}/` package is ignored by Git.

## Minimal Package

```text
library/lenses/{slug}/
  lens.md
  evidence.md
  sources/
    INDEX.md
```

Do not prebuild topic folders. Add raw-source files, `research/`, or `_archive/` only when the material earns them.

### `lens.md`

The compact runtime model. Start with:

```yaml
---
name: Human-readable name
slug: stable-kebab-case
version: 1.0.0
status: draft
source_cutoff: YYYY-MM-DD
updated_at: YYYY-MM-DD
---
```

Cover the subject's recurring principles, diagnostic questions, decision patterns, tensions, characteristic advice, and limits. Cite evidence IDs beside material claims. Aim for roughly 800–1,500 words; split only when repeated use proves that smaller routed modules are needed.

Versions are semantic: patch for clarifications with unchanged substance, minor for newly supported ideas or meaningful source additions, and major for a materially changed model. New and refreshed versions remain `draft` until the operator accepts their coverage and limits; only `active` lenses are usable. Retirement sets `status: retired`; it does not erase provenance.

A refresh earns `_archive/`. Before replacing the runtime file, preserve it byte-for-byte as `_archive/lens-v{version}.md`; never overwrite a different archived copy. Supersede rather than silently delete evidence entries. This archive is also the exact resolution target for a sustained project pointer pinned to an older version.

### `evidence.md`

The proposition-level claim register behind the runtime model. Each entry needs:

- a stable evidence ID;
- the proposition or interpretation;
- supporting and contradicting source IDs;
- confidence and known limits; and
- enough notes to distinguish source meaning from compiler interpretation.

No material claim belongs in `lens.md` without a resolvable evidence entry. Conflicting evidence stays visible.

### `sources/INDEX.md`

The source ledger. Use one row per captured source:

| ID | Type | Creator / title / date | Canonical source | Captured | Acquisition | Coverage and access limits | Local file |
|---|---|---|---|---|---|---|---|

A channel, profile, search result, or inaccessible URL may be a discovery lead, but it is not content evidence. Record only access that actually occurred. Preserve supplied or licensed material when useful; do not duplicate full third-party works merely to populate a package.

## Runtime Rules

- Lens creation and mutation route through [`manage-lenses`](../../system/skills/manage-lenses/SKILL.md).
- Listing, selecting, applying, and resuming route through [`lens-use`](../process/lens-use.md).
- Load `lens.md` by default. Load `evidence.md` or source material only to explain, cite, compare, dispute, or refresh a claim.
- Honor a requested or persisted version exactly: use current `lens.md` only when its version matches, otherwise use `_archive/lens-v{version}.md`. Never silently substitute the latest version.
- A lens never silently becomes operator context. Operator instructions, verified facts, and the active project's objective outrank lens advice.
- Never scan or suggest the lens library ambiently. A named request or an in-scope project pointer is required.
