---
name: manage-lenses
description: Create or mutate source-backed lens packages under library/lenses from supplied text, URLs, channels, profiles, or research. Use only when the operator explicitly asks to build, ingest sources into, refresh, rebuild, version, approve or activate, retire, or export a lens. Do not use to apply an existing active lens, continue an active lens, provide general advice, or research a person unless the requested outcome is a lens package.
---

# Manage Lenses

Create and maintain attributed advisory models without turning their source material into operator truth. This skill changes a lens package; [`library/process/lens-use.md`](../../../library/process/lens-use.md) owns using one.

## Start

1. Read [`library/lenses/README.md`](../../../library/lenses/README.md) for the package contract.
2. Confirm the requested outcome is a new or changed lens. If the operator only wants advice, person research, a lens list, or an existing lens applied, stop and use the appropriate route instead.
3. Resolve one target slug. Inspect only that package if it exists; do not scan unrelated lenses.
4. Read [`references/source-adapters.md`](references/source-adapters.md), then choose only the acquisition routes needed by the named inputs.

## Build Or Update

1. **Acquire deliberately.** Prefer supplied text, transcripts, and files. Record every used source in `sources/INDEX.md`, including how it was acquired, what was actually accessible, and any rights or coverage limit. A URL or channel is a discovery seed, not evidence by itself.
2. **Extract evidence.** Write proposition-level entries in `evidence.md`. Give each entry a stable ID, source IDs, support or contradiction, confidence, and limits. Separate sourced claims from interpretation.
3. **Compile the lens.** Build `lens.md` from the evidence register: the subject's recurring principles, diagnostic questions, decision patterns, tensions, characteristic advice, and known boundaries. Keep it useful for reasoning, not biographical.
4. **Version intentionally.** Create version `1.0.0` with `status: draft` for a new lens. Before a refresh replaces `lens.md`, copy the current file byte-for-byte to `_archive/lens-v{current-version}.md`; if that path already contains different bytes, stop and reconcile instead of overwriting it. Preserve or explicitly supersede evidence entries, advance the version, update the source cutoff, summarize material changes, and set the refreshed runtime file to `draft` regardless of its prior status. Retire without deleting the evidence trail.
5. **Verify and approve.** Confirm the required files exist, every material claim in `lens.md` resolves to evidence IDs, each evidence ID resolves to indexed sources, and unsupported source access is not implied. Surface coverage gaps and limits. Keep every new or refreshed version `draft` until the operator explicitly accepts that version as usable, then change it to `active`.
6. **Export narrowly.** Export only the named package and its provenance. Exclude operator context and any raw third-party material whose rights do not permit redistribution.

Package approval is not project activation: never write `active_lens` state as part of this skill. After a build or mutation, report the slug, version, status, source cutoff, important limitations, and whether the operator separately asked to apply it.

## Boundaries

- Never silently create a lens from ordinary research or advice work.
- Never treat a lens as the operator's beliefs, preferences, identity, or verified fact.
- Do not promise complete channel scraping, private-platform access, or transcript access that was not obtained.
- Do not duplicate full third-party works. Preserve supplied or licensed raw material when appropriate; otherwise retain source metadata, bounded notes, and evidence.
- Add `research/`, raw-source subfolders, or archives only when the material earns them. The three-file package is the default.
