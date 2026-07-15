# Annotated contrastive pairs

The steering layer. Each pair shows the distance from generic prose to the operator's voice. The corpus carries the imitation load; pairs teach specific moves.

Register and task context are separate axes:

- `register`: `formal`, `informal`, or `both`;
- `context`: the posture or output situation, such as `builder-pov`, `market-signal`, `slide`, `cover`, `email`, or `long-form`.

Files may be grouped by register (`informal.md`) or context (`slide-and-cover.md`). Load only files matching the selected recipe and task context, plus any cross-cutting file marked always-load.

Format:

```text
### tag — one-line label
BASE: <generic version>
YOU: <same payload, in your voice>
MOVE: <what changed and why; cite the source delta>
register: formal | informal | both
context: <task context>
```

Use `both` only when the move genuinely survives both registers. Never put a platform name in `register`; platform-specific behavior belongs to a channel profile.

Cap by moves, not count: one pair per distinct, generalizable move. Replace redundant pairs with newer approved evidence. A file creeping past roughly 15 pairs is due a dedup pass because competing exemplars dilute the useful ones.
