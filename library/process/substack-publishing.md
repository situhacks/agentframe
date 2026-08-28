# Substack Publishing

## Purpose

Prepare Substack drafts from canonical project copy, hand off editor-only actions, and reconcile the live publication back into AgentFrame. Publication identity, credentials, and formatting conventions live in [`library/context/channels/substack/profile.md`](../context/channels/substack/profile.md).

## When To Load

Load when a project targets Substack and:

- ready copy and selected media are ready for a draft;
- an existing Substack draft needs a substantive update; or
- a published post is being back-published to Substack.

Also load the canonical source named by the project tracker and the Substack channel profile. When copy is adapted rather than transferred verbatim, resolve its voice recipe independently from the platform; Substack does not imply the informal register.

## Procedure

1. **Resolve the source.** Use the tracker-named ready head: `post-FINAL.md` for an assembled marketing post, or the ready `substack-essay-v{N}.md` for a native essay. Do not reconstruct copy from a PDF, a live LinkedIn post, or an obsolete draft.
2. **Prepare the Substack payload.** Set title, subtitle, and body from the source plus the channel-profile conventions. For a back-published post, include the LinkedIn permalink footer and recover the original date from project state; decode an activity ID only when the stored date is missing. If prose changes, run the full Substack voice sequence and its humanizer gate before surfacing the draft.
3. **Prepare media.** Convert carousel PDFs to ordered per-slide PNGs and place the publish candidates under the post's `media/substack/` directory. In the Substack body, stack slides full-width above a `* * *` divider. Use the MCP's `upload_image` only when its returned CDN URL can be placed in the draft reliably; otherwise hand image placement to the editor. Use the native Subscribe button, never an in-body subscribe link.
4. **Create or update the draft.** The source head must be `ready` before copy goes up: `publish_guard.py` refuses an `update_draft` carrying title, subtitle, or body when the tracker row holding that `substack_draft` id sits below ready. This is the last point AgentFrame can see the copy, because step 5 happens in the editor. Use `create_draft` for a new draft and `update_draft` for an existing one. Do not use `list_drafts` while its `drafts.map is not a function` failure persists; use the known draft identifier, `list_published_posts`, or the editor. `create_note` and `create_note_with_link` publish immediately, so never call them without explicit per-note operator approval.
5. **Hand off editor-only actions.** The operator publishes and sets displayed date, cover image, tags, section, native Subscribe button, settings, or video in the Substack editor. For backdating, publish first, then use **Settings → Displayed Publication Date**. Treat a one-day UTC display shift as a date check, not a reason to rewrite project history.
6. **Reconcile the live result.** Before publishing state is recorded, compare the live copy with the ready source. If it differs materially, create a new version/edition and reconcile there. Then run `python system/af.py publish <project> <row> --url <url>`. For `post-FINAL.md`, the marketing hook also records platform, posted date, shipped media, derived post totals, and the `post_published` event. For a standalone essay, the shared transition records `published_url`, marks artifact and tracker immutable, and writes the generic publish event.

## Verification Or Logging

Before calling the publication reconciled, verify:

- the live title, subtitle, body, image order, and displayed date match the approved draft;
- no `[FILL]`, `[POV]`, `[NERD-NOD]`, or other draft placeholders shipped;
- the native Subscribe button and any editor-only metadata were handled deliberately;
- the canonical source records the live URL and the exact media that shipped; and
- the project activity trail contains one material publish event, not draft-preparation narration.

## Boundaries

- Content shape and readiness criteria belong to the calling deliverable template, especially [`substack-essay`](../domains/marketing/deliverables/substack-essay/template.md) and [`post-final`](../domains/marketing/deliverables/post-final/template.md).
- Publication URL, credentials, session rotation, series labels, footer wording, and CTA conventions belong to the Substack channel profile.
- Generic post state transitions belong to `system/af.py` and the `post-final` template.
- Performance capture belongs to [`composio-notes.md`](composio-notes.md).
- Publication-level optimization such as welcome email, About page, recommendations, naming, and pinned-post strategy is operator work, not part of each publish run.
