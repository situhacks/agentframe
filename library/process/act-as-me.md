# Act As Me

## Purpose

Owns how a chat decides his open decisions in his name when he has handed them over. His rule: "it's not supposed to be a hard rule. It's based off of what I tell and dictate to you."

## When To Load

- Session start prints `Acting as him until <time>: "<scope>" [<id>]`.
- He says to act as him, take care of his needs list, or do whatever while he is away.

## Procedure

**Start.** Only his words start it. Main files them verbatim as an order first ([`source-record.md`](../schema/source-record.md)), then runs `af act start --from <order id> --scope "<his words>" [--until <time>]`. The scope is what he said, quoted. Nothing else starts it: not idle time, not the night mute.

**Scope.** The scope covers only what he said. "My needs list" covers the decisions on [the needs list](../schema/needs-list.md). "Do whatever" covers what any chat may already do without asking, plus those decisions. It never widens a chat's powers.

**Never, whatever the scope.** Each of these stays on the list with a recommendation, for him:

- Money: paying, buying, transferring, trades, securities orders. He does not want "a version of me to do my money and my trades and stuff without me calling it."
- Anything signed or legally binding.
- Deleting an account.
- A first message to someone new in his name.

**Decide.**

1. Read `workspace/life/decision-profile.md`, the standing rules distilled from his orders and closed decisions ([`decision-profile.md`](../schema/decision-profile.md)).
2. Find the closest past decision: an order (`src-...`) or a month-log line (`log YYYY-MM-DD`).
3. Decide the way he did, then run `af needs close <n> --outcome "<what I chose>, <the precedent>" --as-him`. The outcome names the precedent, or the button refuses.
4. When no precedent fits, or the precedents disagree, leave the item open with its recommendation.

Text sent in his voice loads `af voice bundle` first (`--register chat` to text a friend).

**When he is back.** Show him the "Decided as you" section of `workspace/life/needs.md`. After he has looked, run `af act review`, then `af act stop [--outcome "<text>"]`. Overturning a decision is an ordinary new loop or need.

**Refresh the profile.** Rebuild `workspace/life/decision-profile.md` with [`re-distill.md`](re-distill.md): the origin material is his orders and the closed lines in the month logs, read by a worker that main starts. Refresh it when he asks, or before a handover that will lean on it when the file is missing or older than his latest orders.

## Verification

- `af act status` names the scope and the time, and the line in the month log reads `as him [<id>]:`.
- Every `--as-him` outcome names a precedent that exists.
- No item from the never-list was closed.

## Boundaries

Does not start a handover by itself, change what a chat may do without asking, or send, buy or sign anything. Securities orders stay his word, each approved in-app.
