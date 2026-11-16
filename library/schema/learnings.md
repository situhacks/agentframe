# Learnings

Shapes `system/learnings.md` (tracked, public: system facts only, no private fact, client name or text from `workspace/`) and any other log in the same shape. It is an index of mistakes that came back or cost a real failure, and the guards that now stop them. Nothing loads it at a chat's start; a builder reads the entries whose `Watch:` paths its change touches (`af search` finds them by path), and the [harness review](../process/README.md)'s "Came back" section points at the entries still owed.

```
## L-2026-11-15-01 · A source file was written by hand and took a held id
Seen: a main chat, 2026-11-15; order src-2026-11-15-08
Root cause: the id was read from the folder, then written, so two chats could pick the same one.
Guard: `system/af/src/hooks/source_guard.rs` denies a new file under workspace/life/sources/; test `system/af/tests/hooks_source_guard.rs`
Watch: `workspace/life/sources/`, `system/af/src/source.rs`
```

- One entry is the heading `## L-YYYY-MM-DD-NN · <the mistake, in a phrase>` and the four lines `Seen:`, `Root cause:`, `Guard:`, `Watch:`, in that order. `YYYY-MM-DD` is the day the mistake was seen; NN is the next free number for that day.
- `Seen:` is where and when, with the backlog row ids and order ids that carry the evidence.
- `Root cause:` is one or two sentences on why it happened, not what broke.
- `Guard:` is the structure, check or test that now stops it, each path vault-relative in backticks (a private path stays out of backticks: the check looks for the file), a test name after the path. When only a rule could stop it, write `prose:` and the backticked file that holds the rule. An entry with no guard path is not finished.
- `Watch:` lists the paths or verbs where the same class of mistake could appear, in backticks.
- Fix a repeated mistake at the highest level that holds ([`AGENTS.builder.md`](../../AGENTS.builder.md#enforcement)), then write the entry, Guard first. Update the entry when the mistake comes back; a mistake that can no longer happen is deleted, with its guard.
- The harness review lists an entry whose `Guard:` path no longer exists. A test reads the real file and fails on a guard path that is missing.
- `workspace/learnings.md` is the private log for project and life-layer mistakes, in the same shape; `af new-project` creates it for a pack with `learnings: true`, and the harness review checks its guard paths too.
