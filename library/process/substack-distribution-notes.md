# Substack distribution notes

> Working notes from the 2026-06-16 session that set up the Substack MCP and back-published the LinkedIn archive. Not a hardened process yet — see `system/builder-backlog.md` BB-2026-06-16-01 for the hardening task. This file is the source of truth until then.

## What exists

- **Publication:** `bsitu.substack.com` ("brandon's 5-9 newsfeed"). Tagline: "An open newsfeed on navigating the working world…"
- **MCP server:** `@conorbronsdon/substack-mcp` (npx, v0.3.0). Installed in Claude Code (`~/.claude.json`) and VS Code (`%APPDATA%/Code/User/mcp.json`).
- **Auth:** reads three Windows **user env vars** — `SUBSTACK_SESSION_TOKEN` (the `connect.sid` cookie), `SUBSTACK_USER_ID` (520644654), `SUBSTACK_PUBLICATION_URL`. Cookie is NOT in any config file. Sessions last ~90 days; when tools 401, grab a fresh `connect.sid` from the browser and reset the env var.

## What the MCP can and can't do

- **Can:** `create_draft`, `update_draft` (markdown body/title/subtitle), `list_published_posts`, `upload_image` (returns a CDN url; does NOT set the cover).
- **Cannot (all editor-only):** publish, set publish/displayed date, set cover image, tags, sections, settings (welcome email / About / recommendations), native subscribe button, video upload.
- **Notes:** `create_note` / `create_note_with_link` exist but **publish immediately** (no draft state). Never fire without explicit per-note operator go-ahead. Default: draft the text, operator posts it.
- **Bug:** `list_drafts` throws "drafts.map is not a function" — use `list_published_posts` or the editor instead.

## Backdating (confirmed working 2026-06-16)

- Substack supports backdating via the editor: **publish first, then edit → Settings → Displayed Publication Date**. Set any past date. The MCP cannot do this; operator does it after publishing.
- Known wrinkle: storage is UTC, so a late-evening local publish can show as +1 day. Cosmetic; left unfixed this run (dates stayed internally consistent and in order).

## Per-post conventions (this archive)

- **Source of truth:** the campaign's final copy markdown (`copy-vF.md` / `copy-FINAL.md` / highest locked `copy-vN.md`), NOT the PDF and NOT LinkedIn (LinkedIn API can't read own post history; Composio not authed). Use `status: shipped`/`locked` in frontmatter to pick the real final among versioned drafts.
- **Series → subtitle convention:** `"{Series} · Part {N} — {hook}"`.
  - AgentFrame campaign → **"AgentFrame Build With Me"**
  - Enterprise campaign → **"Enterprise AI Adoption POV"**
  - (Third series for future early-career content → **"AI in your early-career"**)
- **Footer on every post:** `*Originally published on [LinkedIn](permalink) on {Month D, YYYY}.*`
- **Date trick:** a LinkedIn activity/ugcPost id encodes the publish time — `new Date(Number(BigInt(id) >> 22n))`. Use it to recover exact dates when frontmatter is fuzzy.
- **Carousels:** Substack has no carousel widget. Republish as **stacked full-width slide images** (export the PDF to per-slide PNGs), placed **above** a `* * *` divider, body below. Operator uploads PNGs in-editor.
- **Subscribe CTA:** operator uses the **native** Subscribe button in-editor, not an in-body markdown link. CTA copy to pair with it (plain & short): "If this was useful, subscribe — new posts on agents and AI adoption, straight to your inbox."

## Voice

- All copy must pass `library/context/operator/voice/voice-profile.md`. Key gotchas hit this run: flowing sentences not short bursts, no three short sentences in a row, **no CTA/sign-off/engagement-question closers**, two-part opposition not triplets, em-dashes allowed but not as every comma (operator asked for fewer this run), Canadian spelling.

## Optimization checklist (status as of 2026-06-16)

Done: 12 posts back-published + backdated; tags created & assigned (operator fixed a name/slug mismatch that 404'd); Enterprise posts use top-divider + native button structure.

Open (all operator-only, in Settings):
- **Welcome email** — drafted, not yet pasted (highest-value remaining).
- **About page** — expand from one-liner to the 3-series version (drafted).
- **Recommendations** — recommend AI / build-in-public Substacks (cross-promo, biggest free growth lever).
- Name consistency: header "newsfeed" vs footer "Brandon's Notebook" — pick one.
- Pinned post: currently v1.0 (a release note); consider the strongest opener or none.
- Verify carousel PNGs + post-4 video actually uploaded.

Drafted onboarding copy (welcome email / About / start-here) lives in the 2026-06-16 chat transcript; re-request if needed. Operator declined a "Start Here" post.
