---
domain: creator
topology: studio
prefix: null
extension_fields: []
verbs: [ready, version, doctor]
exportable: []
assembly_record: null
flows: []
---

# Creator domain pack

The creator domain: a standing short-form channel that ships talking-head videos. Creator work is **not** project-shaped. It lives on the singleton studio surface `workspace/studio/`: a rolling calendar of posts, series that open and close inside it, no terminal retro. That is declared here via `topology: studio`. `af new-project` refuses this domain; posts scaffold with `af studio new`.

This pack is the only artifact that knows creator work. The generic spine (`system/af.py`) reads what is declared here; it names no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Topology | `topology: studio` above | posts scaffold under `workspace/studio/posts/{slug}/`, state-tracked on the board (`calendar.md`), never under `workspace/projects/` |
| Board skeleton | [`calendar-skeleton.md`](calendar-skeleton.md) | the `calendar.md` created on the first `af studio new` |
| Post skeleton | [`post-skeleton.md`](post-skeleton.md) + [`edit-skeleton.md`](edit-skeleton.md) | the `post.md` record and the `edit.md` decision list every post gets |
| Runbook | [`production.md`](production.md) | the per-post order of operations and the situation load map the Operator uses for creator work |
| Edit route | [`short-form-edit.md`](../../process/short-form-edit.md) | footage-first production; shared, so a project can borrow it |
| Intake route | [`media-intake.md`](../../process/media-intake.md) | how captures enter the shared media shelf; shared, never studio-only |
| Channel profile | `library/context/channels/tiktok/profile.md` | platform constraints; a gitignored instance of the channel-profile shape |
| Deliverable templates | none | the `edit.md` shape lives in the edit route; a `script.md` is drafted against the generic deliverable shape and may be versioned; templates arrive only when a retro earns them |
| Doctor rules | none | the spine's studio checks (state validity, row and folder reconciliation, overdue slots, missing renders, capture due) suffice |

## State model (the board owns this)

`planned → captured → cut → scheduled → posted`, terminal `dropped`.

A post is born `planned` when it has a date and nothing shot, or `captured` when it arrives as a recording. `cut` means a render exists. `scheduled` means it sits in the platform's queue. `posted` carries the receipt. Measurement is a field on the post, never a state; `af doctor studio` nudges when it is due.

Transitions are enforced by `af studio stage`. `scheduled` requires `renders/final.mp4`; `posted` is written by `af studio post` with the live URL. State lives only on the board; `post.md` holds the fixed facts and the receipt.

## Locked versus convention

Two things are schema: the board and one `post.md` per post. Everything else is living markdown, born when earned, edited in place, never versioned: `studio.md` (the charter), `ideas.md`, `hooks.md`, `community.md`, `series/{slug}.md`, `research/`, `performance.csv`, and each post's `edit.md`. A `script.md` is the one file that may take `-v{N}` when a scripted piece goes through real drafts.

## Where intent lives

Person over channel over post. `library/context/operator/positioning.md` holds the cross-channel narrative and stances. `workspace/studio/studio.md` holds this channel's why, niche, audience, formats, cadence, series roster and parked ideas. A post's `edit.md` holds one piece. Stances that stabilize in the charter promote into positioning at retro through the mechanism positioning already declares.
