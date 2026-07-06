# Annotated contrastive pairs

The steering layer. Each pair shows the distance from generic-AI prose to YOUR voice, with a note on the move. Generate by writing toward the rewrites and applying the moves. The corpus (`../corpus/`), not the pairs, carries the imitation load; pairs steer.

Group files by register (e.g. `builder-pov.md`, `formal.md`, `short-form.md`). Load the ones matching the task; don't load all. A cross-cutting file may be marked **always-load for long-form prose** when it guards a drift no single register owns (e.g. a `plain-not-clever.md` catching fluent over-writing — smooth, ornamental sentences that pass every cadence test).

Format per pair:

```
### tag — one-line label
BASE: <generic, highest-probability version>
[YOU]: <same payload, in your voice>
MOVE: <what changed and why — generatively useful, one or two sentences; cite the source delta>
register: <which register>
```

Cap by moves, not count: **one pair per distinct move** — a new example of an already-covered move REPLACES the old pair (newest approved work wins), it never joins it. Total count is a curation signal, not a wall; a single register file creeping past ~15 pairs is due a dedup pass, because a draft loads that whole file and in-context exemplars compete — redundant pairs dilute the load-bearing ones. A pair earns its place only if it teaches a GENERALIZABLE move — would this note help write a different piece better? If it's a one-off, skip it.

See `example.md` for the format in action.
