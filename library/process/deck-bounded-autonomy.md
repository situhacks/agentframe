# Deck Work Inside A Bounded-Autonomy Run

## Purpose

Own what changes about deck production when an approved bounded-autonomy run drives it: the rendered visual-QA obligation before a checkpoint, and the noninteractive front-loaded confirmation that replaces PPT Master's interactive gate. [`bounded-autonomy.md`](bounded-autonomy.md) owns run authority, checkpoints, and stop rules; this file owns only the deck-shaped additions to them.

## When To Load

Load when an approved bounded-autonomy run produces or revises deck output — that is, when [`deck-production.md`](deck-production.md) has selected a route and the work is running under a sealed run contract rather than an interactive session. Ordinary interactive deck work never needs this file.

## Procedure

### Rendered visual QA before checkpoint

When a run's `goal` or `done_when` depends on deck appearance, the approved run contract counts as an explicit request for route-appropriate rendered visual QA. The operator does not need to repeat tool-specific invocation wording.

1. Run the selected route's static and structural checks.
2. Render and inspect the complete current deck through its actual viewing surface. For PPT Master, invoke its visual-review workflow after the SVG checker, then inspect a render of the exported PPTX so conversion defects remain visible. Render the exported PPTX natively per [`native-office-render.md`](native-office-render.md) — the SVG stage is not the PPTX, and a LibreOffice render of it would invent defects that are not in the file.
3. Fix the findings, rerender the affected slides, and inspect them again. A successful export or a first-pass claim of "no issues" is not completion evidence.
4. Record the render/review artifact paths and a concise finding -> fix -> recheck result in the autonomy evidence. If a finding requires a decision outside the approved charter, checkpoint with exact outcome `blocked` instead of asking during the run.

### Front-loaded PPT Master confirmation

The normal vendor confirmation gate remains the default. A repo-contained PPT project may use a noninteractive gate only when an approved run front-loads the complete confirmation:

1. Put every Step 4 content source, required `analysis/` identity/slide-library file, template/brand input, and the future sealed wrapper path in both the run's `context_sources` and `frozen_context`. Copy external inputs into the repo first. A named design language puts its `package/` root in both, because an identity the run cannot read is an identity it will invent.
2. Restrict `allowed_paths` to the mutable PPT project. It may not equal or contain the sibling sealed wrapper.
3. Write `<ppt-project>/agentframe-confirmation.draft.json` using the current final-result shape owned by the vendor's `scripts/docs/confirm_ui.md`. Use `mode: fixed-values` for exact operator-approved choices. Use `mode: delegate-strategist` only with exact delegation `{ "fields": "all", "constraints": {} }`; partial or constrained delegation uses the normal UI.
4. Before starting the run, seal it:

```powershell
python system/tools/ppt_master_contract.py seal <ppt-project> `
  --run <run-file> --draft <draft-json>
python system/af.py autonomy start <project> <run-id> `
  --session-binding <harness>:<session-id>
```

The run-unique sibling is `<deck-name>.<run-id>.agentframe-confirmation.json`. It records the pinned vendor commit, input hashes, approval record, and exact final result. The seal is immutable; `by: operator` is a durable record, not cryptographic authentication.

On the vendor's recognized confirmation waits (`stage1` and `final` since vendor `52e85a0`), `ppt_master_guard.py` validates the wrapper, input closure, run hash, and exact session, then blocks the redundant UI launch with the materialize command. Run that command and continue after the gate:

```powershell
python system/tools/ppt_master_contract.py materialize <sealed-wrapper> `
  --session-binding <harness>:<session-id>
```

## Verification Or Logging

Export revalidates the complete closure and exact materialized result. Any declared but malformed, drifted, ambiguous, or wrong-session contract blocks the run. No wrapper means the ordinary vendor UI proceeds.

Visual-QA evidence goes in the run's autonomy evidence, never in `activity.md`.

## Boundaries

- `C:\tmp`, untrusted or missing hooks, and Cursor unattended runs do not claim the noninteractive-gate guarantee.
- Route selection, the design-language handoff, and versioning/round-trip belong to [`deck-production.md`](deck-production.md).
- Run authority, checkpoints, model routing, and stop rules belong to [`bounded-autonomy.md`](bounded-autonomy.md).
- This file never authorizes a run; it describes deck obligations inside one already approved.
