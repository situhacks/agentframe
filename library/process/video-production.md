# Video Production

Load this when a project needs a video deliverable. AgentFrame owns the project context, deliverable state, final export, and publish reconciliation. HyperFrames owns video workflow selection, project files, Studio, and rendering.

## Load

1. Project `project.md`, current post row, and project architecture when present.
2. Ready design-language artifacts when visual identity matters.
3. The calling `video-spec` deliverable.
4. [`system/skills/hyperframes/SKILL.md`](../../system/skills/hyperframes/SKILL.md), then the upstream route it selects.

For raw footage, generated assets, or audio, let the upstream HyperFrames route select the relevant capability. Do not preselect a workflow from an AgentFrame list.

## Project context

Give HyperFrames only the context that changes the video: audience, claim, CTA, platform, ready visual identity, available assets, and delivery constraints. Keep the source-of-truth brief in `video-spec-v{N}.md`.

Use the upstream project's own file formats. `SCRIPT.md` and `STORYBOARD.md` exist only when the selected HyperFrames workflow calls for them; their shape is owned by `hyperframes-core`, not AgentFrame.

## Delivery

1. Build the selected HyperFrames project under the calling post's `video/` folder.
2. Use Studio when interactive visual review or timeline feedback is useful.
3. Run `npx hyperframes check` before review, then preview and render through the selected upstream workflow.
4. Record final render paths in the ready `video-spec-v{N}.md` and the post's `shipped_media[]`.
5. Reconcile the published post and project state through the normal ready and publish paths.

## Boundaries

- AgentFrame does not maintain a parallel video craft guide, demo project, script format, storyboard format, or GSAP reference.
- Use another production tool only when the selected HyperFrames route does not fit the material.
- Do not modify vendored HyperFrames files during a project; refresh the pinned source intentionally.
