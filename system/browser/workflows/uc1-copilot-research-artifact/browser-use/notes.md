# Browser Use Notes

Load this only when running UC1 with local browser-harness.

## Interaction Policy

- Inspect accessible controls, labels, focused element state, URLs, tabs, and targeted DOM before using screenshots.
- Use screenshots to confirm state or resolve ambiguity, not as the default way to choose every click.
- Use coordinate clicks only when browser chrome, canvas-like surfaces, or unreachable controls make semantic interaction impractical.
- Avoid full-page text reads on Copilot/Word. Use targeted control checks and small DOM queries.

## Copilot Researcher

- Stop at Microsoft sign-in, MFA, authenticator, permissions, or compliance prompts. Do not type or store credentials.
- In the first minute after prompt submission, watch for `Short` / `Long` choices or research-direction follow-up questions. Choose `Long`.
- If Copilot asks follow-up research-direction questions, follow the saved intake policy: answer from context when delegated, otherwise pause and ask the operator once.
- During generation, the composer stop-square means Researcher is still writing. Do not treat that as hung.
- Use a long overall generation budget, but keep each check targeted. Inspect the composer action control state, not `document.body.innerText`.
- Completion is composer idle plus `Convert to Word` visible near the final answer.
- If browser-harness times out while Copilot is visibly busy, reconnect/reload the harness once and continue from the same tab before failing the run.

## Word Export

- Use Copilot's `Convert to Word` control as the export path.
- In Word for the web, use `File` -> `Create a Copy` -> `Download a copy`.
- If Word asks `Do you want to download a copy of this file and work offline?`, click that prompt's `Download a copy`.
- If Word's right-side Copilot pane says `Service unavailable`, dismiss or ignore it. That pane is not part of the document export path.
- If Edge leaves the Word export as an `Unconfirmed *.crdownload`, open `edge://downloads` and click `Save` on the pending `Document *.docx` item. Treat this Save approval as expected for this tenant before treating the download as failed.
- If Copilot renders export content instead of creating/opening a Word file, pause and ask the operator whether to save manually, retry export, or capture the rendered text as the artifact source.

Record only durable quirks here. Do not use this file as routine run history.
