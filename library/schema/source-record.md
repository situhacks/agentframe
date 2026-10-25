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

- File name: `<type>-<date>-<slug>.md`, type one of `order`, `receipt`, `email`, `transcript`, `document`, `note`. `date` is the day the source was made or received, the same date in the name and the id.
- `id` is `src-<date>-NN`, NN the next free number for that date in `INDEX.md`. `origin` says where it came from in a few words: a Gmail thread id, the original path, the chat.
- The body is the input byte for byte: no summary, no cleanup, no added headings. A correction goes in the knowledge page, never in the source.
- A binary input (PDF, image) keeps its extension under the same name and carries no frontmatter; its INDEX row holds the metadata.
- `INDEX.md` is one table, a row per source: `| src-2026-10-24-01 | orders/order-2026-10-24-groceries.md | 2026-10-24 | <one line> |` under `| ID | File | Date | Description |`, the file relative to `sources/`.
- An order (the operator's instruction, in his words) lives in `sources/orders/`; every other type sits at `sources/` top level.
- `sources/inbox/` is a staging folder: drops, fetched email, and dictations wait there, and the filer moves each one out. It is empty after every `/file` run.
- A filed source is never edited or deleted; `INDEX.md` is the only file in `sources/` that changes.
