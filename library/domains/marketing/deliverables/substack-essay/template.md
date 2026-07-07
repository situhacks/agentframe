# Template: Substack Essay

## Purpose

A full-length essay for the operator's Substack publication — the long-form home of an idea, event, or proof point. It may pair with a LinkedIn post (the post teases; the essay carries the detail) or stand alone.

## Inputs

- **Voice**: mandatory — run the voice system sequence at `library/context/operator/voice/README.md` with register `substack-informal` (loads the overlay + the full substack corpus; the essay is drafted corpus-anchored, never from rules alone).
- **Channel conventions**: `library/context/channels/substack/profile.md` (series/subtitle format, footer, subscribe CTA, image handling).
- **Source material** when the essay reports a real event: transcript, notes, photos. What only the operator witnessed stays an open slot (see Hard Constraints).
- **Research Artifact** for any externally citable claim.

## Output Shape

- **Title options** (2–3, operator picks) + **subtitle** (descriptive, per channel format).
- **Sectioned body** — headers are flat claims in the operator's words, each section teaching or narrating one thing. A reveal/twist beat near the end when the material has one.
- **Open slots**, explicitly marked, for operator-only material: `[FILL]` (facts only the operator has), `[POV]` (his live phrasing on a take), `[NERD-NOD]` (cultural reference — agent never invents one).
- **Close**: takeaway as a directive, then assets/links block, then the native subscribe CTA. No sentiment-stamp, no engagement question.
- **Paired LinkedIn body**, when the essay ships with a post: drafted as its own `body-copy` deliverable, teasing not retelling.

## Hard Constraints

- **Never fabricate event facts.** Attendance counts, durations, quotes, student/participant outcomes — if the source material doesn't state it, leave a `[FILL]` slot. (Earned: the first essay draft invented "thirty-odd students, three hours"; reality was twelve and two.)
- **Open concrete** — a scene, the literal artifact, or a flat true statement. Never an abstraction, never "imagine".
- **Metaphors are functional, not literary** — a metaphor earns its place only by making a hard idea easier.
- **The register test at every section**: would this paragraph sit inside the pieces in `voice/corpus/substack/`? Editorial-polished prose that would sit in an admired publication but not in the operator's own published essay is off-register.
- Externally citable claims trace to the research artifact or a named source.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `substack-essay-v{N}.md` in the folder named by the active flow. After publishing, record `published_url` in the locked file's frontmatter.

## Humanizer Pass

Required before lock — this prose ships to the public feed. Per [`library/process/humanizer-integration.md`](../../../../process/humanizer-integration.md), run with a Brandon sample, as the separate clean pass of the voice sequence (never in the same context as the corpus exemplars).

## Lock Criteria

- All `[FILL]` / `[POV]` / `[NERD-NOD]` slots resolved or deliberately removed by the operator — no placeholder ships.
- Operator approves after his voice pass.
- Humanizer pass complete.
- Lock-event mechanics per [`library/process/lock-event.md`](../../../../process/lock-event.md) — the voice mini-retro at lock harvests this essay (pairs + corpus promotion once published).
