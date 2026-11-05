---
name: catchup
description: Catch-up after time away. Writes the zero-token brief, has the briefer add the week's judgment against the quarter plan, then walks every due loop to resolve, re-date, keep as his to do, or move to the needs list.
disable-model-invocation: true
---

Catch-up mode. The operator is back after time away and wants to know what is outstanding. Do this in order:

1. From the vault root run `af brief --write`. Keep its output and the path it prints after `saved to`.
2. Dispatch the `briefer` role. Its brief: "`af brief --write` already ran; its output is below and saved at `<path>`, so do not run it again. Read the current quarter's plan in `workspace/life/plans/` and return the brief per your role." Paste the output after that line.
3. Show the briefer's text as it returns. Add nothing to it.
4. Run `af open due --json`. If it is empty, say there are no due loops and stop. Otherwise list each due loop numbered, with its text, age, and source, and ask in prose with lettered options for each:
   - **a. Resolve**: he says the outcome in a few words.
   - **b. Re-date**: he gives a date or `+Nd`.
   - **c. Do**: he still has to do it; it stays due and listed under Do in `workspace/life/needs.md` until he does. A loop under `Waiting on others` stays due but out of Do; main chases it.
   - **d. Decide later**: it needs his decision, not his hands; it moves to the needs list.

   He can answer all at once (`1a paid it, 2b +7d, 3c`). Nothing else happens in this chat until he answers: no other work, no new loops, no edits.
5. Apply his answers with the `af open` verbs, addressing each loop by a text prefix from the JSON, since loop numbers shift after every close:
   - Resolve: `af open close "<text prefix>" --outcome "<his words>"`. Closing a recurring loop (one with `every` in the JSON) re-dates it a period on; add `--final` when he says it is over for good.
   - Re-date: `af open redate "<text prefix>" --nudge <date or +Nd>`; the loop keeps its opened date, text, section, and source.
   - Do: no command.
   - Decide later: write the detail file per [`needs-list.md`](../../../library/schema/needs-list.md), run `af needs add "<question>" --detail needs/<slug>.md`, then close the plain loop with the outcome `moved to the needs list`.
   - A loop whose JSON carries `detail` is already on the needs list: resolve it with `af needs close`, re-date it with `af needs move`, and offer no `d`.

   If a close refuses an ambiguous prefix, rerun it with a longer prefix. Never edit `open.md` by hand.
6. Run the visit sweep: list the past seven days of his calendar through the Calendar connector. For each event whose title or location names a page under `workspace/life/places/` (`af place list`), read that page's `## Visits`. Show the events with no row for that date as a lettered list and ask which happened. For each yes, run `af place visit <slug> --date <date> --what "<title>" --from catchup`, which writes the row and raises the verdict loop. With no connector, or nothing to add, say so in one line. Group chats stay out of this.
7. Run `af open due` and report in one line what is still due, then hand the chat back to him.

Never close or re-date a loop he did not answer for. A loop he resolves with a fact worth keeping (a decision, a date, a person's answer) is a candidate for `/file order`; offer it in one line, do not run it.
