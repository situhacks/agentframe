# Act As Me

## Purpose

Owns how a chat decides his open decisions in his name when he has handed them over. His rule: "it's not supposed to be a hard rule. It's based off of what I tell and dictate to you."

## When To Load

- Session start prints `Acting as him until <time>: "<scope>" [<id>]`.
- He says to act as him, take care of his needs list, or do whatever while he is away.

## Procedure

**Start.** Only his words start it. Main files them verbatim as an order first ([`source-record.md`](../schema/source-record.md)), then runs `af act start --from <order id> --scope "<his words>" [--until <time>]`. The order must be from today or yesterday and contain the scope; a handover lasts 7 days at most. Nothing else starts it: not idle time, not the night mute.

**Scope.** The scope covers only what he said. "My needs list" covers the decisions on [the needs list](../schema/needs-list.md). "Do whatever" covers what any chat may already do without asking, plus those decisions.

**Never, whatever the scope.** Each of these stays on the list with a recommendation, for him:

- Money: paying, buying, transferring, trades, securities orders. He does not want "a version of me to do my money ... without me calling it."
- Anything signed or legally binding.
- Deleting an account.
- A first message to someone new in his name.

**Decide.**

1. Read `workspace/life/decision-profile.md` ([shape](../schema/decision-profile.md)).
2. Find the closest past decision: a filed source (`src-...`) or a month-log day (`log YYYY-MM-DD`).
3. Decide the way he did, then run `af needs close <n> --outcome "<what I chose>, <the precedent>" --as-him`. The button refuses a precedent that does not exist or is the order that handed over.
4. When no precedent fits, or the precedents disagree, leave the item open with its recommendation.
5. A close without `--as-him` must cite an order he filed after the handover started: his own answer.

**Messages.** A message in his name goes only to someone he already talks with, and only when the scope covers messaging ("reply to X for me"). Otherwise the item stays on the list with a draft. Text in his voice loads `af voice bundle` first (`--register chat` to text a friend).

**When he is back.** Show him the "Decided as you" section of `workspace/life/needs.md`. After he has looked, run `af act review`, then `af act stop [--outcome "<text>"]`. Overturning one is a new loop.

**Refresh the profile.** Rebuild `workspace/life/decision-profile.md` with [`re-distill.md`](re-distill.md), in a worker main starts, when he asks or when the file is missing or older than his latest orders.

## Verification

- `af act status` names the scope and the time; the month log reads `as him [<id>]:`; no never-list item was closed.

## Boundaries

Does not start a handover by itself, widen what a chat may do without asking, or buy or sign anything. A message in his name follows the Messages rule. Securities orders stay his word, each approved in-app.
