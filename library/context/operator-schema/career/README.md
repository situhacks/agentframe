# Career bank — schema

Shapes for the operator's career capital under `library/context/operator/career/` (personal, gitignored — these schema files are the tracked mirrors). The careers pack (`library/domains/careers/pack.md`) consumes the bank; [`career-harvest`](../../../process/career-harvest.md) feeds it.

| Surface | Shape | Holds |
|---|---|---|
| `profile.md` | [profile.md](profile.md) | contact header (seeds every resume, exact strings) + comp targets + constraints |
| `master-cv.md` | [master-cv.md](master-cv.md) | presentation layer: resume-ready CDO bullets per role, citing proof-points |
| `proof-points.md` | [proof-points.md](proof-points.md) | numbers ledger: verified metrics/links with source + date |
| `stories/` | [stories/README.md](stories/README.md) | narrative layer: one STAR+R arc per file, ROSTER first line |
| `tracks/` | [tracks/README.md](tracks/README.md) | per-track framing and tone (e.g. startup vs enterprise) |
| `search-profile.md` | [search-profile.md](search-profile.md) | active-search config: targets, dealbreakers, the scout watchlist |
| `employers/` | [employers.md](employers.md) | per-employer relationship page: leveling rubric, KPIs, cycle calendar, expectations (compiled truth + timeline) |

Layer rule — one fact, one home: a number lives once in proof-points; bullets and stories cite its ID. First-time setup: copy each shape into `library/context/operator/career/` and fill, or run career-harvest against existing material.
