# UC1 Copilot Research Artifact

workflow_id: uc1-copilot-research-artifact
status: cursor_replay
app_url: https://m365.cloud.microsoft/chat
approval_mode: autonomous_when_whitelisted
tool_routing: browser_use_only

## Job

Collect UC1 intake, run Copilot Researcher, download the Word research artifact, save it as Phase 1 source material, generate the first Business Frame draft, then hand back to normal AgentFrame iteration.

## Inputs

- Active campaign slug.
- Intake file: `workspace/campaigns/{slug}/phase-1-research/intake.md`
- Source-material folder: `workspace/campaigns/{slug}/phase-1-research/source-material/`
- Business Frame draft: `workspace/campaigns/{slug}/phase-2-strategy/business-frame/draft-vF.md`
- Business Frame feedback folder: `workspace/campaigns/{slug}/phase-2-strategy/business-frame/feedback/`

Demo campaign: `workspace/campaigns/uc1-test/`.

## Intake Widget

Collect run inputs with one Cursor `AskQuestion` widget whenever possible. Include all option-based choices in the same widget:

1. `topic`
2. `sponsoring_service_line`
3. `target_audience` (allow multiple)
4. `objectives` (allow multiple)
5. `additional_context` with only the default `Nothing else / use boilerplate` and `Other / needs custom input`
6. `copilot_follow_up_policy`

Use inferred/default choices where the operator's launch request is clear. Because the widget does not support true free-text fields, include `Other / needs custom input` options for freeform fields; if selected, ask one compact follow-up in chat and then save the final values. Do not ask the five intake questions sequentially in chat.

Follow-up policy:

- `agent_answers_from_context`: if Copilot asks research-direction questions, answer using intake, campaign context, and known operator preferences.
- `pause_for_operator`: if Copilot asks research-direction questions, pause once, ask the operator, then continue.

Save the final intake to `phase-1-research/intake.md` before opening Copilot.

## Prompt Files

- Load `prompts/research-artifact-prompt.md`, fill placeholders from saved intake, then paste it into Copilot Chat with Researcher enabled.

`prompts/kx-internal-research-prompt.md` exists as an inactive experiment. Do not use Prompt by KX in the default UC1 path. For this workflow, rely on Copilot Researcher plus public Deloitte thought leadership/articles and operator-provided uploaded materials.

## Prompt Governance

Treat `prompts/research-artifact-prompt.md` as the canonical current workflow prompt template for the Researcher artifact. It is not a Business Frame deliverable template, but it still owns reusable prompt behavior for this workflow.

- Prompt-quality feedback changes `prompts/research-artifact-prompt.md` directly.
- Campaign-specific feedback on a generated research artifact stays with the campaign, usually in `feedback-log.md` or a source-material note, unless it reveals a reusable prompt defect.
- Business Frame feedback stays in the deliverable `feedback/` folder and does not automatically change the Researcher prompt.

## Path

1. Confirm the campaign folders exist. Do not overwrite an existing Business Frame draft unless this run is explicitly a regeneration.
2. Collect intake with the widget pattern above and save it.
3. Open or reuse the controlled Edge Work Browser at `https://m365.cloud.microsoft/chat`.
4. If not authenticated, run `_shared/auth-handoff/recipe.md` and return only when Copilot is ready.
5. Open or activate `Researcher`. Prefer the left sidebar `Researcher` agent under `Agents`; if Copilot already restored a Researcher conversation, continue there.
6. Fill `prompts/research-artifact-prompt.md` from intake, paste it into Copilot composer, and submit.
7. If Copilot asks for response length, choose `Long`.
8. If Copilot asks follow-up research-direction questions, use `copilot_follow_up_policy`.
9. Wait for the final answer. Treat Copilot as still writing until the composer is idle and `Convert to Word` appears.
10. Click `Convert to Word` from the final answer's export controls.
11. In Word for the web, download the document:
    - `File` -> `Create a Copy` -> `Download a copy`.
    - If Word asks `Do you want to download a copy of this file and work offline?`, click that prompt's `Download a copy`.
    - Ignore or dismiss Word's right-side Copilot pane errors if the document and download controls still work.
12. Copy the downloaded `.docx` from `Downloads` into `phase-1-research/source-material/` using `<Topic>.Research.Artifact.<YYYY-MM-DD>.docx`. If that file exists, add a run timestamp before `.docx`.
13. Extract markdown beside the `.docx`.
14. Generate the first Business Frame draft at `phase-2-strategy/business-frame/draft-vF.md`.
15. Update campaign activity/state and tell the operator the first draft is ready for normal AgentFrame iteration.

## Output Contract

- Research artifact: `phase-1-research/source-material/<Topic>.Research.Artifact.<YYYY-MM-DD>.docx`
- Extracted artifact: same basename with `.extracted.md`
- Business Frame draft: `phase-2-strategy/business-frame/draft-vF.md`
- Campaign activity includes artifact received and first draft generated events.

## Known Controls

- Copilot Chat entry point: `https://m365.cloud.microsoft/chat`
- Expected controls include left-sidebar `Researcher`, `Long`, active-generation composer state, idle composer state, `Convert to Word`, Word `File`, Word `Create a Copy`, and Word `Download a copy`.

## Human Gate

- Stop at credentials, MFA, authenticator, permissions, or policy prompts.
- Stop before submitting the research prompt unless intake has been saved.
- Stop for Copilot follow-up questions only when `copilot_follow_up_policy` is `pause_for_operator`.
- Stop if the download/file destination is ambiguous.
- Stop if Copilot Researcher refuses, errors, or produces a visibly thin/broken research artifact. Do not stop for Word's right-side Copilot pane service error when the Word document and download controls still work.

## Promotion Notes

This recipe is `cursor_replay`: Cursor remains in the loop with browser-harness/live observation. Keep durable browser-harness quirks in `browser-use/notes.md` and long prompts in `prompts/`.
