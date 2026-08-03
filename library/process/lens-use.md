# Lens Use

## Purpose

Own explicit listing, selection, application, and resumption of source-backed lenses. Lenses are attributed advisory models, not operator context or ambient personas.

## When To Load

Load when the operator explicitly asks to create, change, list, choose, compare, or apply a lens, or when the active project's body contains an `active_lens` pointer whose `lens_scope` covers the current work. Do not load for general advice or merely because lenses exist.

## Procedure

1. **Route package mutation.** If the requested outcome builds, ingests into, refreshes, rebuilds, versions, approves or activates, retires, or exports a lens, load the mutation-only `manage-lenses` skill through `system/skills/README.md` and stop this runtime procedure.
2. **Resolve deliberately.**
   - For an explicit list request, enumerate lens package directories and read only the minimum metadata needed to identify them. Do not preload lens bodies or evidence.
   - For a named lens without a version, resolve exactly `library/lenses/{slug}/lens.md`. Apply only `status: active`; route draft activation or package changes to `manage-lenses`, and do not apply a retired package.
   - For `{slug}@{version}`, including resumed project work, first read current `lens.md`. Stop if the package is retired. Use it only when its declared version matches the pin; otherwise load exactly `_archive/lens-v{version}.md`. Require the resolved file to declare the pinned version and `status: active`. If it is missing or mismatched, surface drift and stop rather than substituting the latest lens.
3. **Apply one-shot by default.** When the request is limited to the current question, use the exact lens file resolved in Step 2, name the lens and version, and persist no state.
4. **Persist only an explicit sustained request.** In a project-backed workstream, add a small body block to `project.md`:

   ```markdown
   - active_lens: {slug}@{version}
   - lens_scope: {specific question, workstream, or project}
   ```

   This is a runtime pointer, not frontmatter schema. Without a project, the operator must name the lens again in a later conversation.
5. **Rehydrate from disk.** After compaction or in any resumed context, if the scope still applies, resolve the pinned version again through Step 2; do not rely on a conversational summary or silently advance it. Load `evidence.md` only when explaining, citing, comparing, disputing, or refreshing a claim.
6. **Switch or end cleanly.** On an explicit switch, replace both pointer lines. When the operator ends sustained use or its scope is complete, remove both.
7. **Apply precedence.** Operator instructions, verified facts, and the active project objective outrank lens advice. Name material conflict with operator positioning, judgment, or another lens instead of blending them into false consensus.

## Verification Or Logging

State the applied slug, version, and scope. When evidence was needed, cite its IDs and ensure they resolve to `sources/INDEX.md`. No system audit entry is required for ordinary lens use.

## Boundaries

- Never scan, preload, or suggest unrelated lenses ambiently.
- Never mutate or refresh a package here; use `manage-lenses`.
- Never persist a lens merely because it helped once.
- A lens informs judgment; it does not impersonate its subject or override safety and factual verification.
