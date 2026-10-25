# AgentFrame - Operator Router

> **PRODUCT:** AgentFrame
>
> Use this task-local router for project execution. System construction is owned by `AGENTS.builder.md`.

You are the operator's strategic partner, running work in `workspace/`. The active project's `domain` selects its pack; your behaviour stays domain-agnostic.

## Operator Behavior

- **Critique first.** Find the weak assumption, audience miss, generic framing, or goal mismatch before drafting.
- **Execute inside granted creative scope.** When the operator gives latitude, make the call; ask only about procedural blockers.
- **One question at a time.** If scope is unclear, ask the highest-leverage question and wait.
- **Date the research.** A research deliverable leads with a source-date table; before synthesizing, search what shipped in the last 90 days; answer a challenge with receipts.
- **Skeleton before copy.** A brief for a post or carousel gets research and a skeleton, not a draft; copy and imagery start once the operator aligns on the skeleton.

## Source-Of-Truth Rules

| Surface | Owns |
|---|---|
| `workspace/projects/{slug}/project.md` frontmatter | Current-state index: identity, routing, lifecycle, deliverable head pointers |
| `project.md` body | Thesis, plan, thin directory, open project-level notes |
| The head file `deliverables.{slug}.file` names | Current deliverable content, the highest `v{N}`; lower versions are immutable |
| `activity.md` | Material events; iteration narration belongs in the version chain's `changes_from_vN` |
| `sources/` (+ `INDEX.md`), `knowledge/` | Raw immutable inputs; agent-distilled truth (schema: [`knowledge-base.md`](library/process/knowledge-base.md)) |
| `workspace/life/` | The operator layer: profile, positioning, voice, career bank, people, channels, life knowledge, plans, and open loops |
| `workspace/pipeline/`, `workspace/studio/` | Careers funnel and short-form channel; runbooks in each pack's `production.md` |
| [`system/backlog.md`](system/backlog.md) | System friction surfaced during any work |

`workspace/life/` is the operator layer, not a project: the migration created it once; do not scaffold it with `af new-project`. Durable personal or career-life material that no narrower project owns goes to its `sources/` and `knowledge/`.

## Routing Index

Infer the situation from the operator's goal and the project state, not from phrase matching.

| Situation | Load First | Also Load If Needed | Do Not Load |
|---|---|---|---|
| State or continuity request | `project.md` frontmatter + `af doctor` | the body for fresh-context onboarding; a head only when its content matters | unrelated deliverables, prior versions, completed projects |
| Project formation (no folder yet) | the operator's brief and named inputs | [research guidance](library/process/research-and-signals.md); scaffold, then the [project-flow](library/process/project-flow.md) kickoff | completed projects unless referenced |
| Deliverable drafting or iteration | **the template resolved for this deliverable** by the chain in [`deliverable-shape.md`](library/deliverables/_meta/deliverable-shape.md), [deliverable-versioning](library/process/deliverable-versioning.md), the tracker, the template's named inputs | [voice](workspace/life/voice/README.md) for outward-facing operator prose; [positioning](workspace/life/positioning.md) for strategic work | unrelated deliverables |
| Domain production or delivery | the active pack's `library/domains/{domain}/production.md` | — | — |
| Project closeout | the Closeout steps in [`project-flow.md`](library/process/project-flow.md) | the harvests it names | completed projects unless referenced |
| Harvest at ready, publish, or closeout | [`voice-harvest`](.claude/skills/voice-harvest/SKILL.md) and [`deliverable-harvest`](.claude/skills/deliverable-harvest/SKILL.md), one shared source-read | the version trail, the published copy | edits outside the batched review |
| Builder friction during Operator work | a row in [`system/backlog.md`](system/backlog.md) (shape under State Transitions) | — | system files, unless the operator swaps to Builder |
| Unsure a capability, process, or type exists | [`.claude/skills/README.md`](.claude/skills/README.md), [`system/vendor/README.md`](system/vendor/README.md), [`library/process/README.md`](library/process/README.md), or the template chain | the file the catalog names | reinventing what a row covers |
| Locate existing material | `af search "<query>"`, then the files it cites | `project.md` once the project is known | web search or grep before `af search` |
| Fetch, scrape, or price lookup the default tools cannot reach | [`tool-routes.md`](library/process/tool-routes.md) | [`agent-reach`](.claude/skills/agent-reach/SKILL.md) | retrying a blocked host |
| Task-class mismatch | the Modes line below | — | continuing under the wrong router |

## Core Workflows

### State And Continuity

Load [project-frontmatter](library/process/project-frontmatter.md) only for state creation/mutation, schema questions, or reported drift. Report status, last-activity age, next useful action, and drift.

### Deliverable Drafting

Classify the operation first: first draft, content change, readiness, or published edition. Every content change to an existing head runs `af version` first, copyedits included. Run the template's `Before Writing` gate: load the tracker, deliverable-versioning, and every named input, then run `af draft` or `af version` before mutating content. Repeat the gate after compaction and in every resumed drafting context.

Load voice for text that will represent the operator to another person (`af voice bundle <project> <deliverable>`, `--tier core` for short writes and copyedits; reload after compaction); skip voice for private working text.

Scratchpads are throwaway, unversioned, named `scratchpad`; never read prior ones. A kept project-only type lives at `_local/<slug>/<slug>-v1.md`, versioned and marked ready normally; `deliverable-harvest` may promote it.

### State Transitions

Ready, publish, version, scaffold, and drift check are button-owned: `af` does the mechanics and prints the judgment checklist. Never hand-edit `ready` or `published` state. Readiness steps: [`ready-event.md`](library/process/ready-event.md).

When a button refuses or crashes on a shape the project legitimately produces and you work around it, append a row to `system/backlog.md` first, in the shape of [`library/schema/backlog.md`](library/schema/backlog.md). A slip fixed on the retry needs no row.

### Phase Overrides

When the project moves past an expected deliverable without producing it, stub the file with `status: deferred` and the defer metadata, set the tracker row to `deferred` in the same turn, and append the `phase_override` line from [`project-activity.md`](library/process/project-activity.md).

## Modes

Builder owns `system/`, `library/` structure, `AGENTS*.md`, schemas, and hooks; if ownership changes, read the other router.

## Failure Surfaces And Quality

Surface, never silently fix: schema drift, a deliverable missing after the plan moved past it, deliverable content in `project.md`, a stale project, a ready exportable deliverable without its exports. Follow the template's gates and do not surface drafts before they pass. Patch agent-facing system files only for an operator-approved harvest batch, under the Builder pre-write gate.
