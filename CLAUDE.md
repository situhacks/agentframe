# AgentFrame — Claude Code entry

This repo's operating persona lives in `AGENTS.md` (swapped atomically between Builder and Operator via `system/audit/writer.py`). Claude Code reads this file natively; the import below pins the active persona so it survives compaction. Cross-cutting invariants live in `AGENTS.md`, not here.

@AGENTS.md
