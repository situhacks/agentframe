# Source record

Shapes a filed source in `workspace/life/sources/` or a project's `sources/`, and its row in that layer's `sources/INDEX.md`. The `filer` role writes both; `/file` dispatches it.

```
---
id: src-2026-10-24-01
type: order
date: 2026-10-24
origin: dictated in main
verbatim: true
---
<the input, unchanged>
```

- File name: `<type>-<date>-<slug>.md`, type one of `order`, `receipt`, `email`, `transcript`, `document`, `note`; an order is `order-<date>-NN-<slug>.md`, NN its id's. `date` is the day the source was made or received, the same date in the name and the id.
- `id` is `src-<date>-NN`, NN the next number unused anywhere under the layer's `sources/`. `af source add` assigns it under a lock and writes the file and the INDEX row; never write an id by hand. `af doctor` reports a duplicate id or an order name that disagrees with its id. `origin` says where it came from in a few words: a Gmail thread id, the original path, the chat.
- The body is the input byte for byte: no summary, no cleanup, no added headings. A correction goes in the knowledge page, never in the source.
- A binary input (PDF, image) keeps its extension under the same name and carries no frontmatter; its INDEX row holds the metadata.
- `INDEX.md` is one table, a row per source: ``| `src-2026-10-24-01` | [`orders/order-2026-10-24-01-groceries.md`](orders/order-2026-10-24-01-groceries.md) | 2026-10-24 | <one line> |`` under `| ID | File | Date | Description |`, the file relative to `sources/`; `af source add` writes it.
- An order (the operator's instruction, in his words) lives in `sources/orders/`; every other type sits at `sources/` top level.
- `sources/inbox/` is a staging folder: drops, fetched email, and dictations wait there, and the filer moves each one out. It is empty after every `/file` run.
- A filed source's body is never edited or deleted. The one frontmatter field that may change is `tags` (one to three `topic` ids as `tags: [a, b]`, set when filing with `af source add --tag` or later with `af tag add`, never by hand; [`tags.md`](tags.md)). `INDEX.md` is the only other file in `sources/` that changes.
