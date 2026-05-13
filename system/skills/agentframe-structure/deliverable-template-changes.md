# Deliverable Template Changes

Use this when adding, retiring, or materially reshaping a deliverable type.

## Read First

- `library/deliverables/_meta/template-authoring.md`
- Existing `library/deliverables/*/template.md` files relevant to the proposed type
- `system/skills/deliverable-scaffolding/SKILL.md` when adding a new deliverable type
- `system/skills/system-improvement/SKILL.md` when patching an existing template

## Procedure

1. **Name the deliverable job:** what decision, output, or handoff does this template make reusable?
2. **Locate before inventing:** compare with existing deliverable families. Refuse a new type at 70%+ overlap unless the operator approves the taxonomy cost.
3. **Pick the change path:**
   - New deliverable type: run `deliverable-scaffolding`.
   - Existing deliverable patch: run `system-improvement`.
   - Retiring a deliverable type: handle as a bespoke Builder decision and log the migration plan.
4. **Keep ownership clean:** templates own artifact shape and hard constraints; campaign flows own phase sequence; process files own reusable procedures.
5. **Verify:** confirm the template can be discovered by registry, flow, or deliverable pointer.
6. **Log:** append `system_changes` rows for scaffold, patch, retirement, or wire-up.

## Refusal Checks

- Do not create a deliverable type for a one-off campaign artifact.
- Do not add workflow runbooks to templates when a process file owns the procedure.
- Do not preserve provenance or changelog prose in the runtime template.
