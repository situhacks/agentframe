# Template: Carousel Spec

## Purpose

The per-post carousel production spec. It tells the human and the rendering agent what the carousel should say, what each slide should do, what image/visual direction is needed, and how that lines up with the LinkedIn post copy.

## Inputs

- **Messaging Architecture**: Locked (post role, arc, hook hypotheses, callbacks).
- **Design Language**: Locked (`design-language-v{N}.md` + `tokens.yaml`) when the carousel will be rendered. Do not restate them here.
- **Image Production Menu**: `library/process/image-production.md` when slide planning needs generated-image path selection.
- **Post Copy**: At least drafted when cover hook / CTA alignment matters.
- **Voice Rules**: Load `voice.md` when the carousel spec includes user-facing prose (Recommended LinkedIn Copy, post-copy opening hooks, Slide 1 title options, slide titles, captions, body copy, CTA slide text). For pure visual structure, tokens, HTML, and layout notes, voice rules do not apply.

## Output Shape

**Artifact location**:
```text
phase-4-production/posts/post-{n}/
  carousel-spec-v{N}.md
  visuals/
    carousel-slide-{i}.html
    alt-text.md
```
*Note: The `.html` files and `alt-text.md` are generated artifacts or write-once companion files and are not versioned via the vF/vN convention.*

**Sections per carousel post**:
- **Current preview** — the current preview/render path, if one exists. No run history.
- **Recommended LinkedIn copy** — optional temporary section while the operator is reviewing copy and carousel together.
- **Alternate post-copy opening hooks** — optional, clearly labelled as LinkedIn opening hooks, not Slide 1 titles.
- **Slide 1 title options** — optional, clearly labelled as carousel cover titles, not LinkedIn hooks.
- **Carousel content** — slide-by-slide copy and visual direction. Each slide gets:
  - **Copy** — header, subheader/body, CTA or land line where applicable.
  - **Visual** — layout/image direction specific to this slide.
  - **Slide job** — one line explaining what the slide contributes to the post's arc.
- **Hard constraints** — only current production constraints the operator or renderer must obey.
- **Open before lock** — only unresolved decisions blocking lock.

## Hard Constraints

- The spec is an operator-readable production artifact, not an audit trail. Keep it short, current, and useful. Avoid historical context setting unless it changes what the human should approve or what the renderer should build.
- Do not restate the campaign design language, token definitions, prior visual directions, or resolved rationale. Link to the canonical file when needed.
- Keep the artifact lean: write the carousel and its needed caption, not the upstream campaign direction, goals, audience, or strategy.
- Post-copy opening hooks, Slide 1 title options, slide copy, visual direction, hard constraints, and open lock questions are clearly separated.
- Cover title and slide jobs align with the post hook/thesis.
- CTA placement in the carousel does not fight the LinkedIn CTA.
- **Coherence cross-check.** When both post copy and the carousel are drafted, verify: post-copy hook phrase matches the cover slide; post-copy body does not repeat what the slides already say; CTA in copy and CTA placement in the carousel match. If any of these misalign, surface which artifact is the easier fix and patch the smaller one.
- Slide text stays light. If a slide needs more than two short body sentences, move nuance to the LinkedIn copy.
- Visual direction is actionable enough for rendering: layout, imagery/icon needs, motif usage, and any slide-specific deviation from the design language.
- Before inventing a new rendered layout, identify any locked campaign visual components or motifs that should be reused; deviate only when the slide job earns it.
- Image needs can be named here, but generated-image prompts, variants, and selected outputs belong in `image-prompt-v{N}.md` / `images/` (recorded using `library/process/image-production.md`).
- Every slide role from the per-post plan has a corresponding rendered HTML when the carousel is rendered.
- Alt text is substantive when final visuals exist.

## Draft Frontmatter Convention

Schema follows [`library/process/campaign-frontmatter.md`](../../process/campaign-frontmatter.md).

```yaml
---
status: <drafting | locked | deferred>
last_updated: <ISO-8601 timestamp>
current_version: <integer; incremented on every Write-tool replacement>
version_history:
  - {v: <int>, date: <YYYY-MM-DD>, note: "<one-line reason for this version bump>"}
---
```

*Note: `carousel-spec` does not have its own `shipped` state. The post is the shipping unit, and the shipped record lives in `copy-v{N}.md`.*

## Lock Criteria

- Carousel spec is operator-readable and current-state only.
- All slides have copy, visual direction, and slide job.
- Cover hook/title aligns with Post Copy.
- CTA alignment checked against Post Copy.
- Visual reuse checked against locked campaign components/motifs before net-new rendering.
- Rendered slides exist when this is a rendered carousel.
- Alt text complete when final visuals exist.
- Post unblocks publishing only when Carousel Spec + Post Copy + any needed Image Prompt are locked and cross-checked.

## Review Path

- **Reviewer**: User reviews in browser via localhost preview.
- **Export format**: Visuals exported as PNG (user takes browser screenshots for HTML slides as final delivery).
- **Required before downstream**: Per-post production must lock before publishing.

## Humanizer Pass

**Partial — prose regions only.** See [`library/process/humanizer-integration.md`](../../process/humanizer-integration.md). Scope to temporary LinkedIn copy, post-copy hooks, cover title options, slide titles, captions, body copy, and CTA text. Skip structural blocks, visual notes, HTML, tokens, layout specs, and file paths.

## Exceptions / Branches

- **`copy-v{N}.md` already exists**: Link to it. Do not duplicate the full LinkedIn caption unless the operator is explicitly reviewing copy and carousel together.
- **Single-image post**: Skip carousel spec; image generation flows through `library/process/image-production.md` and records to `image-prompt-v{N}.md`.
