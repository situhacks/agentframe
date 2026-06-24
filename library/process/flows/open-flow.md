# AgentFrame Marketing — Open Campaign Flow

Freeform flow for campaigns that don't fit a fixed phase ladder. Open flow is composition freedom, not absence of structure: the agent proposes a plan scaled to the objective, the operator narrows it, and every artifact still rides the system's spine so the work is reviewable from disk afterwards.

## Read Once

- Every artifact is a versioned deliverable instance per [`deliverable-versioning.md`](../deliverable-versioning.md): `{name}-v{N}.md`, head named by the tracker. Revisions bump the head (`python system/af.py version`) — they never spawn a new differently-named v1. Only scratchpads are throwaway, and they carry `scratchpad` in the filename.
- State transitions are button-owned: `python system/af.py` (`lock`, `publish`, `version`, `new-campaign`, `doctor`); lock triggers per [`lock-event.md`](../lock-event.md). Never hand-edit a terminal `status:`.
- Tracker schema: [`project-frontmatter.md`](../project-frontmatter.md). Apply file edits and `project.md` updates in the same turn.

## Kickoff — propose the plan

The objective is usually known by the time an open-flow campaign starts. The agent's first move is a proposed plan, scaled to the job — then the operator narrows:

1. **Phases** — as many as the objective needs: none for a single ad-hoc deliverable, several named phases for a longer project. Phase ids are campaign-defined (kebab-case, ordered).
2. **Deliverables** — composed from the library first. Reuse existing templates; borrow structured-flow fragments by name ("marketing-solo-flow phases 1 and 3, skip the briefs"); name any ad-hoc artifact that matches no template. Selection menus apply ([`image-production.md`](../image-production.md), [`deck-production.md`](../deck-production.md)).
3. **Manifest moment** — when posts are in the plan, record `post_manifest` in `project.md` now.

The agreed plan lands in the `project.md` body; tracker rows are added at `not_started`.

## The plan section (project.md body)

Short and current:

- **Objective** — one line.
- **Phases** — each with its deliverables ("single-phase" for ad-hoc jobs).
- **Runway** — the next 1–2 steps, pre-staged: what each produces and what it needs.
- **Parked** — ideas not in play.

## The Loop

1. Work the runway's first item using its deliverable template (or the ad-hoc artifact's agreed shape).
2. On lock: run lock-event, sync the tracker, refresh the plan section, and PROPOSE the next 1–2 runway steps — don't make the operator plan from scratch each turn.
3. Recalibrate when scope moves: phases can be added, merged, or dropped. Log plan changes to `activity.md` as `plan_revised` events — the decision trail lives on disk, not in the context window.

## Files

- With phases: `phase-{n}-{name}/` folders, versioned artifacts inside (same shape as the structured flows).
- Zero-ladder: versioned artifacts in one folder per deliverable at the campaign root.

## Tracker Updates

- New open campaigns scaffold via `python system/af.py new-campaign <slug> --flow open-flow`; set `current_phase` to the plan's first phase id once the plan locks (`active` when single-phase).
- `current_phase` values come from the plan section; the schema-drift check validates against that list.
- Deliverables move `not_started -> drafting -> locked -> delivered` in the same turn as their files change; the active pack owns any domain-specific assembly step.

## Closeout

Same two-step learning close as the structured flows: harvest retro (`system-retro-v{N}.md`, both harvest skills with a shared source-read) → performance capture + campaign retro + completion. `LIFECYCLE.status: complete` only after the campaign retro locks or the operator records a closeout override in `activity.md`.
