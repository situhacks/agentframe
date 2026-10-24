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
    ├── decision-log.md    # Born on the first real decision (see below)
    ├── raid-log.md        # Born on the first real risk (see below)
    └── _archive/          # Stale or closed entries moved out of live files
```

## Storage Principles

1. **Sources are Immutable:** The agent never edits or writes files inside `sources/` (except `sources/INDEX.md` when registering a new source). If a source has errors, the correction is documented in `knowledge/`, not in the source file.
2. **Knowledge is Agent-Owned:** Distilled and written by the agent based on evidence from `sources/` or live chat. It is structured to stay lean and current.
   - Distill durable chat or transcription signal directly into the smallest relevant knowledge file. Preserve a raw transcript only when it is supplied or explicitly requested, and create no topic structure until the material earns it.
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

### Pulling MeetCap transcripts

MeetCap (`~/Documents/MeetCap` by default; the app in the meetcap project's `repo:` is authoritative) drops speaker-attributed transcripts into a rolling folder — oldest deleted beyond `keep_last`, so an unpulled transcript can disappear. Speakers read `Me` / `Them` by default and `SPEAKER_NN` when clustering was switched on; the transcript's `diarized:` frontmatter says which, so `Them` is never silently assumed to be one person. When the operator references recent meetings ("check my recent transcripts", "from today's call"):
1. List that folder newest-first and confirm which transcript(s) match the meeting in question.
2. File each through the standard ingest workflow above: copy into `sources/meeting-{date}-{slug}.md`, register in `sources/INDEX.md`, set frontmatter `project:` from `unassigned` to this project's slug.
3. Distill decisions/actions into `knowledge/` as usual. Do not edit the MeetCap folder itself; copying is the durability act.

**Person facts route project-first, always.** New facts about a person land in this project's `knowledge/people/{person-slug}.md` overlay — never directly in `workspace/life/people/`. Promotion to `workspace/life/people/{person-slug}.md` is a judgment call made at project closeout ([`project-flow.md`](project-flow.md)).

## Logs Are Born From Evidence

No `knowledge/` log is created at kickoff. A `knowledge/decision-log.md` (dated, append-only: decision, rationale, owner) or `knowledge/raid-log.md` (risks, assumptions, issues, dependencies, one named owner each) is born the first time the ingest workflow has a real decision or risk to record. A one-off project creates neither.
