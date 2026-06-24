# Marketing production routing

The domain-specific production/delivery routing the Operator lazy-loads when doing marketing production work (the persona's generic "domain production / delivery work" row delegates here for `domain: marketing`). The Operator persona itself names no marketing artifact; this fragment carries them.

| Situation | Load First | Also Load If Needed |
|---|---|---|
| Post production (slide / body / image / video) | the ingredient template named by `post_manifest` (`library/domains/marketing/deliverables/{ingredient}/template.md`, or the shared `library/deliverables/{ingredient}/template.md` for image-prompts/video-spec), campaign-architecture, [voice], [positioning] | the post-final template when an ingredient locks or the post assembles |
| Carousel or visual post | the visual deliverable template, [voice], campaign-architecture | [preview-server] for preview offering and hub hygiene |
| Publish coordination | the post-final template, the post's `post-FINAL.md` named by the tracker, `activity.md` | [voice-mini-retro] if shipped copy materially differs |
| Performance capture | the flow's performance-capture step, each post's `post-FINAL.md` frontmatter | live Composio/Rube tool search for the shipped platform; closeout retro only if closing the project |

## Publishing

When the operator provides a published post link, follow the Publish / Export Mechanics procedure in [`deliverables/post-final/template.md`](deliverables/post-final/template.md). State transitions are button-owned: `python system/af.py publish` does the mechanics (delivered-state record on `post-FINAL.md`, tracker, `posts_published`, lifecycle `shipped_at`) and prints the judgment checklist.

## Output rule

Match the CTA to the post's role in the campaign arc.
