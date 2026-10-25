# Changelog

Notable changes to AgentFrame. Full release notes also live on the [GitHub releases page](https://github.com/situhacks/agentframe/releases).

## Unreleased

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
- The push is the operator's, in two layers: `.claude/settings.json` denies `git push` as a permission rule, and the pre-tool-use hook ends any Bash or PowerShell command that runs `git push` with exit 2, which holds in bypass-permissions mode.
- Opus chats in this project compact at 200K tokens (`modelSettings` in `.claude/settings.json`); the session-start hook re-injects the life bundle and the due loops after every compaction.
- The main chat: `AGENTS.md` is rewritten as main's instruction and role table, `AGENTS.worker.md` is the overlay for background runs, and nine subagent roles live in `.claude/agents/`. `CLAUDE.md` is gone; Claude Code reads `AGENTS.md` natively.
- The sessions registry and open loops: `af session` (register, list, stop, end, bundle), `af open` (add, close, due), `af brief`, and `af close`, with their shapes in `library/schema/`. The Stop hook asks a registered side chat or worker for its handoff once before it stops.
- `/gym` is a skill in `.claude/skills/gym/`, hidden from the model.

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
