# Open Design AgentFrame Handoff

Trigger: agent is staging an Open Design project for an AgentFrame deliverable (image / carousel / deck / template / future).

This file owns the handoff between AgentFrame Marketing and the bundled Open Design runtime. Use it after `SKILL.md` has confirmed Open Design is the selected production path. Do not use it for setup-only questions.

The goal is a non-blank Open Design session: campaign design language staged, correct OD mode/skill selected, first message prepared in the input box, and project URL ready for the operator to open and click Send.

## Inputs

Load the smallest set that can stage the project:

- Locked campaign `design-language-v{N}.md`, plus `tokens.yaml` and `tokens.css` when present.
- [`library/deliverables/design-language/transfer-to-open-design.md`](../../../library/deliverables/design-language/transfer-to-open-design.md) for the DL -> OD `DESIGN.md` mapping.
- Calling deliverable:
  - `image-prompt-v{N}.md` body for a single image.
  - `carousel-spec` slide list for carousel work.
  - Deck/template spec for deck or template work.
- Relevant post copy or messaging architecture excerpt when it helps the visual brief.
- Operator constraints already stated in the turn (platform, dimensions, export format, deadline).

Do not load full campaign history unless the deliverable template requires it.

## Pre-flight

From `system/skills/open-design/source`, check runtime state:

```powershell
corepack pnpm exec tools-dev status
```

If Open Design is not running, start it:

```powershell
corepack pnpm exec tools-dev start
```

Capture the daemon URL and web URL from `tools-dev status` or startup output. Reuse a running instance instead of starting a duplicate.

If setup is missing, return to [`SKILL.md`](SKILL.md) setup paths before staging. Do not patch OD source or write `app.sqlite` directly.

## Stage Design System

Open Design loads design systems from:

```text
system/skills/open-design/source/design-systems/<design-system-id>/DESIGN.md
```

Build a campaign-scoped id:

```text
agentframe-<campaign-slug>
```

Translate the locked campaign design language into OD's 9-section `DESIGN.md` schema using [`transfer-to-open-design.md`](../../../library/deliverables/design-language/transfer-to-open-design.md), then write:

```text
system/skills/open-design/source/design-systems/agentframe-<campaign-slug>/DESIGN.md
```

Do not invent missing Components or Motion detail. Use the skip-with-reason defaults from the transfer resource unless the campaign DL explicitly includes those sections.

## Pick Mode And Skill

Open Design is mode-first, then skill-within-mode. Default mapping:

| AgentFrame deliverable | Default OD mode | Default OD skillId | Why |
|---|---|---|---|
| `image-prompt` single image, e.g. 1080x1080 LinkedIn hero | `image` | `canvas-design` | Anthropic image-mode skill for posters, illustrations, and static pieces. Custom canvas sizes are first-class. Used in the example POV run. |
| `carousel-spec` square social carousel | `image` | `canvas-design` per slide | Best canvas fidelity for LinkedIn/IG square slides. Stage one project per slide, or one project iterated slide-by-slide, so every slide honors the campaign DL. |
| `carousel-spec` content-heavy and 16:9 acceptable | `deck` | `simple-deck` | Bulk slide generation in one project. Use only when the operator confirms 16:9 is acceptable. |
| Deck / PPT long-form presentation | `deck` | `magazine-web-ppt` | OD's bundled deck default. PPTX/PDF export is first-class; swap to `pptx` when PPTX editability is more important than visual style. |

Surface the default with one-line rationale before staging when the operator has not already chosen the mode/skill. Accept an operator override and pass the confirmed `skillId` to project creation.

## Compose `pendingPrompt`

`pendingPrompt` is the prepared first message that appears in OD's input box but is not sent. Keep it decisive and production-ready.

Recommended structure:

```markdown
Create <deliverable type> for AgentFrame campaign "<campaign name>".

Output goal:
- <platform / format / dimensions / export target>

Use the active design system:
- `agentframe-<campaign-slug>`

Visual brief:
<calling deliverable body or slide-by-slide brief>

Copy/context to preserve:
<short excerpt from post copy, messaging architecture, or deck outline>

Constraints:
- Honor the campaign design system.
- Produce exportable HTML/PNG/PDF/PPTX appropriate to the selected OD mode.
- Keep text legible at the target canvas size.
- Do not introduce new strategic claims not present in the campaign material.
```

Avoid duplicating the full design-language body in the prompt. OD injects the active design system separately through `designSystemId`.

## Create Project

Create a stable project id:

```text
agentframe-<campaign-slug>-<deliverable-id>
```

Call the daemon:

```http
POST <daemon-url>/api/projects
Content-Type: application/json

{
  "id": "agentframe-<campaign-slug>-<deliverable-id>",
  "name": "<Campaign Name> - <Deliverable Name>",
  "skillId": "<confirmed-skill-id>",
  "designSystemId": "agentframe-<campaign-slug>",
  "pendingPrompt": "<prepared first message>",
  "metadata": {
    "source": "agentframe",
    "campaignSlug": "<campaign-slug>",
    "deliverablePath": "<workspace/campaigns/... path>",
    "mode": "<confirmed-od-mode>"
  }
}
```

The response returns `conversationId`; the endpoint also seeds the default conversation. Do not create a separate conversation unless the API shape changes.

## Drive Tab

Open Design's tab API is file-tab state, not mode navigation. Before generation, project creation with `skillId`, `designSystemId`, and `pendingPrompt` is the important setup.

After a generated file exists, optionally set the active file tab:

```http
PUT <daemon-url>/api/projects/<project-id>/tabs
Content-Type: application/json

{
  "tabs": ["index.html"],
  "active": "index.html"
}
```

If no file exists yet, skip this step. Do not fabricate tabs.

## Operator Handoff

Reply with one short handoff:

- Open Design URL: `<web-url>/projects/<project-id>`
- Runtime mode: local CLI detected, or BYOK fallback needed in Settings -> Execution & model.
- Action: "The prompt is already in the input box; click Send."
- Export target: where locked assets should return in the calling deliverable.

Do not ask the operator to copy a prompt from a file.

## Lock-Time Import

When the operator locks the OD output:

1. Locate exported files in the OD project directory or read them through the daemon files API.
2. Copy the final assets into the calling deliverable's `visuals/imports/`.
3. Update the calling deliverable frontmatter using that deliverable's own schema, for example:
   - `image_method: open_design`
   - `shipped_media: visuals/imports/<file>`
   - deck/PPT export fields where applicable
4. The calling deliverable owns lock criteria and campaign tracker updates.

## Optional Cleanup

At campaign close, offer to remove:

```text
system/skills/open-design/source/design-systems/agentframe-<campaign-slug>/
```

Only remove it with operator confirmation. Until Open Design supports per-project `DESIGN.md` loading, the global design-system folder is the supported staging location.
