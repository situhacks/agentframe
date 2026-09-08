# TikTok Studio Schedule

workflow_id: tiktok-studio-schedule
status: learning
browser: chrome
app_url: https://www.tiktok.com/tiktokstudio/upload?from=upload&lang=en
approval_mode: human_review
tool_routing: browser_use_only

## Job

Upload one rendered studio post (`workspace/studio/posts/{slug}/renders/final.mp4`) to TikTok Studio, set its caption, and schedule it for the slot on `calendar.md`, so batches ship without the operator sitting at the upload page. The Content Posting API is not an option: unaudited apps post private-only.

## Inputs

- The post slug; its render at `renders/final.mp4`; its caption and tags from `edit.md` (Platform caption and tags).
- The slot date and time from `calendar.md` (a row at `cut`; the native scheduler covers up to ten days ahead, videos only).
- Controlled Chrome profile running: `npm run home-browser` from `system/browser` (port 9223), `BU_CDP_URL=http://127.0.0.1:9223` exported.
- Whether the operator authorized autonomous completion for this run (default: no).

## Path

1. Confirm the session is signed in (`page_info()` on the upload URL lands on the Studio, not a login page). Always keep `&lang=en` on the URL.
2. Dismiss any "A video you were editing wasn't saved" banner: Discard button near the top, then the red Discard in the confirmation modal; repeat if stacked.
3. Attach the file through `upload_file('input[type="file"]', <absolute path>)`; wait ~12 s per 10 MB for processing.
4. Caption: TikTok pre-fills the filename. Focus the `div[contenteditable="true"][role="combobox"]`, `End`, Backspace it clear, `type_text(<caption with tags>)`, `Escape` to close tag suggestions, click away. Verify with `innerText`.
5. Select the Schedule radio (click the label whose text is `Schedule`). Set the date by clicking the day, then the time through the scroll-wheel picker (see execution notes). Collapse "Show more" before touching the time picker.
6. AI-generated content disclosure: only when the post's `edit.md` says generated visuals or voice appear. It lives under "Show more"; the toggle is `[aria-checked]`; accept the "Turn on" confirmation if shown.
7. **Human gate** (below) before the Schedule button.
8. After the redirect to `/tiktokstudio/content`, confirm the post appears in the scheduled list, then `python system/af.py studio stage <slug> scheduled` and mirror the slot to the operator's calendar per the studio runbook.

## Known Controls

- Schedule radio label text is exactly `Schedule`; the submit button reads `Schedule` only after that radio is selected (otherwise `Post`).
- Date picker: click the date input, then the target day number span.
- Time picker is a virtual scroll wheel, not a native select: each `scroll(x, y, dy=32)` steps +1 unit, `dy=-32` steps −1; the hour column sits left, the minute column right in five-minute steps. Read the default time from the input before stepping.
- Scheduled entries render a narrow no-break space between time and AM/PM; match with `indexOf('12:30')`, never an exact string.

## Browser-Harness Execution

- Mixed signals: `js(...)` for state reads (caption text, toggle state, button text), CDP `click_at_xy` for the time-picker items and the final button (JS `.click()` does not fire on them), `upload_file` for the attach step.
- Never set the caption via `innerHTML`; it breaks React state. Clear then type.
- "Show more" pushes the time picker off-viewport: collapse it, adjust time, expand again if the disclosure toggle is needed.
- A `beforeunload` dialog blocks navigation while an upload is in flight; dismiss with `cdp("Page.handleJavaScriptDialog", accept=True)`.
- Upload size: the vendored notes say <50 MB; measure the real ceiling on the first upload and record it here, then set the render bitrate from it.
- Source of these mechanics: `system/skills/browser-harness/agent-workspace/domain-skills/tiktok/upload.md`; patch this recipe, not that file, when a durable quirk changes.

## Human Gate

**Sign-in is human-owned.** The controlled Chrome profile is dedicated; the first run lands on TikTok's login and stops there for the operator. Never enter credentials, handle codes, or store session tokens.

**Stop before the final Schedule click** and show the operator: the file name, the caption as read back from the page, the disclosure state, and the scheduled date and time. Click only on their word, or when the run was authorized as autonomous up front. The render itself was already reviewed at `cut`; this gate is about what the platform will publish, not the edit.

**Scope limit:** the operator's own account, own renders, scheduling only. No engagement actions of any kind (no comments, likes, follows, duets) from this or any recipe.
