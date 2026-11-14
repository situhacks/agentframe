# Ledgers

A **ledger** is one CSV file under `workspace/life/`; its rows are the truth for one kind of event. `workspace/life/ledgers.yaml`, written only by `af data`, names each. A query loads the files into a throwaway in-memory SQLite.

```yaml
ledgers:
  - name: sets
    file: systems/fitness/sets.csv      # relative to workspace/life/, ends .csv
    key: [source, exercise, set]        # no two rows share a key; key columns are required
    columns:
      - {name: date, type: date, required: true}
      - {name: weight, type: number}
pages: [systems/fitness/log.md]
```

Names are lower-case letters, digits and underscores, starting with a letter. A ledger name must work bare in `select * from <name>` (`group` is refused). A column named `set`, `group` or `order` needs double quotes in SQL.

| Type | Holds | SQL |
|---|---|---|
| `date` | `YYYY-MM-DD` | TEXT |
| `integer` | digits, optional minus | INTEGER |
| `number` | plain decimal, optional minus; not `1,200` or `1e3` | REAL |
| `text` | one line | TEXT |

An empty value is NULL unless the column is required.

## Verbs

- `af data new <name> --file <path> --column name:type[!]... --key a,b`: `!` marks required. An existing CSV is adopted if every row passes; else faults print as `file:line: what`.
- `af data column <ledger> name:type`: an optional column; old rows stay empty there.
- `af data add <ledger> col=value...`; `--replace` replaces the row with that key.
- `af data import <ledger> --csv <file>`: any column order; a same row is skipped, a changed row refused without `--replace`; one bad row writes none.
- `af data list [<ledger>]`: rows, date range, columns; named, the last five rows.
- `af data sql "<select>" [--json|--csv] [--limit N]`: read-only, 200 rows unless limited; a bad file stops it.

Appends leave earlier bytes untouched. `af doctor` flags a missing file, changed header, bad row, repeated key.

## Pages

A block in a page under `workspace/life/`, refilled by `af data render`, and after each write for pages added with `af data page <file>`. Only what is between the markers changes; a failing block keeps its content.

```
<!-- af-data: select date, weight from sets where exercise = 'Squat' order by date desc limit 10 -->
(generated table)
<!-- /af-data -->
```

## Suggested shapes

Made-up rows; `*` is the key.

| Ledger | Columns | Example |
|---|---|---|
| `sets` | `date`, `exercise*`, `set*`, `weight`, `source*` | 2026-03-02, Squat, 1, 100, log |
| `body` | `date*`, `kg`, `waist_cm` | 2026-03-02, 80.5, 84 |
| `runs` | `date*`, `km`, `minutes` | 2026-03-03, 5.2, 28 |
| `meals` | `date*`, `meal*`, `kcal` | 2026-03-02, lunch, 650 |
| `transactions` | `date`, `account*`, `ref*`, `amount` | 2026-03-04, chequing, t-102, -12.5 |
| `balances` | `as_of*`, `account*`, `balance` | 2026-03-31, chequing, 1520.4 |
| `card_statements` | `statement_date*`, `card*`, `balance` | 2026-03-20, visa, 310 |
| `budget` | `month*`, `category*`, `planned` | 2026-03, food, 400 |
| `prices` | `date*`, `item*`, `store*`, `price` | 2026-03-02, milk, corner, 4.5 |
