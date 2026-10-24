# Deliverable Template Changes

Use this when adding, retiring, or materially reshaping a deliverable type.

## Read First

- `library/deliverables/_meta/template-authoring.md`
- Existing `library/deliverables/*/template.md` files relevant to the proposed type
- `system/skills/deliverable-scaffolding/SKILL.md` when adding a new deliverable type

## Procedure

1. **Name the deliverable job:** what decision, output, or handoff does this template make reusable?
2. **Locate before inventing:** compare with existing deliverable families. Refuse a new type at 70%+ overlap unless the operator approves the taxonomy cost.
3. **Pick the change path:**
   - New deliverable type: run `deliverable-scaffolding`.
   - Existing deliverable patch: edit the template under the Builder pre-write gate.
   - Retiring a deliverable type: handle as a bespoke Builder decision; the commit's `MIGRATION:` line names what replaces it.
4. **Keep ownership clean:** templates own artifact shape and hard constraints; the project plan owns phase sequence; process files own reusable procedures. Size budget: 900 words (`af doctor` flags overages) — over budget, cut before adding.
5. **Verify:** confirm the template can be discovered by pack, catalog, or deliverable pointer.

## Refusal Checks

- Do not create a deliverable type for a one-off campaign artifact.
- Do not add workflow runbooks to templates when a process file owns the procedure.
- Do not preserve provenance or changelog prose in the runtime template.
