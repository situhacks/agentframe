# Template: Slide Copy

## Purpose

The carousel's slide-by-slide text—the copy, a few notes, nothing else. This is what the operator reads top-to-bottom and approves. It becomes ready before body copy drafts and before imagery work consumes its word counts.

## Inputs

- **Campaign Architecture** ready (this post's job in the arc, hook angle, callbacks)
- **Voice**: mandatory — load the voice system at `workspace/life/voice/README.md` before drafting
- **Design Language** only when slide text interacts with the visual treatment (text-over-image contrast, title placement)

## Before Writing

1. Classify the operation as first draft, content change, or readiness reconciliation; load the project tracker and [`deliverable-versioning.md`](../../../../process/deliverable-versioning.md).
2. For first draft, run `af draft <project> <post-row> --artifact slide-copy`. For any change to existing prose, run `af version <project> <post-row> --artifact slide-copy` first — copyedits included.
3. Read every input above. For Voice, follow the full [`voice/README.md`](../../../../../workspace/life/voice/README.md) route to its required register, corpus, and pairs; nearby shipped posts are not substitutes.
4. Before the first review surface, run the voice clean pass ([`voice/README.md`](../../../../../workspace/life/voice/README.md) step 3) and `af voice lint <head>` on slide prose. Later agent rewrites get `anti-patterns.md` on the rewritten region only; operator hand-tuning gets no pass.

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

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `slide-copy-v{N}.md` in the post folder named by the project plan.

## Readiness Criteria

- Operator approves the full deck read top-to-bottom.
- The clean pass already ran on slide prose in the first draft. Do not rerun it because the artifact is becoming ready.
- Ready-event mechanics per [`library/process/ready-event.md`](../../../../process/ready-event.md); the ready copy lands in the post's `post-FINAL.md` in the same turn.
