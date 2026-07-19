# Template: Slide Copy

## Purpose

The carousel's slide-by-slide text—the copy, a few notes, nothing else. This is what the operator reads top-to-bottom and approves. It becomes ready before body copy drafts and before imagery work consumes its word counts.

## Inputs

- **Campaign Architecture** ready (this post's job in the arc, hook angle, callbacks)
- **Voice**: mandatory — load the voice system at `library/context/operator/voice/README.md` before drafting
- **Design Language** only when slide text interacts with the visual treatment (text-over-image contrast, title placement)

## Before Writing

1. Classify the operation as first draft, surgical edit, replacement, or readiness reconciliation; load the project tracker and [`deliverable-versioning.md`](../../../../process/deliverable-versioning.md).
2. For first draft, run `python system/af.py draft <project> <post-row> --artifact slide-copy`. For replacement, run `python system/af.py version <project> <post-row> --artifact slide-copy` before changing prose. A surgical edit stays in the current drafting head.
3. Read every input above. For Voice, follow the full [`voice/README.md`](../../../../context/operator/voice/README.md) route to its required register, corpus, and pairs; nearby shipped posts are not substitutes.
4. Run the early agent-authored pass in [`humanizer-integration.md`](../../../../process/humanizer-integration.md) on slide prose before the first review surface. Later agent rewrites get a delta pass; operator hand-tuning does not.

## Artifact Shape

One block per slide:

- **Title** (when the slide has one) and **body text**, written exactly as they will appear on the slide.
- One optional note line per slide when a reviewer needs it (weight target, what this slide must not touch).

Options the operator is choosing between stay in the file until narrowed; remove the losers once picked.

## Hard Constraints

- Slide jobs are agreed before full prose is written. No two slides own the same job; each slide's content stays inside its job.
- The cover carries the complete thesis as flowing prose — not a headline plus mystery.
- Weight curve is managed: density alternates across the deck, the heaviest slide is the load-bearing argument beat, and no two adjacent slides sit at similar density.
- One recurring load-bearing noun threads the deck at structural anchors; the closer mirrors the cover (bookends).
- Slide text stays light. Nuance that doesn't fit moves to body copy—which drafts after this is ready.
- This file carries copy only. Prompts and imagery live in the post's image-prompts file; the platform caption lives in body copy.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `slide-copy-v{N}.md` in the post folder named by the active flow.

## Readiness Criteria

- Operator approves the full deck read top-to-bottom.
- Required early/delta humanizer work already ran on slide prose per [`humanizer-integration.md`](../../../../process/humanizer-integration.md). Do not rerun a full pass solely because the artifact is becoming ready.
- Ready-event mechanics per [`library/process/ready-event.md`](../../../../process/ready-event.md); the ready copy lands in the post's `post-FINAL.md` in the same turn.
