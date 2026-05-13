# Video Production

Use this when a campaign post is video-shaped: raw talking-head footage, an authored product/feature video, a HyperFrames launch-style montage, generated motion assets, or any hybrid of those.

This file owns the **when, what, and how-to-compose** for video production inside AgentFrame Marketing. Vendored skill packs own their generic craft:

- HyperFrames teaches HTML video composition, preview, inspection, and rendering.
- video-use teaches transcript-driven raw-footage editing.
- Flow / Veo / Nano Banana are manual browser workflows for generated footage, transitions, and image/video assets until an API integration earns its place.

AgentFrame Marketing owns campaign context: arc, audience, design language, CTA, prior posts, state tracking, and publish reconciliation.

---

## Load Order

When video work starts, load:

1. This file.
2. The campaign `campaign.md` frontmatter and the current post row.
3. The campaign messaging architecture if it exists.
4. The design language artifacts if visual style is relevant: `design-language-v{N}.md`, `tokens.yaml`, and existing previews.
5. `library/deliverables/video-spec/template.md`.
6. The capability references only as needed:
   - `system/skills/hyperframes/SKILL.md`
   - `system/skills/hyperframes-cli/SKILL.md`
   - `system/skills/gsap/SKILL.md`
   - `system/skills/hyperframes/references/launch-video/README.md`
   - `system/skills/hyperframes/references/launch-video/HANDOFF.md`
   - `system/skills/hyperframes/references/launch-video/SCRIPT.md`
   - `system/skills/hyperframes/references/launch-video/STORYBOARD.md`
   - `C:\Cursor Projects\MarketingOS-video-references\video-use\SKILL.md` when raw footage needs transcript-driven editing.

Treat HyperFrames/GSAP skill folders as vendored snapshots: update intentionally, not ad hoc during campaign drafting.

---

## Capability Palette

Do not classify a video into one exclusive tool path. Pick and combine capabilities based on the material.

| Capability | Use when | Typical outputs |
|---|---|---|
| HyperFrames | The video needs authored motion graphics, product feature scenes, UI walkthroughs, launch-video pacing, caption/timeline control, or final MP4/WebM rendering. | `video/index.html`, `video/compositions/*.html`, `video/meta.json`, `video/renders/scenes/*/draft.mp4`, `video/renders/final/final.mp4` |
| video-use | The user drops raw talking-head footage, screen recordings, multiple takes, or any source where transcript-driven cutting matters. | `edit/takes_packed.md`, `edit/edl.json`, transcript cache, `edit/preview.mp4`, `edit/final.mp4` |
| Manual Flow / Veo / Nano Banana | The video needs generated transitions, intricate backgrounds, motion clips, image variants, or stylized footage and the operator is using Flow in the browser. | Files dropped into `video/assets/` or `edit/`, with prompt/provenance recorded in `video-spec-v{N}.md` or `HANDOFF.md` |

For a hybrid video, let these feed each other. Example: video-use edits talking-head footage into `edit/final.mp4`; HyperFrames uses that clip as an asset with product UI scenes and animated captions; Flow-generated clips become transitions or backgrounds in `video/assets/`.

---

## Recommended Project Shape

For authored or hybrid videos, use the HyperFrames launch-video structure inside the post folder:

```text
phase-4-production/posts/post-{n}/
  video-spec-v{N}.md
  video/
    README.md
    HANDOFF.md
    SCRIPT.md
    STORYBOARD.md
    meta.json
    index.html
    compositions/
    assets/
    transcript_*.json
    renders/
      scenes/                  # per-scene draft renders
        scene-1-{name}/        # mirrors compositions/scene-1-{name}.html
          draft.mp4            # optional per-scene draft render
      final/                   # assembled final cut
        draft.mp4              # optional pre-final assembly render
        final.mp4              # the one that ships
```

For raw-footage-heavy sessions, allow a sibling `edit/` folder using video-use conventions:

```text
phase-4-production/posts/post-{n}/
  video-spec-v{N}.md
  raw/                 # optional user-dropped source footage
  edit/
    project.md
    takes_packed.md
    edl.json
    transcripts/
    animations/
    preview.mp4
    final.mp4
```

