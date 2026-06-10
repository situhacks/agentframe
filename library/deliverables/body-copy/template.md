# Template: Body Copy

## Purpose

The platform caption that ships with the post — body text, CTA, alt text. It drafts LAST: for visual posts, after slide copy locks; for text-only posts, straight from campaign architecture.

## Inputs

- **Slide Copy** locked when the post has a carousel (the body diverges from it, so it has to exist first)
- **Voice**: mandatory — load the voice system at `library/context/operator/voice/README.md` before drafting
- **Campaign Architecture** locked (CTA matches this post's role in the arc)
- **Research Artifact** for any externally citable claim

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

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../process/deliverable-versioning.md). File: `body-copy-v{N}.md` in the post folder named by the active flow.

## Lock Criteria

- Operator approves.
- Humanizer pass per [`library/process/humanizer-integration.md`](../../process/humanizer-integration.md) — this prose goes to the public feed.
- Lock-event mechanics per [`library/process/lock-event.md`](../../process/lock-event.md); the locked copy lands in the post's `post-FINAL.md` in the lock turn. The publish record lives in `post-FINAL.md`, not here.
