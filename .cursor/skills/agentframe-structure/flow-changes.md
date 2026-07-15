# Campaign Flow Changes

Use this when adding, renaming, retiring, or changing the default campaign flow.

## Read First

- `library/process/flows/README.md`
- Existing flow files in `library/process/flows/`
- `library/process/flow-authoring.md`
- `system/af.py` — the `FLOWS` dict (flow id → initial phase)
- The domain pack `flows:` list (`library/domains/{domain}/pack.md`) for a domain-scoped flow
- Any starter campaign scaffold that claims a default flow
- `library/process/project-frontmatter.md` if the change affects `flow`

## Procedure

1. **Classify the change:** new flow, rename, default change, phase-system edit, or retirement.
2. **Check overlap:** compare against existing flows. If an existing flow covers 70%+ of the job, prefer extending it or documenting a branch condition inside it.
3. **Choose owner:** phase sequence and flow-specific tracker transitions live in the flow file. Shared mechanics stay in `library/process/*.md`.
4. **Update registry + wire-ups:** `flows/README.md` owns available flows and default selection. A new flow id must also be added to the `FLOWS` dict in `system/af.py` (flow id → initial phase) — without it, `new-project --flow <id>` raises `KeyError`. For a domain-scoped flow, add the id to the domain pack's `flows:` list (`library/domains/{domain}/pack.md`).
5. **Update campaign selector rules:** `project.md` frontmatter owns the selected flow for each campaign instance via `flow`.
6. **Plan compatibility:** keep a pointer when renaming a loaded path, or update all live references in the same change. The pointer is not canonical; it only prevents old references from dead-ending.
7. **Verify:** search for stale references that treat one flow as the only flow or bypass `flow`.
8. **Log:** append a `system_changes` row with default-flow impact and compatibility notes.

## Refusal Checks

- Do not define a new flow from a single one-off campaign unless the operator explicitly wants it as a product surface.
- Do not duplicate deliverable template details inside a flow. Link to templates.
- Do not duplicate shared procedures such as lock events, frontmatter schema, Composio notes, or voice mini-retros. Link to process files.
