---
name: price-read
description: The Thursday logged-in price read. In the operator's own Chrome, reads Metro Moi offers, PC Optimum offers, and Costco.ca bulk items against the groceries watch list, and appends price rows through read_prices.py.
disable-model-invocation: true
---

Price-read mode. The groceries system lives in `workspace/life/systems/groceries/` (call it GRO). It runs on Thursday, after the flyer check.

## 1. Check the browser first

Load the `anthropic-skills:chrome-browser` skill, then run ToolSearch for "chrome" and confirm the `mcp__claude-in-chrome__*` tools are present. If they are absent, reply only: "Run `/price-read` in a Claude Code VS Code session with the Chrome extension connected." Say nothing else and stop.

## 2. Read the watch list

Read `GRO/watch.json`. Each item has `match` and `exclude` keywords, a `basis` (`lb` or `unit`), and the `good` and `stock_up` thresholds. The Costco items are the ones carrying `"bulk": true`; if none does, skip Costco and say so in the report.

## 3. Read the three pages

In his Chrome, signed in as he already is:

- **Metro-Moi**: the Moi personalized coupons and offers page.
- **PC Optimum**: the offers page.
- **Costco.ca**: a search for each bulk item.

Keep only offers that match a watch item: a `match` keyword in the offer name and no `exclude` keyword. For each one, read the pack weight or volume and the unit price the page prints (per lb, per kg, per 100 g, per can). Never record a headline price alone: a case of 8 × 796 ml at $30 is $3.75 a can, and that per-can figure is what gets judged.

If a page asks for a sign-in, stop reading that site and tell him. Never type a credential. Nothing from his accounts is written anywhere except the price rows.

## 4. Build the rows

One row per matched offer, with exactly these keys:

| Key | Value |
|---|---|
| `seen` | today, `YYYY-MM-DD` |
| `store` | `Metro-Moi`, `PC Optimum`, or `Costco.ca` |
| `valid_from`, `valid_to` | the offer dates as `YYYY-MM-DD`; empty when the page shows none |
| `watch_item` | the watch.json key |
| `flyer_line` | the offer name as printed |
| `price` | the price of one purchase, two decimals |
| `unit` | `/lb`, `/kg`, `/100 g`, or `ea.` |
| `per_lb` | the rule below, two decimals; empty when no weight is known |
| `verdict` | the rule below |
| `pack` | the name plus the size text the page shows, such as `8 x 796 ml` |

`per_lb`: import `parse_price` from `GRO/check_flyers.py` and call it as `parse_price({"name": flyer_line, "price": price}, {"current_price": price, "price_text": <the page's unit text, such as "/lb">, "description": pack})`; it returns `(price, unit, per_lb, pack)`, and liquids count as water. Multipacks by weight or volume (`8 x 796 ml`) are counted in full by the parser. Leave per_lb empty when no weight is known; the row is still accepted.

`verdict`: call `verdict(spec, price, unit, per_lb, pack)` from `GRO/check_flyers.py` with the item's watch.json entry as `spec` and the pack text; it applies the item's `good` and `stock_up` for all three bases (`lb` per pound, `unit` per unit, `each` per item of a multipack) and returns the text to store (`STOCK UP`, `good`, `meh`, or `size unknown`). Do not restate the thresholds.

## 5. Append

Write the rows as a JSON list to a file in your scratchpad, then from the vault root run:

`python workspace/life/systems/groceries/read_prices.py --append <file> --dry-run`

It checks the eleven keys and the store names and writes nothing; exit 2 names the bad input, so fix the rows and rerun. When the dry run is clean, run the same command without `--dry-run`. It appends to `GRO/prices-history.csv` and adds the week's "Logged-in reads" section to `GRO/inbox/flyer-check-<date>.md`. A rerun skips rows already logged.

## 6. Report

Under ten lines: rows appended per store, each watch item searched and not found, and any page that needed a sign-in. Close with one line: posting the list to Discord is his step, `python notify.py list.md` from `GRO`. Do not run it.
