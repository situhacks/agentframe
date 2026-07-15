# AgentFrame — Claude Code entry

Claude Code reads `AGENTS.md` natively. This file intentionally adds no second instruction layer: the import preserves the stable task classifier across compaction, and that classifier lazy-loads the one task router the work needs.

@AGENTS.md
