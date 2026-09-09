# Creator production — the studio runbook

Operator-mode routing for `domain: creator` work. The board (`workspace/studio/calendar.md`) owns post state; buttons own transitions; [`short-form-edit.md`](../../process/short-form-edit.md) owns the craft. This file owns the order of operations and the load map.

## Load map

Load the first column, then only what the second names. The third column is what a fresh context does not open.

| Situation | Load first | Also if needed | Never by default |
|---|---|---|---|
| Continuity: what is coming up, where are we | `calendar.md` frontmatter + `af doctor studio` | `studio.md` on a fresh context or a strategy question | post folders, hooks, research |
| He hands over recordings, footage or photos | [`media-intake.md`](../../process/media-intake.md) | [`library/assets/README.md`](../../assets/README.md) for the record schema | the studio at all; intake is shared |
| A recording arrives to be edited | [`short-form-edit.md`](../../process/short-form-edit.md), the channel profile, the video design language the profile names | `hooks.md` for the hook check; `af search` for cutaways; `studio.md` when the piece needs a frame | other posts' `edit.md` |
| Plan or script a post or a series | `studio.md`, [`positioning`](../../context/operator/positioning.md), [`voice`](../../context/operator/voice/README.md) | `hooks.md`, `series/{slug}.md`, `ideas.md` | research pulls |
| Creator study or hook research | `research/whitelist.md` + the study section of `short-form-edit.md` | `hooks.md` to append | posts |
| Schedule or publish a batch | the browser workflow recipe named by the channel profile, `af studio stage` / `af studio post` | the calendar mirror step below | anything else |
| Performance capture | [`composio-notes.md`](../../process/composio-notes.md), `performance.csv`, `hooks.md` | the post's `post.md` | renders |

## The post

| Step | Do | Owner |
|---|---|---|
| 0 Arrive | Two entry points. **Captured-first** (the default): he records and drops a file. **Scripted**: an idea gets a date and a `script.md` first | operator |
| 1 New | `af studio new <slug> --from <file>` (born `captured`) or `af studio new <slug> --date D` (born `planned`); adds the board row, scaffolds `posts/{slug}/` with `post.md` and `edit.md` | spine |
| 2 Transcribe | `python system/tools/transcribe.py video/source.mp4` → `video/transcript.json` (word timestamps, HyperFrames shape) | tool |
| 3 Cut | Mark retakes, fillers, dead air in `edit.md`; compile with `transcript-cut.mjs`; encode `video/cut.mp4` | edit route |
| 4 Decide | Fill `edit.md`: hook, punch-ins, b-roll slots via `af search`, text cards, SFX, captions, end card, platform caption | edit route |
| 5 Compose and render | HyperFrames project under `video/`; `npx hyperframes check`; render 1080×1920 to `renders/final.mp4`; `af studio stage <slug> cut` | edit route + spine |
| 6 QC | First three seconds, safe zones, duration, size, no watermark. **The operator watches the render before it is scheduled** | operator |
| 7 Schedule | The channel's browser workflow recipe uploads and schedules; human gate before the final click unless the run is authorized; `af studio stage <slug> scheduled`; mirror the slot to the operator's calendar (below) | recipe + spine |
| 8 Post | When live: `af studio post <slug> --url U [--posted-at T]` writes the receipt | spine |
| 9 Measure | Around day 14 `af doctor studio` nudges; capture per `composio-notes.md` into `performance.csv`, `unknown, not zero` for what no surface exposes; stamp `metrics_captured_at` | operator + process |
| 10 Learn | Score the hook row in `hooks.md`; propose next week's hooks from what retained. One batched weekly review, no thresholds | agent |

Batching: the native TikTok scheduler covers ten days ahead. Beyond that the queue stays on the board at `cut` and a later session schedules the next window.

## Where files live

| Tier | Files | Rule |
|---|---|---|
| Locked (spine-owned) | `calendar.md`, `posts/{slug}/post.md` | buttons write state; never hand-edit a state field |
| Living (convention) | `studio.md`, `ideas.md`, `hooks.md`, `community.md`, `series/{slug}.md`, `research/`, `performance.csv`, `posts/{slug}/edit.md` | born when earned, edited in place, never `-v{N}` |
| Media | `posts/{slug}/video/`, `posts/{slug}/renders/` | untracked; the post's own recording stays here; anything reusable is registered on the media shelf per `media-intake.md`, by path, never copied |

Reusable captures never live in the studio. They live on `library/assets/media/` and are found with `af search`.

## Standing rules

- **Voice.** Studio posts default to the informal base recipe; resolve it through the voice README, never from the platform.
- **Design.** The video design language named by the channel profile is the only design input for a recut. Never load HyperFrames' creative house style for one.
- **Engagement.** No automation on the platform, ever. `community.md` holds peers and drafted comments, stitches and DMs; the human posts them.
- **Calendar mirror.** At `scheduled` and `posted`, push a one-way event (title, platform, post link) to the operator's Google Calendar through the connected calendar tool when it is authorized. The board stays truth; the mirror never writes back.
- **Perception is delegated, judgment is not.** Watching footage, bulk tagging, and long transcripts go to Gemini through `system/tools/agy_call.py` (the operator's Antigravity login); what to cut, what to say, and what ships stay with the session.
- **Overlap with projects.** A short that recaps a project essay is a studio post whose `post.md` names `source_project`. That is a link, not a shared folder.
