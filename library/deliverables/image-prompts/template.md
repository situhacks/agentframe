# Template: Image Prompts

## Purpose

The post's generation prompts — versioned and locked like any deliverable, so the prompts that produced the shipped images live in the campaign record instead of dying in the transcript.

## Inputs

- **Design Language** locked — its treatment block is the shared base every prompt builds on
- **Slide Copy** locked when the images carry slide text (titles, body) inside them
- The campaign's generator preference from `project.md` `post_manifest`

## Artifact Shape

One file per post: `image-prompts-v{N}.md`.

- **Treatment block** at the top, stored ONCE — the design language's paste-once visual treatment, copied verbatim. Every assembled prompt = this block + the slide's delta.
- **Per-slide delta blocks** — only what changes per slide: subject, composition, text content and placement. No restating the treatment.
- **FINAL section** — the locked, fully assembled prompts (treatment + delta merged), copy-paste ready for any generator, accumulated slide by slide as the operator approves renders.
- Render-feedback annotations live OUTSIDE the prompt blocks (e.g. "field labels like HEADLINE: render literally — write text inline").

## Hard Constraints

- Prompts are fresh-context: positive present-tense description of the image wanted, nothing else. No accumulated bans from prior iterations, no provenance or series history inside a prompt block.
- Never one-shot a slide. Offer options varying ONE axis at a time (composition / object / text placement), render, narrow with the operator, then lock that slide into FINAL before moving on.
- A locked slide's prompt is carried verbatim when iterating neighbouring slides — shared elements (background field, treatment) must not drift between slides.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../process/deliverable-versioning.md), plus `image_method: <generator>` once chosen (paths in [`library/process/image-production.md`](../../process/image-production.md)).

## Lock Criteria

- Every slide that needs an image has its assembled prompt in FINAL, and the operator approved the render it produced.
- Lock-event mechanics per [`library/process/lock-event.md`](../../process/lock-event.md); the FINAL section lands in the post's `post-FINAL.md` in the lock turn.
