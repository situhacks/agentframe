//! `af data sql` and what renders pages: every registered ledger loaded into an in-memory SQLite,
//! one read-only statement run against it, and the result formatted as markdown, CSV or JSON.
//! Nothing is kept: the database is dropped when the command ends.

use super::ledger::{load_checked, quote_ident, Registry, Ty};
use crate::core::Ctx;
use rusqlite::types::{Value, ValueRef};
use rusqlite::Connection;

/// One result cell.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(usize),
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

pub enum QueryErr {
    /// The statement is not one af runs (not a single read-only select).
    Refused(String),
    /// SQLite's own message.
    Sql(String),
}

/// A REAL value to four decimal places with the trailing zeros removed, so a sum of prices does not
/// print binary noise.
pub fn fmt_real(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

impl Cell {
    pub fn text(&self) -> String {
        match self {
            Cell::Null => String::new(),
            Cell::Int(i) => i.to_string(),
            Cell::Real(r) => fmt_real(*r),
            Cell::Text(t) => t.clone(),
            Cell::Blob(n) => format!("<{n} bytes>"),
        }
    }
}

// ------------------------------------------------------------------ loading

/// Every ledger as a table named after it. A ledger whose file fails its checks stops the load: the
/// faults come back and nothing is queried.
pub fn load_db(ctx: &Ctx, reg: &Registry) -> Result<Connection, Vec<String>> {
    let mut faults = Vec::new();
    let mut loaded = Vec::new();
    for l in &reg.ledgers {
        match load_checked(ctx, l) {
            Ok((raw, f)) if f.is_empty() => loaded.push((l, raw)),
            Ok((_, f)) => faults.extend(f),
            Err(f) => faults.extend(f),
        }
    }
    if !faults.is_empty() {
        return Err(faults);
    }
    let fail = |e: rusqlite::Error| vec![format!("the in-memory database failed: {e}")];
    let con = Connection::open_in_memory().map_err(fail)?;
    con.execute_batch("begin").map_err(fail)?;
    for (l, raw) in &loaded {
        let cols: Vec<String> = l.columns.iter().map(|c| format!("{} {}", quote_ident(&c.name), c.ty.sql())).collect();
        con.execute_batch(&format!("create table {} ({})", quote_ident(&l.name), cols.join(", "))).map_err(fail)?;
        let marks = vec!["?"; l.columns.len()].join(", ");
        let mut ins = con.prepare(&format!("insert into {} values ({marks})", quote_ident(&l.name))).map_err(fail)?;
        for (_, row) in &raw.rows {
            let vals: Vec<Value> = l
                .columns
                .iter()
                .zip(row)
                .map(|(c, v)| {
                    if v.is_empty() {
                        Value::Null
                    } else {
                        match c.ty {
                            Ty::Integer => v.parse::<i64>().map(Value::Integer).unwrap_or(Value::Null),
                            Ty::Number => v.parse::<f64>().map(Value::Real).unwrap_or(Value::Null),
                            Ty::Date | Ty::Text => Value::Text(v.clone()),
                        }
                    }
                })
                .collect();
            ins.execute(rusqlite::params_from_iter(vals)).map_err(fail)?;
        }
    }
    con.execute_batch("commit").map_err(fail)?;
    Ok(con)
}

/// The tables and columns, one line per ledger, for an error message.
pub fn schema_lines(reg: &Registry) -> Vec<String> {
    if reg.ledgers.is_empty() {
        return vec!["no ledgers are registered".to_string()];
    }
    let mut v = vec!["tables:".to_string()];
    for l in &reg.ledgers {
        let cols: Vec<String> = l.columns.iter().map(|c| format!("{} {}", c.name, c.ty.name())).collect();
        v.push(format!("  {} ({})", l.name, cols.join(", ")));
    }
    v
}

// ------------------------------------------------------------------ the one read-only statement

/// The first word of the statement after white space and comments, lower-cased.
fn first_word(sql: &str) -> Option<String> {
    let b = sql.as_bytes();
    let mut i = 0;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i..].starts_with(b"--") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            match sql[i + 2..].find("*/") {
                Some(n) => i += 2 + n + 2,
                None => return None,
            }
        } else {
            break;
        }
    }
    let start = i;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
        i += 1;
    }
    (i > start).then(|| sql[start..i].to_ascii_lowercase())
}

