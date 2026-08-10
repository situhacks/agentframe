# Template: Substack Essay

## Purpose

A full-length essay for the operator's Substack publication — the long-form home of an idea, event, or proof point. It may pair with a LinkedIn post (the post teases; the essay carries the detail) or stand alone.

## Inputs

- **Voice**: mandatory — resolve a formal, informal, or base-plus-borrow recipe through `library/context/operator/voice/README.md`. Select it from the essay's audience, purpose, and operator direction, never from the fact that it publishes on Substack. Draft against the selected base corpus, not rules alone.
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
- **The register test at every section**: would this paragraph sit inside the selected base register's corpus while satisfying any explicit borrow direction? Editorial-polished prose that would sit in an admired publication but not in the operator's own work is off-register.
- Externally citable claims trace to the research artifact or a named source.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `substack-essay-v{N}.md` in the folder named by the active flow. Run `af publish ... --url <url>` after it goes live; the generic transition records `published_url` and makes the head immutable.

## Humanizer Pass

**Run it on the first complete draft, before the operator ever sees it — not at readiness.** A draft surfaced with the pass still owed puts the operator's line edits onto un-swept prose, and from that point the pass can no longer run without washing his wording back to generic. Per [`library/process/humanizer-integration.md`](../../../../process/humanizer-integration.md), run with a Brandon sample, as the separate clean pass of the voice sequence (never in the same context as the corpus exemplars). Readiness only verifies that it happened.

## Readiness Criteria

- All `[FILL]` / `[POV]` / `[NERD-NOD]` slots resolved or deliberately removed by the operator — no placeholder ships.
- Operator approves after his voice pass.
- Humanizer pass complete.
- Ready-event mechanics per [`library/process/ready-event.md`](../../../../process/ready-event.md)—the voice mini-retro at readiness harvests this essay (pairs + corpus promotion once published).
