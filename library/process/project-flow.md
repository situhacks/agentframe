# AgentFrame — Project Flow

The one project shape. A project is `project.md`, its versioned deliverables, `sources/` and `knowledge/` ([`knowledge-base.md`](knowledge-base.md)), and `activity.md`. The agent proposes a plan scaled to the objective, the operator narrows it, and every artifact rides the system's spine so the work is reviewable from disk afterwards.

## Read Once

- Every artifact is a versioned deliverable instance per [`deliverable-versioning.md`](deliverable-versioning.md): `{name}-v{N}.md`, head named by the tracker. Revisions bump the head (`python system/af.py version`); they never spawn a new differently-named v1. Only scratchpads are throwaway, and they carry `scratchpad` in the filename.
- State transitions are button-owned: `python system/af.py` (`draft`, `adopt`, `ready`, `publish`, `version`, `new-project`, `doctor`); readiness triggers per [`ready-event.md`](ready-event.md). Never hand-edit tracker `status:`.
- Tracker schema: [`project-frontmatter.md`](project-frontmatter.md). Apply file edits and `project.md` updates in the same turn.

## Kickoff — propose the plan

The objective is usually known by the time a project starts. The agent's first move is a proposed plan, scaled to the job; then the operator narrows it:

1. **Phases** — as many as the objective needs: none for a single ad-hoc deliverable, several named phases for a longer project. Phase ids are project-defined (kebab-case, ordered). A domain pack may carry an optional phase checklist to borrow from by name (marketing: the campaign phase checklist in [`production.md`](../domains/marketing/production.md)).
2. **Deliverables** — composed from the library first. Reuse existing templates. If you need a kept, versioned, project-scoped artifact that matches no library template, mint it as a **local project-level deliverable** under `_local/<deliverable-slug>/` using the generic shape. Selection menus apply ([`image-production.md`](image-production.md), [`deck-production.md`](deck-production.md)).
3. **Manifest moment** — when posts are in the plan, record `post_manifest` in `project.md` now.

The agreed plan lands in the `project.md` body; tracker rows are added at `not_started`.

## The plan section (project.md body)

Short and current:

- **Objective** — one line.
- **Phases** — each with its deliverables ("single-phase" for ad-hoc jobs).
- **Runway** — the next 1–2 steps, pre-staged: what each produces and what it needs.
- **Parked** — ideas not in play.

## The Loop

1. Work the runway's first item using its deliverable template (or for ad-hoc artifacts, create the directory `_local/<slug>/` and the file `_local/<slug>/<slug>-v1.md` following the generic shape).
2. When an item is ready: run ready-event, sync the tracker, refresh the plan section, and PROPOSE the next 1–2 runway steps; don't make the operator plan from scratch each turn. `_local` deliverables use `af version` and `af ready` identically to library-backed deliverables.
3. Recalibrate when scope moves: phases can be added, merged, or dropped. Log plan changes to `activity.md` as `plan_revised` events using one event per material state change; the first clause names the result, then only the useful consequence or reason.

## Files

- With phases: `phase-{n}-{name}/` folders, versioned artifacts inside.
- Zero-ladder: versioned artifacts in one folder per deliverable at the project root.

## Tracker Updates

- New projects scaffold via `python system/af.py new-project <slug> [--domain <pack>]`; `current_phase` starts `active`. Once the plan is agreed, set it to the plan's current phase id (`active` when single-phase). It is free text the plan section owns.
- Deliverables move `not_started -> drafting -> ready` in the same turn as their files change. `ready -> published` is optional and only applies when the artifact is issued as an immutable record; the active pack owns any domain-specific assembly step.

## Closeout

Closeout starts when the operator says the project is done. In order:

1. Run [`deliverable-harvest`](../../.claude/skills/deliverable-harvest/SKILL.md) and [`voice-harvest`](../../.claude/skills/voice-harvest/SKILL.md) on every published head they have not already covered, and [`career-harvest.md`](career-harvest.md) when the project produced a bankable win.
2. Promote durable `knowledge/` entries to the operator's context layer: person facts from `knowledge/people/` to `library/context/people/{slug}/profile.md` ([`person-profile.md`](../schema/person-profile.md)), operator-level facts to `library/context/operator/`. Project-specific detail stays in the project.
3. Set `status: complete` and `completed_at` (or `cancelled` and `cancelled_at`), then move the folder to `workspace/projects/completed/{slug}/`.
4. Append one `project_closed` line to `activity.md`.
