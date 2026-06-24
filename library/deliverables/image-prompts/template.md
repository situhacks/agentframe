# Template: Image Prompts

## Purpose

The generation prompts for a deliverable's images — versioned and locked like any deliverable, so the prompts that produced the delivered images live in the project record instead of dying in the transcript.

## Inputs

- **Design Language** locked — its treatment block is the shared base every prompt builds on
- Any text the images must render verbatim (titles, labels, body), locked, when the images carry it inside them
- The generator preference for this project (the active pack's generation settings, if it declares any)

## Artifact Shape

One file per image set: `image-prompts-v{N}.md`.

- **Treatment block** at the top, stored ONCE — the design language's paste-once visual treatment, copied verbatim. Every assembled prompt = this block + the image's delta.
- **Per-image delta blocks** — only what changes per image: subject, composition, text content and placement. No restating the treatment.
- **FINAL section** — the locked, fully assembled prompts (treatment + delta merged), copy-paste ready for any generator, accumulated image by image as the operator approves renders.
- Render-feedback annotations live OUTSIDE the prompt blocks (e.g. "field labels like HEADLINE: render literally — write text inline").

## Hard Constraints

- Prompts are fresh-context: positive present-tense description of the image wanted, nothing else. No accumulated bans from prior iterations, no provenance or series history inside a prompt block.
- Never one-shot an image. Offer options varying ONE axis at a time (composition / object / text placement), render, narrow with the operator, then lock that image into FINAL before moving on.
- A locked image's prompt is carried verbatim when iterating neighbouring images — shared elements (background field, treatment) must not drift between images.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../process/deliverable-versioning.md), plus `image_method: <generator>` once chosen (paths in [`library/process/image-production.md`](../../process/image-production.md)).

## Lock Criteria

- Every image needed has its assembled prompt in FINAL, and the operator approved the render it produced.
- Lock-event mechanics per [`library/process/lock-event.md`](../../process/lock-event.md); the FINAL section lands in the active pack's assembly-record deliverable in the lock turn, when the pack declares one.
