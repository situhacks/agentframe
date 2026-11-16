# Learnings

Mistakes that came back or cost a real failure, and the guard that now stops each. Shape: [`library/schema/learnings.md`](../library/schema/learnings.md). A builder reads the entries whose `Watch:` paths its change touches; it adds or updates one after fixing a mistake that came back, Guard first. Tracked and public: system facts only.

## L-2026-11-13-01 · Parallel tool calls fired the context nudge five to eight times
Seen: a build chat, 2026-11-13; the review of the compaction-nudge build
Root cause: each hook process decided to speak from the transcript alone, so every simultaneous process saw the same unsent state and spoke.
Guard: `system/af/src/context.rs` claims one marker file per nudge tier, and only the process that creates it speaks; test `system/af/tests/session_classes.rs::eight_processes_at_once_say_the_soft_nudge_exactly_once`
Watch: `system/af/src/context.rs`, `system/af/src/hooks.rs`, any hook that says something once

## L-2026-11-13-02 · A nudge named the size from before a compaction
Seen: a build chat, 2026-11-13, on its first tool call after compacting: "483k tokens" at a real 79k
Root cause: the context size was read from the last reply in the transcript, which still carried the pre-compaction usage until the next reply.
Guard: `system/af/src/reach.rs` stops its usage scan at a real compaction boundary, so the size is unknown until the next reply and no nudge fires; test `system/af/tests/session_classes.rs::a_real_shaped_compaction_re_arms_both_tiers`
Watch: `system/af/src/reach.rs`, `system/af/src/context.rs`, `system/af/src/compact.rs`

## L-2026-11-13-03 · A model pick in the panel rewrote the operator's default model
Seen: panel trials, 2026-11-13: the global default changed to another model from 09:52 to 12:00
Root cause: the panel's model picker writes the global `model` key as a side effect, and nothing checked that setting after a pick.
Guard: `system/tools/claude_panel.ps1` saves the settings file's hash and the one model line before a pick, writes the line back right after, verifies, and exits 6 naming the hand fix if it does not match; test `system/tests/test_claude_panel.py`
Watch: `system/tools/claude_panel.ps1`, any tool that drives a Claude Code or VS Code control that also writes settings

## L-2026-11-15-01 · A source file was written by hand and took an id another chat held
Seen: a main chat, 2026-11-15; order src-2026-11-15-08
Root cause: the id was read from the folder and then written without a lock, so two chats picked the same number; `af source add` took the lock but nothing stopped the hand route.
Guard: `system/af/src/hooks/source_guard.rs` denies a `Write` that would create a new file under workspace/life/sources/ and names `af source add`; edits to existing files and to the index stay allowed; test `system/af/tests/hooks_source_guard.rs`
Watch: `workspace/life/sources/`, `system/af/src/source.rs`, any verb that hands out the next id

## L-2026-11-15-02 · A clock time was written into a report from memory
Seen: build and main chat reports, 2026-11-15; order src-2026-11-15-08, item 5
Root cause: a model has no clock; asked "when", it writes a plausible time as a fact.
Guard: prose: `.claude/agents/builder.md` (the report's times come from `date`, run when the line is written)
Watch: `.claude/agents/`, any report, handoff or loop line that carries a time of day

## L-2026-11-15-03 · A Gemini call ran bare, outside the fence
Seen: a side chat, 2026-11-15, web research through a bare `agy -p --mode accept-edits` in the repo, able to write files
Root cause: print mode does not gate file writes, and the fence that copies inputs into a disposable read-only folder lives only in the wrapper, so the bare command had no fence.
Guard: `system/tools/agy_call.py` is the one way in (the fence, `--web` through `search_web` only, no skip flag by default); its rule is in `library/process/media-intake.md`; test `system/tests/test_tools_media.py`
Watch: `system/tools/agy_call.py`, `.claude/agents/researcher.md`, `.claude/agents/reviewer.md`, any call to `agy`