/// Run one statement: it must start with `select` or `with`, be a single statement, and be reported
/// read-only by SQLite. At most `limit` rows are kept (None keeps all); the second value is how many
/// rows the statement gave in all.
pub fn run(con: &Connection, sql: &str, limit: Option<usize>) -> Result<(Table, usize), QueryErr> {
    match first_word(sql).as_deref() {
        Some("select") | Some("with") => {}
        _ => return Err(QueryErr::Refused("only a single select (or with ... select) statement is run; af data sql is read-only".to_string())),
    }
    if sql.to_ascii_lowercase().contains("load_extension") {
        return Err(QueryErr::Refused("load_extension is not allowed".to_string()));
    }
    let mut stmt = match con.prepare(sql) {
        Ok(s) => s,
        Err(rusqlite::Error::MultipleStatement) => return Err(QueryErr::Refused("give one statement, not several".to_string())),
        Err(e) => return Err(QueryErr::Sql(e.to_string())),
    };
    if !stmt.readonly() {
        return Err(QueryErr::Refused("the statement is not read-only; af data sql only reads".to_string()));
    }
    let headers: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let n = headers.len();
    let mut rows = stmt.query([]).map_err(|e| QueryErr::Sql(e.to_string()))?;
    let mut table = Table { headers, rows: Vec::new() };
    let mut total = 0usize;
    while let Some(row) = rows.next().map_err(|e| QueryErr::Sql(e.to_string()))? {
        total += 1;
        if limit.is_some_and(|l| table.rows.len() >= l) {
            continue;
        }
        let mut cells = Vec::with_capacity(n);
        for i in 0..n {
            let v = row.get_ref(i).map_err(|e| QueryErr::Sql(e.to_string()))?;
            cells.push(match v {
                ValueRef::Null => Cell::Null,
                ValueRef::Integer(i) => Cell::Int(i),
                ValueRef::Real(r) => Cell::Real(r),
                ValueRef::Text(t) => Cell::Text(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => Cell::Blob(b.len()),
            });
        }
        table.rows.push(cells);
    }
    Ok((table, total))
}

// ------------------------------------------------------------------ formats

fn md_cell(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\n', '\r'], " ").replace('|', "\\|")
}

/// A markdown table; no rows gives the header, then a line `(no rows)`.
pub fn to_markdown(t: &Table) -> String {
    let head: Vec<String> = t.headers.iter().map(|h| md_cell(h)).collect();
    let mut lines = vec![format!("| {} |", head.join(" | ")), format!("|{}|", vec!["---"; head.len()].join("|"))];
    if t.rows.is_empty() {
        lines.push(String::new());
        lines.push("(no rows)".to_string());
    }
    for r in &t.rows {
        let cells: Vec<String> = r.iter().map(|c| md_cell(&c.text())).collect();
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    lines.join("\n")
}

pub fn to_csv(t: &Table) -> String {
    let mut w = csv::WriterBuilder::new().terminator(csv::Terminator::Any(b'\n')).from_writer(Vec::new());
    let _ = w.write_record(&t.headers);
    for r in &t.rows {
        let _ = w.write_record(r.iter().map(Cell::text));
    }
    let s = String::from_utf8_lossy(&w.into_inner().unwrap_or_default()).into_owned();
    s.trim_end_matches('\n').to_string()
}

pub fn to_json(t: &Table) -> String {
    use crate::runlog::json_str;
    if t.rows.is_empty() {
        return "[]".to_string();
    }
    let rows: Vec<String> = t
        .rows
        .iter()
        .map(|r| {
            let fields: Vec<String> = t
                .headers
                .iter()
                .zip(r)
                .map(|(h, c)| {
                    let v = match c {
                        Cell::Null => "null".to_string(),
                        Cell::Int(i) => i.to_string(),
                        Cell::Real(x) if x.is_finite() => fmt_real(*x),
                        Cell::Real(_) => "null".to_string(),
                        Cell::Text(s) => json_str(s),
                        Cell::Blob(_) => json_str(&c.text()),
                    };
                    format!("{}: {v}", json_str(h))
                })
                .collect();
            format!("  {{{}}}", fields.join(", "))
        })
        .collect();
    format!("[\n{}\n]", rows.join(",\n"))
}
