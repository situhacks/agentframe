# Career Harvest

Move real wins into the career bank while the evidence is fresh. The bank (`library/context/operator/career/`, schema at `library/context/operator-schema/career/`) feeds resumes, promotion cases, and "recent wins" writeups; a win that never lands there is lost to the next application.

## When to load

- A project closeout or system retro runs — harvest is a closing step, sibling of `deliverable-harvest`.
- Ad hoc: the operator names a win, ships a milestone, or asks to update the resume/bank.
- An interview round finishes — the playbook pass below runs even when the round produced no bankable win.

## Procedure

1. **Identify the material wins** in the retro or named source: shipped outcomes, decisions that paid off, metrics moved. Skip routine execution.
2. **Bank each win at the layers it earns** (one fact, one home):
   - `proof-points.md` — the verified number/link, with source and date. Numbers live only here; the other layers cite the proof-point ID.
   - `master-cv.md` — a resume-ready CDO bullet (context → decision/trade-off → quantified outcome) under the right role, citing the proof-point.
   - `stories/{slug}.md` — only when the win carries a narrative arc (conflict, decision, result, reflection). Line 1 is the ROSTER line: `ROSTER: <competencies> | <one-line hook>`.
   - `employers/{slug}.md` - when the win changes standing against the current rubric, or the source states a new expectation, append a dated timeline line (the evidence itself still lands in the layers above).
3. **Link origin.** Every entry carries `origin:` (project slug or external source) so a future resume bullet traces back to the work that produced it.
4. **Prune while there.** Mark superseded bullets/stories rather than deleting; the bank is append-heavy and occasionally curated.

## After an interview round

A separate, smaller pass with its own destination: `career/interview-playbook.md`. Run it while the call is fresh, because this is the step whose absence lets two rounds derive the same failure independently.

1. **Keep the per-call detail in the application folder.** The post-mortem belongs beside that application's `interview-prep`, not in the bank.
2. **Append one dated block to the playbook** in the shape its "After every round" section defines: the pattern that repeated, any new do-not-say phrase in the operator's verbatim words, and which drill-set row worked or was missing.
3. **Only what repeats is promoted.** A one-off stumble stays in the application folder. A pattern seen in two rounds becomes a numbered playbook entry with a `Fix:` line.
4. **Promote a new drill-set opener** when the round surfaced a question theme the playbook had no opener for, citing the `pp-NNN` it rests on. Bank the underlying number through the main procedure above first if it is not already there.
5. **Prune.** Drop a pattern that two consecutive rounds have not reproduced, so the file stays short enough to actually re-read before a round.

Any genuine win the round revealed still goes through the main procedure; the two passes are independent.

## Boundaries

- Operator-owned content work; no system files change.
- Bank instances are personal (gitignored); only the schema mirrors under `library/context/operator-schema/career/` are tracked.
- Proof-points record what the source supports — never rewrite history to sound better.
