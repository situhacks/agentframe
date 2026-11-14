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

/// Seconds a statement may run before it is stopped (10), in milliseconds; AF_DATA_SQL_TIMEOUT_MS overrides it.
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

pub enum QueryErr {
    /// The statement ran past the time limit and was stopped.
    Stopped(String),
    /// The statement is not one af runs (not a single read-only select).
    Refused(String),
    /// SQLite's own message.
    Sql(String),
}

/// A REAL value with up to twelve significant digits (more only where the integer part itself is longer,
/// to 17), trailing zeros removed, so a sum of prices does not print binary noise and a small value
/// does not print as 0.
pub fn fmt_real(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let exp10: i32 = format!("{:e}", v.abs()).split('e').nth(1).and_then(|e| e.parse().ok()).unwrap_or(0);
    let sig = ((exp10 + 1).max(12) as usize).min(17);
    let sci = format!("{:.*e}", sig - 1, v.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    // The value is 0.<digits> times 10^(exp + 1).
    let point = exp + 1;
    let plain = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if (point as usize) >= digits.len() {
        format!("{digits}{}", "0".repeat(point as usize - digits.len()))
    } else {
        format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
    };
    let plain = if plain.contains('.') { plain.trim_end_matches('0').trim_end_matches('.').to_string() } else { plain };
    if v < 0.0 {
        format!("-{plain}")
    } else {
        plain
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

/// How long a statement may run before it is stopped: `DEFAULT_TIMEOUT_MS`, or `AF_DATA_SQL_TIMEOUT_MS`.
pub fn timeout_ms() -> u64 {
    parse_timeout_ms(std::env::var("AF_DATA_SQL_TIMEOUT_MS").ok().as_deref())
}

/// The limit a value of `AF_DATA_SQL_TIMEOUT_MS` asks for.
///
/// Clamped to 1 to 60000 milliseconds, so the override can shorten the limit but never switch it off; a
/// value that is not a whole number is the default. (A number too large for u64 is still a number.)
pub fn parse_timeout_ms(raw: Option<&str>) -> u64 {
    let Some(v) = raw.map(str::trim) else { return DEFAULT_TIMEOUT_MS };
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return DEFAULT_TIMEOUT_MS;
    }
    v.parse::<u64>().map(|ms| ms.clamp(1, MAX_TIMEOUT_MS)).unwrap_or(MAX_TIMEOUT_MS)
}

/// The longest limit the override may ask for.
pub const MAX_TIMEOUT_MS: u64 = 60_000;

/// One time budget for a whole command (a render, the re-render after a write, the doctor's page check),
/// shared by every block of every page, so a page of slow blocks costs one limit, not one per block.
pub struct Deadline {
    end: std::time::Instant,
}

impl Deadline {
    pub fn new() -> Deadline {
        Deadline { end: std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms()) }
    }

    pub fn remaining_ms(&self) -> u64 {
        self.end.saturating_duration_since(std::time::Instant::now()).as_millis() as u64
    }
}

impl Default for Deadline {
    fn default() -> Self {
        Deadline::new()
    }
}

fn seconds_text(ms: u64) -> String {
    if ms % 1000 == 0 {
        let s = ms / 1000;
        format!("{s} second{}", if s == 1 { "" } else { "s" })
    } else {
        format!("{:.1} seconds", ms as f64 / 1000.0)
    }
}

/// Stops the statement that is running on a connection once the time is up. Dropping it (the statement
/// is done) ends the thread without an interrupt.
struct Watchdog {
    done: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    fired: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Watchdog {
    fn start(con: &Connection, ms: u64) -> Watchdog {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let fired = std::sync::Arc::new(AtomicBool::new(false));
        let (flag, handle) = (fired.clone(), con.get_interrupt_handle());
        let thread = std::thread::spawn(move || {
            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(std::time::Duration::from_millis(ms)) {
                flag.store(true, Ordering::SeqCst);
                handle.interrupt();
            }
        });
        Watchdog { done: Some(tx), thread: Some(thread), fired }
    }

    fn fired(&self) -> bool {
        self.fired.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.done.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// How many rows a statement gave: all of them, or more than `n` when counting stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Total {
    pub n: usize,
    pub exact: bool,
}

impl Total {
    pub fn text(&self) -> String {
        if self.exact {
            self.n.to_string()
        } else {
            format!("more than {}", self.n)
        }
    }
}

/// Run one statement: it must start with `select` or `with`, be a single statement, and be reported
/// read-only by SQLite. At most `keep` rows are kept. Rows past that are only counted: up to
/// `count_cap` of them when given, else until the time limit, when the count is "more than n". A
/// statement still giving no row (or fewer than `keep`) when the time is up is stopped with an error.
pub fn run(con: &Connection, sql: &str, keep: usize, count_cap: Option<usize>, deadline: Option<&Deadline>) -> Result<(Table, Total), QueryErr> {
    // A statement may run for the single-statement limit, or for what is left of its command's budget.
    let ms = match deadline {
        Some(d) => {
            let left = d.remaining_ms();
            if left == 0 {
                return Err(QueryErr::Stopped("not run: the time limit for this command was spent".to_string()));
            }
            left.min(timeout_ms())
        }
        None => timeout_ms(),
    };
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
    let stopped = || QueryErr::Stopped(format!("the query was stopped after {}", seconds_text(ms)));
    let dog = Watchdog::start(con, ms);
    let headers: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let n = headers.len();
    let mut rows = match stmt.query([]) {
        Ok(r) => r,
        Err(_) if dog.fired() => return Err(stopped()),
        Err(e) => return Err(QueryErr::Sql(e.to_string())),
    };
    let mut table = Table { headers, rows: Vec::new() };
    let mut total = Total { n: 0, exact: true };
    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(_) if dog.fired() => {
                if table.rows.len() >= keep {
                    total.exact = false;
                    break;
                }
                return Err(stopped());
            }
            Err(e) => return Err(QueryErr::Sql(e.to_string())),
        };
        total.n += 1;
        if table.rows.len() >= keep {
            if count_cap.is_some_and(|cap| total.n > cap) {
                total = Total { n: count_cap.unwrap_or(0), exact: false };
                break;
            }
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

/// One markdown table cell that cannot break its table or the page around it: line breaks become spaces,
/// `|` is escaped (a backslash before it is doubled first, so `\|` stays one backslash and one pipe), and
/// `<` becomes `&lt;`, so a value can never hold a marker comment.
fn md_cell(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\n', '\r'], " ").replace("\\|", "\\\\|").replace('|', "\\|").replace('<', "&lt;")
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
