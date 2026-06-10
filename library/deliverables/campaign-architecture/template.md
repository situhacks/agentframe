# Template: Campaign Architecture

## Purpose

The full execution plan, post by post. Lives where the rubber meets the road: every post mapped to a hook, story, payoff, and CTA. Anchored in `positioning.md`, with audience extracted inline below. This is the copywriter's primary brief.

## Depends On

- **Standard Flow:** Campaign Brief locked + Research Artifact locked
- **Solo Flow:** Concurrent Research Artifact (`phase-1-research/research-artifact-v{N}.md`)
- `positioning.md` loaded (POV anchoring)
- the `voice/pairs/` examples NOT loaded yet (this is structure, not output copy)

## Sections

**Author/Reader Stance:** Internal execution doc. Author is a marketer drafting this for the team's own use. Reader is future-you or whoever's drafting the post ingredients (slide copy, body copy, prompts) next. Treat the copywriter as smart — give them the steer, not the script.

- **Story arc** — the spine across the campaign. 3-5 sentences max. Beginning/middle/end.
- **Audience extraction** (required mini-section, grounded in the research artifact):
  - **Persona label** — one line: role + situation that frames why they care.
  - **Language tells** — 3-5 phrases they actually use, quoted/paraphrased from `research-artifact-v{N}.md` sources.
  - **Top 3 objections** — short, specific, grounded.
  - **Disconfirmation** — one line: what would prove this persona does not actually exist or does not care, with the answer.
  - **Multi-persona** — repeat this block only when the campaign genuinely targets more than one audience.
- **Per-post breakdown**, in order:
  - **Post N: [working title]**
  - **Job in the arc** — one line: hook / build / payoff / CTA / callback.
  - **Hook angle** — locked direction in 1-2 lines, plus a one-line rationale (why this angle, not the alternate from Campaign Brief).
  - **Core message** — one sentence
  - **CTA** — one line; matches the post's role.
  - **Callbacks** — 1-2 lines naming prior post(s) and how.
  - **Risks** — 1-3 bullets specific to this post.
- **Post manifest** — which ingredient deliverables this series' posts assemble from (e.g. `slide-copy`, `body-copy`, `image-prompts`, `video-spec`), plus any campaign-wide generation preference. One or two lines; recorded into `campaign.md` `post_manifest` when this deliverable locks (schema in [`campaign-frontmatter.md`](../../process/campaign-frontmatter.md)). Name per-post deviations here when a post needs a different ingredient set.

## Hard Constraints

- Story arc has clear beginning/middle/end
- Every post has a "job in the arc" stated
- CTAs are context-appropriate (no "follow me!" on a problem-statement post)
- Callbacks reference specific prior posts when claimed
- Hook angle for each post is locked, with 1-line rationale (why this angle, not an alternate)
- No two posts have the same job (no two "hooks," no two "payoffs")
- Risks per post are not empty
- Audience extraction language tells and objections are grounded in `research-artifact-v{N}.md` citations or quoted source material, not LLM-prior assumptions.
- Per-post sections stay tight: no numbered story-shape narratives, no hero-image notes, no thesis paragraphs, and no slide-by-slide content (those belong in the post's ingredient deliverables — slide-copy, body-copy, image-prompts).
- Anchored in `positioning.md` POV stances
- Tone is tight; skip qualifiers

## Draft Frontmatter Convention

Standard deliverable frontmatter per `library/process/campaign-frontmatter.md`. Canonical file path depends on the selected flow:
- Standard Flow: `phase-3-planning/campaign-architecture/draft-v{N}.md`
- Solo Flow: `phase-1-research/campaign-architecture/draft-v{N}.md`

## Lock Criteria

- User-approved
- `post_manifest` recorded into `campaign.md` in the lock turn
- Lock event mechanics per `library/process/lock-event.md`

