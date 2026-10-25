---
name: file
description: The one ingest entry. Files a path, the inbox, Gmail threads, or the operator's last message into sources, distills each into the owning knowledge page through the filer role, and raises loops.
disable-model-invocation: true
---

Filing mode. Every input becomes a source record and its durable facts land in knowledge pages. The shapes are [`source-record.md`](../../../library/schema/source-record.md) and [`knowledge-page.md`](../../../library/schema/knowledge-page.md); read both before the first dispatch.

The layer is `workspace/life/` unless `$ARGUMENTS` ends in `for <slug>` or this chat is registered as `project/<slug>`; then it is `workspace/projects/<slug>/`, and every filer brief names that project.

## Collect the inputs from `$ARGUMENTS`

- **A path**: that one file. Confirm it exists; a folder means every file directly in it.
- **`inbox`**: every file in `workspace/life/sources/inbox/`. If the folder is empty, say so and stop.
- **`gmail <search>`**: run `mcp__claude_ai_Gmail__search_threads` with the search, show the matching threads (sender, subject, date) as a lettered list, and file the ones he picks, or all when the search was specific. For each, fetch it with `mcp__claude_ai_Gmail__get_thread` and write the thread verbatim (headers, then every message in order) to `workspace/life/sources/inbox/email-<date>-<slug>.md`, `origin` the thread id. Never send, draft, label, archive, or delete in Gmail.
- **`order`**: his last message before `/file`, verbatim, written to `workspace/life/sources/inbox/order-<today>-<slug>.md`, `origin` the chat name.

Everything this skill writes goes to the inbox first, so every input reaches the filer the same way.

## Dispatch

For each input, one at a time, dispatch the `filer` role. Its brief names:

- the input path and the layer;
- the two schemas by path;
- the move rule: an input in `sources/inbox/` is moved into `sources/` (an order into `sources/orders/`) under its schema name, never copied; an input outside the inbox is copied and the original left alone;
- the project, when the layer is a project.

Wait for each return before the next dispatch, since two filers can pick the same next id.

## Close

1. If the run took inputs from the inbox, list `workspace/life/sources/inbox/`. It must be empty; name any file still there and why.
2. Show each filer return as it came back: the source id, the pages touched, the loops raised.
3. End with one line totalling the sources filed and the loops raised.