Use both folders when useful. `edit/` is the raw-footage workbench; `video/` is the authored HyperFrames project and final assembly surface.
Within `video/`, keep `renders/` for rendered video outputs (`.mp4`/`.webm`) only; per-scene folders mirror `compositions/scene-N-{name}.html`.

---

## Process

1. **Name the video job.** Tie the video to the campaign arc: what it proves, what post it follows, what post it tees up, and what CTA it earns.
2. **Inventory material.** List raw footage, screenshots, product UI captures, existing design-language assets, Flow-generated assets, audio/voiceover needs, and any hard platform requirements.
3. **Choose capabilities from the palette.** State which tools are useful for this job and why. Do not force a single path.
4. **Create or update `video-spec-v{N}.md`.** Keep it as the planning/checklist artifact. It should describe the intended structure and asset provenance, not over-script the creative work.
5. **Build the working folders.** Use `video/`, `edit/`, or both.
6. **For raw footage:** load video-use, produce transcript/EDL artifacts, render a preview, and only show the user after self-eval passes or residual issues are named.
7. **For authored video:** load HyperFrames skills, use `SCRIPT.md` and `STORYBOARD.md` when the video needs narration or beat-by-beat direction, then build `index.html` and `compositions/`.
8. **For Flow assets:** the operator generates assets in Flow manually, drops files into the project, and the agent records the prompt, model/tool used, date, and path.
9. **Preview.** Use HyperFrames Studio for timeline authoring. Use the AgentFrame Marketing preview hub for campaign artifact browsing and rendered video review.
10. **Verify.** Run the relevant checks before lock:
    - HyperFrames: `npx hyperframes doctor`, `npx hyperframes lint`, `npx hyperframes validate`, `npx hyperframes inspect`, preview as needed, then draft/final render.
    - video-use: transcript cache present, EDL exists, preview render exists, self-eval notes recorded.
11. **Lock.** `video-spec-v{N}.md` can lock only when the user has reviewed the draft render or explicitly waived review, final render paths are recorded, and open production issues are either fixed or named.
12. **Publish reconciliation.** When the post ships, `copy-v{N}.md` remains the shipped state owner. Add final video files to `shipped_media[]`, update publish fields, mirror `campaign.md`, and append the campaign activity entry.

---

## Flow / Veo / Nano Banana Provenance

For Post 4 and near-term work, Flow is external. Do not call APIs from AgentFrame Marketing.

When the operator provides Flow outputs, record:

- Tool: Flow, Veo, Nano Banana, or combined.
- Prompt or short prompt summary.
- Date generated.
- Purpose in the cut: background, transition, b-roll, hero still, texture, etc.
- File path after the asset is dropped into the post folder.
- Any limitations: watermarks, crop constraints, text-in-image issues, style mismatch, or needs regeneration.

This shape is deliberately compatible with a future API helper. If API generation later earns its place, it can write the same provenance fields automatically.

---

## Lock Criteria

A video post is ready to lock when:

- `video-spec-v{N}.md` has `status: locked`.
- Final render path exists and is recorded.
- Draft render has been reviewed by the user or review is explicitly waived.
- HyperFrames/video-use verification appropriate to the project has run or any skipped check is named with reason.
- CTA and ending frame align with the post's role in the campaign arc.
- Any generated assets have provenance recorded.
- `copy-v{N}.md` and the video agree on CTA, visual promise, and shipped media.

---

## Non-Goals

- Do not casually patch vendored HyperFrames/video-use skills during campaign drafting; either use them as-is or run an intentional refresh/update pass.
- Do not create a custom AgentFrame Marketing video renderer.
- Do not require `SCRIPT.md` / `STORYBOARD.md` for tiny clips that do not need them.
- Do not require video-use for authored HyperFrames videos.
- Do not require HyperFrames for a simple raw-footage cut unless overlays, product scenes, or authored assembly make it useful.
- Do not hide video state in external folders only. Final state and shipped media still reconcile into AgentFrame Marketing files.
