---
name: open-design
description: "Bundled Open Design runtime. Use after selecting the Open Design path from the image or deck production menu."
---

# Open Design (Bundled)

Vendored source is in [`source/`](source/); vendoring provenance and cuts are tracked in [`VENDOR.md`](VENDOR.md). This file owns everything AgentFrame-side: runtime setup, project staging, and lock-time import. Load it when CMO has picked the Open Design path for a deliverable.

OD's `source/` tree carries its own `AGENTS.md` and `CLAUDE.md` hierarchy. Keep campaign/system agent work at the AgentFrame repo root and only enter `system/skills/open-design/source` for runtime control commands (`tools-dev start/status/stop`) to avoid loading OD-local agent instructions into broader AgentFrame tasks.

---

## Setup

First-time install (Node 24 required per upstream engines):

```powershell
cd system/skills/open-design/source
corepack enable
corepack pnpm install
```

Launch:

```powershell
corepack pnpm exec tools-dev start
```

Setup paths:

- **Local CLI (default).** OD scans `PATH` for `claude`, `codex`, `gemini`, `cursor-agent`, `qwen`, `qoder`, and `copilot`. If any is installed and authenticated against an existing subscription, OD picks one automatically. Verify in Settings → Execution & model.
- **BYOK fallback.** If no CLI is on `PATH` or detection fails, drop a provider key into Settings → Execution & model. The `GEMINI_API_KEY` already in `.env` is the easiest reuse.

Sanity check on first launch:

```powershell
corepack pnpm exec tools-dev status
```

Output should show the daemon URL and web URL. Open the web URL once to confirm the daemon is reachable before staging a campaign project. Reuse a running instance instead of starting another.

---

## Inputs

Before staging, load:

- Locked campaign `design-language-v{N}.md` + `tokens.yaml` / `tokens.css` when present.
- [`library/deliverables/design-language/transfer-to-open-design.md`](../../../library/deliverables/design-language/transfer-to-open-design.md) for the DL → OD `DESIGN.md` mapping table.
- Calling deliverable body (e.g., `image-prompt-v{N}.md` for a single image, `carousel-spec-v{N}.md` for a carousel).
- Operator constraints stated in the turn (platform, dimensions, export format, deadline).

Do not load full campaign history unless the deliverable template requires it.

---

## Stage

Goal: a non-blank Open Design session with campaign design language staged, the right mode/skill selected, the first message prepared in the input box, and a project URL ready for the operator to open.

1. **Pre-flight.** Run `corepack pnpm exec tools-dev status` from `system/skills/open-design/source`. If OD is not running, `tools-dev start`. Capture the daemon URL and web URL.

2. **Translate DL → OD `DESIGN.md`.** Walk the mapping table in [`transfer-to-open-design.md`](../../../library/deliverables/design-language/transfer-to-open-design.md). Write the result to:

   ```text
   system/skills/open-design/source/design-systems/agentframe-{campaign-slug}/DESIGN.md
   ```

   Do not invent missing Components or Motion content; use the skip-with-reason defaults from the transfer table.

3. **Pick mode + skill** from the defaulting table in [`library/process/image-production.md`](../../../library/process/image-production.md) § "Open Design mode + skill defaults". Surface the default with a one-line rationale if the operator hasn't already picked. Accept operator overrides.

4. **Compose `pendingPrompt`.** Decisive, production-ready first message. Recommended structure:

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

5. **Create the project.** POST to `<daemon-url>/api/projects` with: project id `agentframe-{campaign-slug}-{deliverable-id}`, the confirmed `skillId`, `designSystemId: agentframe-{campaign-slug}`, the `pendingPrompt`, and metadata (`source: agentframe`, `campaignSlug`, `deliverablePath`, `mode`). Daemon docs in `source/docs/` own the request shape — read those if the API changes.

6. **Reply to the operator.** Short handoff:
   - Open Design URL: `<web-url>/projects/<project-id>`
   - Runtime mode: local CLI detected, or BYOK fallback needed in Settings → Execution & model.
   - Action: "The prompt is already in the input box; click Send."
   - Export target: where locked assets should return in the calling deliverable.

   Do not ask the operator to copy a prompt from a file.

---

## Lock-Time Import

When the operator locks the OD output:

1. Locate exported files in the OD project directory or read them through the daemon files API.
2. Copy final assets into the calling deliverable's `visuals/imports/`.
3. Update the calling deliverable's frontmatter using that deliverable's own schema, for example:
   - `image_method: open_design`
   - `shipped_media: visuals/imports/<file>`
   - deck/PPT export fields where applicable
4. The calling deliverable owns lock criteria and campaign tracker updates.

---

## Cleanup

At campaign close, optionally remove `system/skills/open-design/source/design-systems/agentframe-{campaign-slug}/`. Until OD supports per-project `DESIGN.md` loading, the global design-system folder is the supported staging location.

---

## Lifecycle Reference

- **Status:** `corepack pnpm exec tools-dev status`
- **Stop:** `corepack pnpm exec tools-dev stop`
- **Restart:** stop, then start. Reuse running instances when possible.
- Exports return to the calling deliverable's `visuals/imports/`; the calling deliverable owns lock criteria.
