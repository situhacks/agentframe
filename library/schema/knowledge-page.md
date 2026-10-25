# Knowledge page

Shapes `workspace/life/knowledge/<slug>.md` and a project's compiled entity pages under `knowledge/` ([`knowledge-base.md`](../process/knowledge-base.md)). The `filer` writes facts in; the `consolidator` recompiles.

```
# <Topic>

## Current

- The lease renews on 2027-03-01 at the same rent (as of 2026-10-24) [src-2026-10-24-01]

## Timeline

- 2026-09-02 · renewal notice received [src-2026-09-02-01]
- 2026-10-24 · landlord confirmed renewal terms by email [src-2026-10-24-01]
```

- `## Current` is what is true now. Each fact appears once, with `(as of YYYY-MM-DD)` and the source id that carries it.
- A superseded fact is replaced in Current, never struck through and never contradicted by a line below it. The old fact survives only in the timeline.
- `## Timeline` is dated and append-only, oldest first, new entries at the bottom: `- YYYY-MM-DD · <what happened> [<source id>]`. Every entry cites a source id or a chat name.
- Current is rebuilt from the timeline and what it points at, never from the previous Current. A Current older than the newest timeline entry is stale.
- Material too long for the page (a full analysis, a long table, a working draft) goes in `knowledge/_history/<slug>/<YYYY-MM-DD>-<name>.md`; the timeline entry links it and Current carries the conclusion.
- Timeline entries older than six months move to `knowledge/_archive/<slug>-<YYYY-MM>.md` (the month of the consolidation pass), leaving one line in their place: `- archived: entries before YYYY-MM-DD → _archive/<slug>-<YYYY-MM>.md`. A Current fact whose evidence moved stays, citing its source id.
- A page with `private: true` frontmatter stays private: nothing from it goes into a tracked file.
