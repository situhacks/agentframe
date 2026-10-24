# Template: Body Copy

## Purpose

The platform caption that ships with the post—body text, CTA, alt text. It drafts LAST: for visual posts, after slide copy is ready; for text-only posts, straight from campaign architecture.

## Inputs

- **Slide Copy** ready when the post has a carousel (the body diverges from it, so it has to exist first)
- **Voice**: mandatory — load the voice system at `workspace/life/voice/README.md` before drafting
- **Campaign Architecture** ready (CTA matches this post's role in the arc)
- **Research Artifact** for any externally citable claim

## Before Writing

1. Classify the operation as first draft, content change, or readiness reconciliation; load the project tracker and [`deliverable-versioning.md`](../../../../process/deliverable-versioning.md).
2. For first draft, run `af draft <project> <post-row> --artifact body-copy`. For any change to existing prose, run `af version <project> <post-row> --artifact body-copy` first — copyedits included.
3. Read every input above. For Voice, follow the full [`voice/README.md`](../../../../../workspace/life/voice/README.md) route to its required register, corpus, and pairs; nearby shipped posts are not substitutes.
4. Before the first review surface, run the voice clean pass ([`voice/README.md`](../../../../../workspace/life/voice/README.md) step 3) and `af voice lint <head>`. Later agent rewrites get `anti-patterns.md` on the rewritten region only; operator hand-tuning gets no pass.

## Artifact Shape

- **Body** — flowing prose, no bullets, hook first.
- **Alt text** — why the image matters, not "image of…".
- Hook or body variants while the operator is choosing; remove the losers once picked.

## Hard Constraints

- The hook is declarative and specific; the first two lines earn the rest.
- The body is a different on-ramp to the idea — supporting material on its own arc, never a slide-by-slide retelling of the carousel.
- CTA matches the post's job (tease / prove / ask / callback).
- Claims citing external facts trace to the campaign's research artifact.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `body-copy-v{N}.md` in the post folder named by the project plan.

## Readiness Criteria

- Operator approves.
- The clean pass already ran on the first draft. Do not rerun it because the artifact is becoming ready.
- Ready-event mechanics per [`library/process/ready-event.md`](../../../../process/ready-event.md); the ready copy lands in the post's `post-FINAL.md` in the same turn. The publish record lives in `post-FINAL.md`, not here.
