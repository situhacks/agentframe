# Project Knowledge Base Schema & Workflows

To ensure long-horizon projects do not suffer from context bloat while keeping all project information accessible, each project maintains a structured knowledge substrate.

## Workspace Layout

```
workspace/projects/{slug}/
├── sources/               # Raw, IMMUTABLE inputs (transcripts, docs, letters)
│   └── INDEX.md           # Source inventory and citation index
└── knowledge/             # Agent-OWNED, distilled truth (entity pages, decisions)
    ├── people/            # Per-project stakeholder overlay pages
    ├── meetings/          # Meeting notes and meeting index
    ├── decision-log.md    # Dated append-only log of decisions
    ├── raid-log.md        # Risk, Assumption, Issue, Dependency log
    ├── workback-schedule.md  # Milestone WBS planned backward from deadlines
    └── _archive/          # Dream-pass output: stale/closed entries
```

## Storage Principles

1. **Sources are Immutable:** The agent never edits or writes files inside `sources/` (except `sources/INDEX.md` when registering a new source). If a source has errors, the correction is documented in `knowledge/`, not in the source file.
2. **Knowledge is Agent-Owned:** Distilled and written by the agent based on evidence from `sources/` or live chat. It is structured to stay lean and current.
3. **Compiled-Truth + Timeline:** Entity pages (e.g., meeting index, stakeholder overlays) use a two-part layout:
   - A **Compiled-Truth Header** at the top summarizing what is true *now*.
   - A **Dated, Append-Only Timeline** at the bottom showing the evidence/history.
   - When new evidence is added to the timeline, the header must be refreshed. If the header is older than the latest timeline entry, the page is flagged as out of date.

## Ingest Workflow (Sources → Knowledge)

When the operator pastes a new transcript, brief, or document:
1. Write the raw content to `sources/{type}-{date}-{slug}.md`.
2. Register the file in `sources/INDEX.md` with:
   - `id`: unique kebab-case ID (e.g., `src-2026-06-25-01`)
   - `file`: relative path
   - `date`: date received
   - `description`: 1-sentence summary
3. Extract new decisions, risks, stakeholders, or milestones and update the corresponding `knowledge/` files. Reference the source ID for all extractions (e.g., `[src-2026-06-25-01]`).

## Living Governance Docs

**When these exist — instantiation is flow-driven, not automatic:**

- Under **`project-mgmt-open-flow`** they are derived at kickoff from the charter (see [`project-mgmt-open-flow.md`](flows/project-mgmt-open-flow.md)).
- Under **`open-flow`** (the default, including PM one-offs) none are created at kickoff; a doc is born only when the ingest workflow above has real evidence for it — a decision creates `decision-log.md`, a risk creates `raid-log.md`. A pure one-off creates none.

1. **`knowledge/raid-log.md`:** Holds open risks, assumptions, issues, and dependencies. Every entry has exactly one named owner. Stale entries are flagged during weekly reviews. Replaces separate risk registries.
2. **`knowledge/decision-log.md`:** Dated append-only log: `Decision` · `Rationale` · `Owner`.
3. **`knowledge/stakeholder-map.md`:** Cross-references global person profiles and links roles to their per-project stakeholder overlay pages at `knowledge/people/{person-slug}.md`.
4. **`knowledge/workback-schedule.md`:** Milestones/tasks planned backward from deadlines. Updated continuously; does not lock.
