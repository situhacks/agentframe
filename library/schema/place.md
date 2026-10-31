# Place

Shapes `workspace/life/places/<slug>.md`: one file per venue, shop, service, or area he considers, books, or visits. Main writes it the turn it searches, books, or hears a reaction; the `researcher` writes candidates; the `filer` adds visits from bookings. Every file is `private: true`.

```
---
private: true
kind: restaurant        # restaurant, bar, club, cafe, venue, shop, gym, park, service, area
area: west-end          # slug of a kind: area page
address: 12 Example St
transit: Red line, then the 12 bus
tags: [italian, sharing-plates, group]   # registry ids only; af tag match first
status: want            # candidate, want, been, regular, avoid, closed
checked: 2026-10-27     # hours and prices last confirmed
---
# Trattoria Example
Restaurant · Italian sharing · [[west-end]] · 12 Example St · 12 bus · want

## What it is
## Practical
## Vibe and music
## Reviews found
## His take
## Visits
- 2026-11-02 · dinner for 4 · with: [[a-friend]], [[a-group]] · verdict: pending
```

- The slug is the file stem, kebab-case from the name; one file per venue. The same address under a new name is the same file, with "formerly X" in What it is. Run `af search "<name>"` before creating one.
- The card line, the first body line under the title, repeats kind, tags, area, address, transit, and status in prose; it is what a text query matches, so keep it in step with the frontmatter. `kind`, `area`, `status`, and `tags` are also exact fields: `af search --tag <id> [--kind <k>] [--area <a>]` filters on them before ranking, and lists the pages with no query.
- `tags` carries only ids from the registry ([`tags.md`](tags.md)): before writing one, `af tag match "<phrase>" --facet <f>` and write what it returns; on `none`, `af tag propose <slug> --facet <f> --scope "<one line>"` and write that. Three to eight tags on a venue, one per single-valued facet (price), none that repeat a field. `af tag add <page> <tag>...` does the check and the write on an existing page.
- Durable facts carry no dates. Perishable facts (hours, prices, menu) sit in Practical and are covered by `checked`; re-check before recommending a place not checked in six months. A venue that closes becomes `status: closed` with one line in What it is; the file stays.
- His take holds his words, near-verbatim, each with the date he said them. The agent's description and reasoning are the other sections.
- A visit is one row under Visits, planned or done: `- YYYY-MM-DD · <what> · with: <people and groups> · verdict: <pending | his words>`. An event (a show, a party) is a visit row on its venue, not its own file; a recurring night is a line in Vibe and music.
- Every planned visit raises one loop for the day after: `af open add "verdict: <place>, <date> <what>" --nudge <date+1> --from <source id or chat>`. When he answers, the row's verdict becomes `yes | no | maybe` (go back?), his words go under His take, `status` moves, the loop is closed with `af open close --outcome "<verdict>"`, and main tells him how the place matched what he asked for and records what it learned in the city taste page, `workspace/life/knowledge/<city>.md`.
- Links are `[[slug]]` to a place, area, person, or group page and are written on both ends: the area page lists the place under `## Places`. A visit is written once, on its place page; person and group pages do not copy it. Slugs are unique across `places/` and `people/`; group slugs start with `group-`. Nothing follows backlinks; `af search` and grep are the retrieval path.
- A `kind: area` page carries where it is, how he gets there from home, character, noise, what it is known for, and `## Places` with one line per place. `address` and `tags` may be empty on an area.
- `private: true` on every file: nothing from a place page goes into a tracked file.
