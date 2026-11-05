# Changelog

Notable changes to AgentFrame. Full release notes also live on the [GitHub releases page](https://github.com/situhacks/agentframe/releases).

## Unreleased

- One chat. No rule reserves a tool or work for one chat; each does its job and routes the rest to main. `af session reach` says what a message would meet and checks it arrived, because `SendMessage` answers success for a message that is then held. Workers start with `claude -p`, and their exit wakes the chat that started them.
- `af place`, `af person`, and `af backlog ready` are new buttons. `af ready` lifts a head's `## Ship` region into `post-FINAL.md`, a deferred ingredient counts as finished, a post row can override the manifest, and a published record stays published. `af draft --file` registers a head written first.
- A weekly backlog pass (`system/tools/backlog-pass.ps1`) raises a loop naming the ready rows for main to dispatch to a reviewed builder.
- Claude Code only. The Cursor and Codex wiring, the harness manifest, and `af sync-harnesses` are gone.
- Removed the managed-run daemon and `af automation`, lenses, and the Composio notes. Performance capture lives in `library/process/performance-capture.md`.
- Removed the board and the orchestrator router, the sealed autonomy contracts, the Workspace Dashboard, the browser harness, open-design, the Gemini research runtime, and upstream-sync.
- One project shape. `project-flow.md` replaces the three flows and the `flow` field, and a project can point at a code repository with `repo:`.
- Removed retros, the feedback log and `af feedback`, technical-build, the project-mgmt governance templates, the consolidate skill, the audit database, and every `evolution.md`. The harvests at ready and publish carry the learning, button runs log to `system/logs/af-runs.jsonl`, and friction goes to `system/backlog.md`.
- Removed the humanizer skill and its process file. For operator-voiced prose, the voice clean pass and `af voice lint` own the cleanup.
- One authoring standard, `library/process/authoring.md`, replaces the structure skill, the scaffolding skill, and the process and template authoring files. The template sections now live in `library/deliverables/_meta/deliverable-shape.md`.
- `system/skills/` is gone. AgentFrame-owned skills live in `.claude/skills/`, where six of them are now operator-invoked; vendored runtimes live in `system/vendor/`. Each folder has its own catalog.
- The public schemas moved to `library/schema/`, so `library/context/` is wholly private.
- The routers are shorter, and the builder router gains the enforcement ladder and a rule-to-enforcer table. The README is rewritten for what ships today.
- The buttons are one Rust binary, `af`, built from `system/af/` and installed with `cargo install --path system/af`. Every verb keeps its contract; `system/af.py`, the indexer, the voice bundle and lint scripts, the pack `rules.py` modules, and their Python tests are gone. Python stays for the media and Office tools.
- Every hook is `af hook <event>`: session start, stop, session end, notification, and the tool guards, wired once per matcher in `.claude/settings.json`. The Python hook scripts are gone.
- A push needs the operator's explicit yes: a `permissions.ask` rule prompts for `git push` in every mode, bypass included, and the pre-tool-use hook turns a push buried in a compound Bash or PowerShell command into the same prompt. The harness may push once he approves.
- The project sets no compaction window; the operator sets each chat's with `/autocompact`. The session-start hook re-injects the life bundle and the due loops after every compaction.
- The main chat: `AGENTS.md` is rewritten as main's instruction and role table, `AGENTS.worker.md` is the overlay for background runs, and nine subagent roles live in `.claude/agents/`. `CLAUDE.md` is gone; Claude Code reads `AGENTS.md` natively.
- The sessions registry and open loops: `af session` (register, list, stop, end, bundle), `af open` (add, close, due), `af brief`, and `af close`, with their shapes in `library/schema/`. The Stop hook asks a registered side chat or worker for its handoff once before it stops.
- `/gym` is a skill in `.claude/skills/gym/`, hidden from the model.
- An unregistered chat is told at session start to ask, in its first reply, whether it is main (offered only when no main is live), a side chat, or a one-off; a task given instead of a letter makes it a one-off. `af hook` takes exactly one event.
- A background worker is registered before launch with a fixed `--session-id`, so its first session start prints its brief.
- `/catchup`, `/file`, and `/upgrade`: catching up after time away, the one ingest entry (a path, the inbox, Gmail threads, or the last message), and the self-repair loop that hands backlog rows to a builder worktree and merges on the operator's yes.
- Schemas for source records, knowledge pages, and one backlog row shape, with done rows moving to `system/backlog-completed.md`. MIGRATION: rewrite old-format rows in `system/backlog.md` into the new shape.
- `af open redate <loop> --nudge <date or +Nd>` changes a loop's nudge date in place; `/catchup` uses it for the re-date answer.
- Recurring loops and a daily phone push. `af open add --every <monthly|weekly|Nd>` writes `· every <period>` after the nudge; `af open close` logs the close and re-dates the loop one period on, and `--final` ends it; `af open due --json` carries `every`. `system/tools/notify.py` is the shared Discord notifier, `push-due-loops.ps1` posts the due loops, and `register-daily-reminders.ps1` schedules it daily at 08:30. The loop rule lives once in `library/schema/open-loops.md`, and every router points at it. MIGRATION: groceries `notify.py` becomes a wrapper over `system/tools/notify.py`; a loop line whose text carries ` · nudge ` or ` · every ` no longer parses.
- A weekly headless consolidation of the life layer, run by Task Scheduler on Sundays at 18:00 as a registered worker session, with a Monday review loop.
- The `deep-research` writer leads with the bottom line, uses headers that state each section's finding, interprets the data and takes positions, and ends on new insight only. Every specialist triages its sources for speculation, aggregators, and boilerplate authority, and may add labeled `inference:` lines kept apart from its atoms. Adapted from Anthropic's deep-research skill, with no new waves, roles, or budget.
- A daily bills watch. `system/tools/bills-watch.ps1` (scheduled at 07:45 by `register-bills-watch.ps1`) reads card emails through the claude.ai Gmail connector in headless don't-ask `claude -p` runs locked to `search_threads` and `get_thread` (one per thread, saved results deleted after parsing), parses the raw results in `bills_watch.py`, keeps the statement ledger `workspace/life/systems/finances/bills.md`, and raises, re-dates and closes one `Bills` loop per unpaid statement: the day the figures land, 5 days before due, 1 day before, and on a missed-payment email; payment evidence closes them, and any failure raises its own loop. Shape and timing in `library/schema/bills.md`.

## v0.1.0 — a context workspace for any kind of work

First public cut of the rebuilt, domain-neutral AgentFrame. It's what I use day to day, it's still moving, and it's free to fork. Works with Claude Code, Codex, Cursor, VS Code, Antigravity, or anything else that reads files.

- **Context that survives the chat.** A fresh agent reconstructs a project from disk after a compaction, provider switch, or a month away; `sources/` and `knowledge/` stay separate, with a consolidation pass to archive resolved detail; the persona → project → flow → process chain loads only what the step needs.
- **A domain-neutral core with packs on top.** The project is the smallest unit of work, generic enough for a frontier model to scaffold for almost anything. Marketing, project management, and careers ship today.
- **A voice system trained on your real edits.** It learns from the gap between its draft and your rewrite, using reusable registers selected independently from channel and folding manual edits back in as contrastive pairs.
- **Deterministic state and a real audit trail.** `af` owns creation, versioning, locking, publishing, and doctor checks; every deliverable keeps a version trail with immutable snapshots and tracked exports; an append-only audit database records system changes.
- **Managed automations for standing work.** A project defines a job, receives queued tasks, and runs them through a bounded local agent that reports back `done`, `blocked`, or `failed`. Early — just started running on real work.
- **A Workspace Dashboard rendered from the files.** Projects, an attention list, calendar and timeline views, a preview workspace, and a read-only Automations pulse, all read straight from the same files the agents use — no second database, no model call.
- **Learn from finished work.** Closeout harvests compare drafts with manual edits and logged friction, then propose template, process, and voice-corpus changes behind a short human review; bounded runs name their goal, evidence, budget, and review points before starting.

[v0.1.0 release notes](https://github.com/situhacks/agentframe/releases/tag/v0.1.0)
