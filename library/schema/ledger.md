# Ledgers

A **ledger** is one CSV file under `workspace/life/` whose rows are one kind of event. `workspace/life/ledgers.yaml`, written only by `af data`, names each.

```yaml
ledgers:
  - name: sets
    file: systems/fitness/sets.csv
    key: [source, exercise, set]
    columns:
      - {name: date, type: date, required: true}
pages: [systems/fitness/log.md]
```

`file` is relative to `workspace/life/`, ends `.csv`. Key columns are required; no two rows share a key. Names are lower-case letters, digits, underscores, starting with a letter. A ledger name must work bare in `select * from <name>` (`group` is refused); a column named `set`, `group` or `order` needs double quotes in SQL.

Types: `date` (`YYYY-MM-DD`), `integer`, `number` (plain decimal, at most 12 significant digits; not `1,200` or `1e3`), `text` (one line). An empty value is NULL unless required. Values are trimmed when written; an `integer` is written canonically (`01` is `1`), a `number` as given (`5.50` stays). Keys, "unchanged" and the doctor compare values, not text: `1` is `01`, `5` is `5.0`, `abc` is `abc `.

## Verbs

- `af data new <name> --file <path> --column name:type[!]... --key a,b`: `!` marks required. An existing CSV is adopted if every row passes.
- `af data column <ledger> name:type`: an optional column; run again to finish a half-added one.
- `af data add <ledger> col=value...`; `--replace` merges into the row with that key: columns given are set (`col=` empties), the rest keep their values.
- `af data import <ledger> --csv <file>`: any column order; a same row is skipped, a changed row refused unless `--replace`, which merges per row. A repeated key or bad row writes nothing. `--scope col=value...` replaces the rows matching every pair with the file's rows (each file row must match; an empty value is refused); `--whole` replaces every row.
- `af data delete <ledger> col=value... [--dry-run]`: removes and lists rows matching every condition (by value); other lines keep their bytes.
- `af data list [<ledger>]`; `af data sql "<select>" [--json|--csv] [--limit N]`: read-only, 200 rows, stopped after 10 seconds. It prints numbers to at most 12 significant digits; the file keeps what was written.

`af doctor` flags a bad file or `pages:` entry.

## Pages

A block in a `.md` page under `workspace/life/`, outside code fences, refilled by `af data render` and, for pages added with `af data page <file>`, after each write. At most 500 rows; a failing block keeps its content.

```
<!-- af-data: select date, weight from sets where exercise = 'Squat' order by date desc limit 10 -->
(generated table)
<!-- /af-data -->
```

## Suggested shapes

`*` is the key.

| Ledger | Columns | Example |
|---|---|---|
| `sets` | `date`, `source*`, `exercise*`, `set*`, `weight` | 2026-03-02, w1, Squat, 1, 100 |
| `body` | `date*`, `kg` | 2026-03-02, 80.5 |
| `transactions` | `date`, `account*`, `ref*`, `amount` | 2026-03-04, chequing, t-102, -12.5 |
| `balances` | `as_of*`, `account*`, `balance` | 2026-03-31, chequing, 1520.4 |
