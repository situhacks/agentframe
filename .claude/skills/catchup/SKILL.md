---
name: catchup
description: Catch-up after time away. Writes the zero-token brief, has the briefer add the week's judgment against the quarter plan, then walks every due loop to resolve, re-date, or leave.
disable-model-invocation: true
---

Catch-up mode. The operator is back after time away and wants to know what is outstanding. Do this in order:

1. From the vault root run `af brief --write`. Keep its output and the path it prints after `saved to`.
2. Dispatch the `briefer` role. Its brief: "`af brief --write` already ran; its output is below and saved at `<path>`, so do not run it again. Read the current quarter's plan in `workspace/life/plans/` and return the brief per your role." Paste the output after that line.
3. Show the briefer's text as it returns. Add nothing to it.
4. Run `af open due --json`. If it is empty, say there are no due loops and stop. Otherwise list each due loop numbered, with its text, age, and source, and ask in prose with lettered options for each:
   - **a. Resolve**: he says the outcome in a few words.
   - **b. Re-date**: he gives a date or `+Nd`.
   - **c. Leave**: it stays as it is and nudges again at the next session start.

   He can answer all at once (`1a paid it, 2b +7d, 3c`). Nothing else happens in this chat until he answers: no other work, no new loops, no edits.
5. Apply his answers with the `af open` verbs, addressing each loop by a text prefix from the JSON, since loop numbers shift after every close:
   - Resolve: `af open close "<text prefix>" --outcome "<his words>"`.
   - Re-date: `af open close "<text prefix>" --outcome "re-dated to <date>"`, then `af open add "<same text>" --nudge <date> --from <same from> --section "<same section>"`.
   - Leave: no command.

   If a close refuses an ambiguous prefix, rerun it with a longer prefix. Never edit `open.md` by hand.
6. Run `af open due` and report in one line what is still due, then hand the chat back to him.

Never close or re-date a loop he did not answer for. A loop he resolves with a fact worth keeping (a decision, a date, a person's answer) is a candidate for `/file order`; offer it in one line, do not run it.
