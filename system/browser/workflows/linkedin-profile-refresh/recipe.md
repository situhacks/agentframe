# LinkedIn Profile Refresh

workflow_id: linkedin-profile-refresh
status: cursor_replay
browser: chrome
app_url: https://www.linkedin.com/in/{operator-handle}/
approval_mode: human_review
tool_routing: browser_use_only

## Job

Capture the operator's own live LinkedIn profile to a dated snapshot in
`library/context/channels/linkedin/`, so profile drift is observable over time. The live profile is
canonical; version files in the archived revamp project are not.

## Inputs

- The operator's own LinkedIn handle, substituted into `app_url` above.
- Controlled Chrome profile running: `npm run home-browser` from `system/browser` (port 9223).
- `BU_CDP_URL=http://127.0.0.1:9223` exported before invoking browser-harness.
- Prior snapshot in `library/context/channels/linkedin/` for the delta table.

## Path

1. Start the browser, confirm the session is signed in (`page_info()` on `/feed/`, not `/authwall`).
2. Navigate to the profile, `wait_for_load()`, then sleep ~3s — LinkedIn hydrates after load fires.
3. Scroll-load the page (~14 × 900px, 0.5s apart), return to top, capture `document.querySelector('main').innerText`.
4. Capture the three details subpages the main page truncates:
   `/details/skills/`, `/details/certifications/`, `/details/experience/` — same scroll-then-read pattern.
5. Screenshot at fixed scroll offsets into `system/browser/local/screenshots/linkedin-{date}/`.
6. Write `profile-snapshot-{YYYY-MM-DD}.md` with a delta table against the prior snapshot.

## Known Controls

- Analytics (profile views / post impressions / search appearances) sits in the main page's top card,
  labelled "Private to you". Only visible while signed in as the owner.
- **Skills order on `/details/skills/` is the profile order** — the top entries are the pinned ones.
  The main page shows only a truncated "Top skills" chip and hides the tail.
- Certifications show issue and expiry dates only on the details page.
- Experience bullets are truncated on the main page; the details page carries them in full.

## Browser-Harness Execution

- **`js(...)` innerText extraction, not screenshots, for all text.** Screenshots are for the banner and
  visual layout only. The AX tree is enormous on this page and unnecessary — there is no clicking to do.
- **`screenshot()` is not a defined helper.** Use raw CDP:
  `data = cdp("Page.captureScreenshot", format="png")["data"]` then `base64.b64decode`.
- The harness runs Python — write captures to disk directly from inside the heredoc rather than piping
  a 30k-character dump through stdout.
- Everything below the fold is lazy-loaded. Reading `main` without scrolling first returns a partial page.

## Human Gate

**Sign-in is human-owned.** The controlled Chrome profile is dedicated, so it does not inherit the
operator's everyday Chrome session — the first run on any new profile lands on `/authwall` and stops
there for the operator to log in. Never enter credentials, handle MFA, or store session tokens.

Stop after writing the snapshot. Copy changes to the live profile are the operator's to make by hand;
this workflow observes and reports, it never edits the profile.

**Scope limit:** the operator's own profile only. `system/skills/job-scout/SKILL.md` forbids automating
LinkedIn for job discovery — searching, scraping postings, or visiting other people's profiles is out of
scope for this recipe and must stay that way.
