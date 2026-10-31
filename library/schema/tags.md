# Tags

Shapes `workspace/life/tags.yaml`, the controlled vocabulary behind `tags:` on place and person pages ([`place.md`](place.md), [`person-profile.md`](person-profile.md)); `tags-log.md` beside it is the append-only change log and `tags.md` the rendered view. Every file is private. `af tag` owns every write: no agent edits the registry by hand.

```yaml
facets:
  - name: cuisine
    applies: places          # places | people | both
    cardinality: multi       # single: one tag of this facet per page
  - name: price
    applies: places
    cardinality: single
tags:
  - id: noodles
    facet: cuisine
    status: active           # proposed | active | deprecated | merged
    alt: [noodle, noodle-bar]
    scope: "Noodle houses of any tradition; not a ramen-only bar"
    since: 2026-01-10
  - id: noodle-house
    facet: cuisine
    status: merged
    alt: []
    scope: ""
    merged_into: noodles
```

- A tag id is a slug: lower case, digits, single hyphens, forty characters at most, singular, one basic level per facet (`italian`, not `food` and not `cacio-e-pepe`). Alt labels are the synonyms, plurals, and misspellings that `af tag match` folds into the id; a page carries only ids.
- A structured fact is a field, never a tag: `kind`, `area`, `status` on a place; `groups`, `met`, `title` on a person.
- Lifecycle: `proposed` (usable, unconfirmed) becomes `active` at three uses on two pages or on his yes; `deprecated` still resolves and takes no new use; `merged` points at its replacement and `af tag merge` has rewritten every page. Use counts are never stored; `af tag stats` reads the pages.
- The write-time rule, for every agent writing `tags:`: `af tag match "<phrase>" [--facet <f>]` per candidate; write the id it returns; on `none`, `af tag propose <slug> --facet <f> --scope "<one line>"` and write that. `af tag add <page> <tag>...` does both the check and the write on an existing page. Three to eight tags on a venue page, one tag per single-valued facet, none on an area unless it says something the fields do not.
- `af tag merge <from> --into <to>` and `af tag rename <old> <new>` rewrite every page and keep the old form as an alt label, so it is never reinvented; `af tag deprecate` and `af tag drop` retire a tag without a replacement.
- `af search --tag <id> [--tag <id>] [--kind <k>] [--area <a>] [--type <t>]` filters before ranking, with no query listing the pages; an alt label given to `--tag` resolves through the registry.
- Weekly, the `consolidator` runs `af tag lint` ([`consolidator.md`](../../.claude/agents/consolidator.md)): it applies promotions and drops with `--fix`, runs clear plural and alt-label merges, backfills under-tagged pages add-only, and lists semantic merges, splits, and deprecations for his one yes. `af doctor` flags an unknown or merged tag on a page, a deprecated one as a note, two tags in a single-valued facet, a stale `tags.md`, and a `[[slug]]` with no page under `places/` or `people/`.
- `af tag init` writes a registry with the starter facets and no tags on a new instance.
