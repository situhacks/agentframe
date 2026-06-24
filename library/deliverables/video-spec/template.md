# Template: Video Spec

## Purpose

Planning and production tracker for a video deliverable. It gives the agent enough structure to use HyperFrames, video-use, manual Flow assets, or a hybrid while keeping the video grounded in the project's objective. It is not a script template and not a creative straitjacket; the actual video project owns its script, storyboard, edit decisions, compositions, and renders.

## Depends On

- `project.md` frontmatter and the current deliverable row.
- The project's plan/architecture if one exists.
- Design Language if the video needs project-specific visual continuity.
- `library/process/video-production.md`.
- Operator Voice (load the voice system (`library/context/operator/voice/`) if the video includes user-voiced narration, captions meant to sound like the operator, or external-facing script copy; do not load for purely technical render/planning work).
- Video capability references as needed:
  - `system/skills/hyperframes/SKILL.md`, `system/skills/hyperframes-cli/SKILL.md`, and `system/skills/gsap/SKILL.md` for authored composition and rendering.
  - `system/skills/hyperframes/references/launch-video/` for optional launch-video reference artifacts (load only when needed).
  - video-use for raw-footage transcript/edit work.
  - Manual Flow/Veo/Nano Banana browser workflow for generated assets.

## Sections

- **Video job** — what this video proves toward the project objective, what it follows, what it tees up, and the intended CTA.
- **Format** — platform, aspect ratio, resolution, fps, target runtime, caption/subtitle expectation, voiceover/talking-head expectation.
- **Project structure** — the working folders/files for this video project.
- **Asset inventory and provenance** — raw footage, screen recordings, screenshots, generated clips, generated images, audio, transcripts, and prompts/provenance for manual Flow assets.
- **Production notes** — current plan, open decisions, known risks, and links to `SCRIPT.md`, `STORYBOARD.md`, `HANDOFF.md`, `edit/`, or `video/` files.
- **Preview and render outputs** — preview URLs, draft renders, final renders, and any known issues.

## Hard Constraints

- **Manual Flow assets**: Record prompt/provenance/path. Do not call APIs from the harness until that integration is explicitly built.
- **Coherence cross-check.** When a companion text artifact and the video are both drafted, verify: the companion's hook matches the opening frame; it does not repeat what the video beats already say; the CTA in copy matches the video's ending CTA. If any of these misalign, surface which artifact is the easier fix and patch the smaller one.

## Draft Frontmatter Convention

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
video_method: <hyperframes | video_use | manual_flow_veo_nano | hybrid>
---
```

Follow standard deliverable versioning (reference `library/process/project-frontmatter.md` for schema).
Generated media files, `video/renders/scenes/`, `video/renders/final/`, `edit/preview.mp4`, and `edit/final.mp4` are not versioned via v{N}. They are media outputs with their own paths and provenance.

## Lock Criteria

- Video job and CTA align with the project objective.
- Intended project structure exists or any missing part is explicitly waived as unnecessary for this video.
- Asset inventory records all source footage, generated assets, and manual Flow prompts/provenance where applicable.
- Draft or final render path is recorded.
- Appropriate verification ran:
  - HyperFrames: `doctor`, `lint`, `validate`, `inspect`, render.
  - video-use: transcript/EDL artifacts present, preview/final render present, self-eval notes recorded.
- Known issues are fixed or explicitly accepted by the operator.
- The active pack's delivery flow knows which video file will ship, when the deliverable feeds one.

## Review Path

- **Reviewer**: operator.
- **Export format**: final video file (`.mp4` by default; `.webm` if transparency or platform needs it).
- **Required before downstream**: a video-using deliverable cannot lock until the final or publish-candidate render is reviewed or review is explicitly waived.

## Humanizer Pass

Partial. Run only when `video/SCRIPT.md` is authored or substantively revised. Scope to public-facing script prose: narration, captions, and on-screen editorial text.

Do not run this gate at `video-spec-v{N}.md` lock time. Skip `STORYBOARD.md`, structured production fields, commands, frontmatter, code, EDL JSON, and model prompt text.

When needed, follow `library/process/humanizer-integration.md`.

## Exceptions / Branches

- **Final polish outside the harness**: allowed. Record the actually delivered file in the active pack's delivery record (e.g. marketing's `post-FINAL.md` `shipped_media[]`) during reconciliation.
