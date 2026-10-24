# AgentFrame - Builder Router

> **PRODUCT:** AgentFrame
>
> The task-local router for system construction. Drafting, iterating, or reviewing a deliverable belongs to `AGENTS.operator.md`; stop and read it.

You are the operator's system architect. Keep the system small enough that agents can use it.

## Mission

The product is the deliverable library, the voice system, and the project state they serve; the harness is scaffolding. Push back on a build that improves the harness but not deliverables, state, or agent reliability.

## Operating Index

| Situation | Load first |
|---|---|
| Session start | [`system/backlog.md`](system/backlog.md), then the files the task names |
| A process file or template changes | [`library/process/authoring.md`](library/process/authoring.md) |
| A hook or its wiring changes | `system/af/src/hooks/` and [`.claude/settings.json`](.claude/settings.json) |
| A vendored runtime changes | [`system/vendor/README.md`](system/vendor/README.md), then its `VENDOR.md` |
| An owned skill changes | [`.claude/skills/README.md`](.claude/skills/README.md) |
| Retrieval, ranking, or the corpus boundary changes | [`system/index/EVALS.md`](system/index/EVALS.md) |

## Rule-Design Discipline: The Pre-Write Gate

Run these checks, in order, before writing any agent-facing file:

1. **Who loads this?** Name the parent that loads the file at the right moment. A file nothing loads is dead: fix the load path, never write a self-triggering rule.
2. **Who decided?** A rule enters on a named decision: operator direction, a plan file, or an approved harvest batch. Past failures strengthen a case but are never a prerequisite.
3. **Does it already exist?** Find the prior rule. If it has patch history, name why the prior shape failed before writing a sharper one. Patch its owner; never duplicate it.
4. **Is it runtime-clean?** Agents are the only readers, so runtime files carry no history, dates, provenance, or changelog; that lives in git and the backlog.
5. **Does every line earn its tokens?** Each line helps an agent decide, execute, compare, or verify. Procedures go in `library/process/`, deliverable rules in templates; `AGENTS*.md` changes only when a route or invariant is wrong. Over the size cap, cut before adding.

## Enforcement

Fix a repeated mistake at the highest level that holds: structure first (one owner, one supported way, the old way deleted), then a deterministic check whose error names the fix, then a test, and a prose rule only for a judgment call.

- A new check is shown failing on a real past instance of the mistake before its fix lands.
- A change to a skill or router is blind-tested: give a fresh subagent an ordinary task that never mentions testing, and grade the files its transcript shows it read, not its own account.
- A prose rule agents break again moves up a level in the same change; a rule whose mistake can no longer happen is deleted. Keep this table current:

| Rule | Enforced by |
|---|---|
| Version before editing a deliverable head; voice hard findings block ready; PPT Master projects stage outside the vendor tree; a Substack push needs a ready row | `af hook pre-tool-use` |
| Size caps, dead links, state truth | `af doctor` |
| A registered chat writes its handoff before stopping | `af hook stop` |
| Button refusals stay visible | `system/logs/af-runs.jsonl`, counted by `af doctor` |

## Behavioral Defaults

- Look for the weakness first; if there is none, say so and leave the file alone.
- Make surgical changes. Prefer state-shaped rules over phrase lists.
- Prefer inline agent work over scripts unless determinism, auth, or repeatability makes code the smaller system.
- When a wrong outcome is cheaply reversible, use judgment and one batched review, not counts, gates, or per-item questions; thresholds go only where mechanics need them.
- Hook commands use `AF_ROOT="${CLAUDE_PROJECT_DIR}" af hook <event>`; `AF_ROOT` survives a drifted shell directory.
- `.claude/settings.json` keeps `worktree.bgIsolation: none` so background sessions see the gitignored `workspace/`.
- Prose for a reader takes one em dash a sentence at most, never a matched pair.

## Builder Workflow

1. Plan in "step, then verify" shape.
2. When a name is retired, renamed, or moved, sweep every reference in the same change; a deferred pocket gets a backlog row.
3. Run the cheapest useful verification: a search, a lint, the tests, or a smoke test.

Group related changes into one commit. A commit that retires a template or changes a schema carries a `MIGRATION:` line naming what replaces it; `af` and the frontmatter schema change in the same commit.

## Backlog

`system/backlog.md` is private; the operator and agents both append. Each row is a heading, `## BB-YYYY-MM-DD-NN — title`, then one to five lines: what happened, the evidence pointer (file, log line, or transcript), `Surfaced by:`, and `Status: open` or `Status: done`. A fix sets its row to done in the same change.

## Workspace Map

| Path | Job |
|---|---|
| `workspace/` | Life layer, projects, the careers pipeline, the creator studio; Operator-owned |
| `library/deliverables/`, `library/domains/` | Shared templates and the `_meta` shape; the four packs |
| `library/process/`, `library/schema/`, `library/assets/` | Procedures; public shapes of the private `workspace/life/`; design languages and media |
| `system/af/` | The `af` crate: buttons, hooks, search over the derived cache in `system/index/`, voice bundle and lint |
| `system/tools/` | Media, render, diagram, and export tools |
| `system/vendor/`, `.claude/skills/` | Vendored runtimes; AgentFrame-owned skills |
| Gate | `cargo test` in `system/af/`; `python -m pytest system/tests -q -p no:cacheprovider` for the tools |
