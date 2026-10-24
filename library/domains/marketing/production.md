# Marketing production routing

The domain-specific production/delivery routing the Operator lazy-loads when doing marketing production work (the persona's generic "domain production / delivery work" row delegates here for `domain: marketing`). The Operator persona itself names no marketing artifact; this fragment carries them.

| Situation | Load First | Also Load If Needed |
|---|---|---|
| Post production (slide / body / image / video) | the ingredient template named by `post_manifest` (`library/domains/marketing/deliverables/{ingredient}/template.md`, or the shared `library/deliverables/{ingredient}/template.md` for image-prompts/video-spec), campaign-architecture, [`voice`](../../../workspace/life/voice/README.md), [`positioning`](../../../workspace/life/positioning.md) | the [`post-final`](deliverables/post-final/template.md) template when an ingredient becomes ready or the post assembles |
| Carousel or visual post | the visual deliverable template, [`voice`](../../../workspace/life/voice/README.md), campaign-architecture | — |
| Publish coordination | the [`post-final`](deliverables/post-final/template.md) template, the post's `post-FINAL.md` named by the tracker, `activity.md` | the target channel profile; [`substack-publishing`](../../process/substack-publishing.md) when the target is Substack; at publish, [`voice-harvest`](../../../.claude/skills/voice-harvest/SKILL.md) and [`deliverable-harvest`](../../../.claude/skills/deliverable-harvest/SKILL.md) unconditionally on a user-voiced post (corpus promotion is eligible once the final is live; [`ready-event`](../../process/ready-event.md) holds the gate) |
| Performance capture | each post's `post-FINAL.md` frontmatter | [`performance-capture`](../../process/performance-capture.md) for the shipped platform; [`project-flow`](../../process/project-flow.md) closeout only if closing the project |

## Campaign phase checklist (optional)

A campaign may borrow this sequence into its plan ([`project-flow`](../../process/project-flow.md)) by name; phases merge, skip, or reorder as the campaign needs. Folders are `phase-{n}-{name}/`.

| Phase | Produces | Gate to advance |
|---|---|---|
| 1 Research | `idea-bank.md` and `research-artifact-v{N}.md` ([`research-and-signals`](../../process/research-and-signals.md)) | Operator accepts the research artifact |
| 2 Strategy (stakeholder campaigns) | business-brief, then campaign-brief; stakeholder feedback lands while each is drafting | Business brief ready before the campaign brief starts; campaign brief ready before planning |
| 3 Planning | campaign-architecture with a breakdown of every post; design-language in parallel | Architecture ready; design language ready, or deferred for text-only work. Record `post_manifest` and one `post-{n}` row per planned post at `not_started` |
| 4 Production and launch | per-post ingredients named by `post_manifest`, assembled in `post-FINAL.md` | Every active post published, deferred, or removed from scope |
| 5 Measure | `performance-data.csv` per [`performance-capture`](../../process/performance-capture.md), about 14 days after each post | Closeout per [`project-flow`](../../process/project-flow.md) |

A solo campaign folds phases 1 and 3 into one research-and-architecture phase and skips phase 2.

Inside the sequence:

- The campaign-architecture breakdown covers every post before production starts; posts in a series call back to each other.
- Within a post, slide copy is ready before body copy drafts; image prompts consume the design language's treatment block and the ready slide text.
- A post row points at `post-FINAL.md` from its first drafting ingredient and stays `drafting` until every manifest ingredient is in.
- Skipping a phase: name the missing prerequisite and the cleaner path once; if the operator insists, log `phase_override` to `activity.md` ([`project-activity`](../../process/project-activity.md)) and proceed.

## Publishing

When the operator provides a published post link, follow the Publish / Export Mechanics procedure in [`deliverables/post-final/template.md`](deliverables/post-final/template.md). State transitions are button-owned: `python system/af.py publish` does the mechanics (published-state record on `post-FINAL.md`, tracker, derived published-post receipt, project `shipped_at`) and prints the judgment checklist.

## Output rule

Match the CTA to the post's role in the campaign arc.
