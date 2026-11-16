# Cache Warm

## Purpose

Owns keeping a chat's prompt cache alive through a long wait with `af session warm`, and the choice between warming a chat and letting it lapse or compacting it.

## When To Load

A chat is about to wait over an hour on work it will return to (an agent running past an hour, a queued job, "I'm going to bed, keep going") and its context is over 150k tokens.

## Procedure

Why it pays: Claude Code writes this project's chat caches with a 1-hour lifetime, and any request that reads the cache resets that hour. A 1-hour cache write costs twice the model's input price; a cache read has its own price, far below the generic 0.1x on the current models (the cache-read prices in [`models.md`](../schema/models.md#the-models)). So rebuilding a lapsed cache costs about 40 times one read on Opus and Sonnet and 80 times on Fable (20 times at a 0.1x read price), and a night of hourly wakes costs less than one cold restart. A measured wake read 165,832 tokens from cache and wrote 1,861 (the notification and the reply).

The only trigger that starts a turn in an idle chat is a background command the chat itself started: when it exits, Claude Code wakes the chat with the exact prompt prefix. A hook runs only on events, and a headless `claude -p --resume` sends a different system prompt, so it misses this chat's cache. `af session warm` is that background command.

1. Arm it with the Bash tool's `run_in_background`, once, before the wait: `af session warm <name>`. Name is the chat's registry name. It waits until the chat has been idle for 55 minutes (any activity resets the count), prints three lines and exits, which wakes the chat.
2. On the wake, read the lines. Still waiting on work you will return to: run the re-arm command it printed, in the background, and end the turn. Task done: run the compact command it printed instead.
3. The timer counts only main-chat replies, so during a long foreground tool call or a foreground subagent it can fire mid-turn; that wake is queued until the turn ends and costs one small turn.
4. A line starting `stop:` means nothing was armed or the timer ended; its text says why and what to do. Do not re-arm after a stop unless the text says to.

Options carry into the re-arm line: `--minutes` (5 to 56, default 55), `--max-wakes` (default 12), `--min-tokens` (default 150000).

Do not arm when:

- The task in hand is done: compact instead (`af session compact <name> --when-idle`).
- The chat is on the 5-minute cache (the verb stops itself): a wake every 4 minutes costs more than it saves.
- The context is small: a cold restart rewrites little, and the verb stops itself.
- The wait is one he did not ask the chat to come back from: a chat that is simply finished lets its cache lapse.

Switching a chat's model discards its cache anyway, so a warm timer is pointless across a model switch; start a new one afterwards.

## Verification

After the wake, the chat's transcript usage shows cache read near the full context and cache creation small. `system/logs/warm/warm.log` holds one line per run: time, name, wake, outcome (`fired` or the stop reason), idle minutes, context tokens.

## Boundaries

Does not wake a chat that did not arm itself, change settings, or estimate costs (prices live in [`models.md`](../schema/models.md)). Compaction itself belongs to `af session compact` and the context nudges ([`sessions-registry.md`](../schema/sessions-registry.md)).
