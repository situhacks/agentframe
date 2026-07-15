# Changelog

Notable changes to AgentFrame. Full release notes also live on the [GitHub releases page](https://github.com/situhacks/agentframe/releases).

## v0.1.0 — a context workspace for any kind of work

First public cut of the rebuilt, domain-neutral AgentFrame. It's what I use day to day, it's still moving, and it's free to fork. Works with Claude Code, Codex, Cursor, VS Code, Antigravity, or anything else that reads files.

- **Context that survives the chat.** A fresh agent reconstructs a project from disk after a compaction, provider switch, or a month away; `sources/` and `knowledge/` stay separate, with a consolidation pass to archive resolved detail; the persona → project → flow → process chain loads only what the step needs.
- **A domain-neutral core with packs on top.** The project is the smallest unit of work, generic enough for a frontier model to scaffold for almost anything. Marketing, project management, and careers ship today.
- **A voice system trained on your real edits.** It learns from the gap between its draft and your rewrite, per channel register, folding manual edits back in as contrastive pairs.
- **Deterministic state and a real audit trail.** `af` owns creation, versioning, locking, publishing, and doctor checks; every deliverable keeps a version trail with immutable snapshots and tracked exports; an append-only audit database records system changes.
- **Managed automations for standing work.** A project defines a job, receives queued tasks, and runs them through a bounded local agent that reports back `done`, `blocked`, or `failed`. Early — just started running on real work.
- **A Workspace Dashboard rendered from the files.** Projects, an attention list, calendar and timeline views, a preview workspace, and a read-only Automations pulse, all read straight from the same files the agents use — no second database, no model call.
- **Learn from finished work.** Closeout harvests compare drafts with manual edits and logged friction, then propose template, process, and voice-corpus changes behind a short human review; bounded runs name their goal, evidence, budget, and review points before starting.

[v0.1.0 release notes](https://github.com/situhacks/agentframe/releases/tag/v0.1.0)
