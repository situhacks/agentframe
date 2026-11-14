//! `af data`: ledgers (one CSV file of rows per kind of event), the SQL that asks questions of them,
//! and the pages that show their rows. The registry is `workspace/life/ledgers.yaml`, written only
//! here; shapes are in `library/schema/ledger.md`.
//!
//! A ledger file is appended to, never rewritten, unless a row is replaced or a column added; every
//! write runs under `life::locked` on the ledger's file. Nothing is kept between commands: a query or
//! a page render loads the files into an in-memory SQLite and drops it.

pub mod ledger;
pub mod pages;
pub mod query;

use crate::core::Ctx;
use crate::die;
use crate::error::{refuse, R};
use crate::out;
use crate::life::locked;
use crate::{outln, pyos};
use clap::{ArgAction, Args, Subcommand};
use ledger::{
    append_records, bare_table_name_ok, check_value, encode, ledger_file_rel, load_checked, page_rel, parse_col_spec, parse_csv, refuse_faults, rewrite, shown,
    valid_name, Col, Ledger, Raw, Registry, Ty,
};
use std::collections::HashMap;

#[derive(Subcommand, Debug)]
pub enum DataCmd {
    /// Register a ledger and create its CSV file (an existing file is adopted if it passes the checks)
    New(NewArgs),
    /// Add a column at the end of a ledger: the registry, the header, and an empty value on every row
    Column(ColumnArgs),
    /// Append one row (--replace replaces the row with the same key)
    Add(AddArgs),
    /// Add the rows of a CSV file: new keys appended, same rows skipped, changed rows need --replace
    Import(ImportArgs),
    /// Each ledger: file, rows, date range, columns; with a name, its last five rows too
    List {
        ledger: Option<String>,
    },
    /// Run one read-only SELECT over every ledger (a table named after each)
    Sql(SqlArgs),
    /// Refill the af-data blocks of these pages (none given: every registered page) from their queries
    Render {
        /// Pages inside workspace/life/
        files: Vec<String>,
    },
    /// Register a page that holds af-data blocks, then render it
    Page {
        /// A page inside workspace/life/
        file: String,
    },
}

#[derive(Args, Debug)]
pub struct SqlArgs {
    /// One statement starting with select or with (a leading comment is fine)
    #[arg(allow_hyphen_values = true)]
    statement: String,
    /// An array of objects instead of a markdown table
    #[arg(long, conflicts_with = "csv")]
    json: bool,
    /// CSV instead of a markdown table
    #[arg(long)]
    csv: bool,
    /// Rows to print (default 200)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    limit: Option<u64>,
}

/// Rows `af data sql` prints unless --limit says otherwise.
pub const DEFAULT_LIMIT: usize = 200;

#[derive(Args, Debug)]
pub struct NewArgs {
    /// Lower-case letters, digits and underscores, starting with a letter
    name: String,
    /// The CSV file, relative to workspace/life/
    #[arg(long)]
    file: String,
    /// name:type, with ! after the type when required (date:date!); types are date, integer, number, text
    #[arg(long, num_args = 1.., action = ArgAction::Append)]
    column: Vec<String>,
    /// The columns that make a row unique, comma-separated
    #[arg(long, value_delimiter = ',', required = true)]
    key: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ColumnArgs {
    ledger: String,
    /// name:type (never required)
    spec: String,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    ledger: String,
    /// column=value, one per column to set
    values: Vec<String>,
    /// Replace the row that has this key instead of refusing
    #[arg(long)]
    replace: bool,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    ledger: String,
    /// A CSV file whose header names the columns it carries (it may be anywhere)
    #[arg(long)]
    csv: String,
    /// Replace rows whose key exists with different values instead of refusing
    #[arg(long)]
    replace: bool,
}

pub fn run(ctx: &Ctx, cmd: DataCmd) -> R<i32> {
    match cmd {
        DataCmd::New(a) => cmd_new(ctx, &a),
        DataCmd::Column(a) => cmd_column(ctx, &a),
        DataCmd::Add(a) => cmd_add(ctx, &a),
        DataCmd::Import(a) => cmd_import(ctx, &a),
        DataCmd::List { ledger } => cmd_list(ctx, ledger.as_deref()),
        DataCmd::Sql(a) => cmd_sql(ctx, &a),
        DataCmd::Render { files } => cmd_render(ctx, &files),
        DataCmd::Page { file } => cmd_page(ctx, &file),
    }
}

// ------------------------------------------------------------------ helpers

fn io_err(e: &std::io::Error, path: &str) -> crate::error::AfError {
    pyos::os_error(e, path)
}

/// Create `path` and write `data`; a file that exists is refused and nothing is written.
fn create_new(path: &str, data: &[u8]) -> R<()> {
    use std::io::Write;
    let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => die!("{path} already exists; nothing was written"),
        Err(e) => return Err(io_err(&e, path)),
    };
    if let Err(e) = f.write_all(data).and_then(|_| f.flush()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(io_err(&e, path));
    }
    Ok(())
}

/// The registry and the named ledger, or the refusal that lists what is registered.
fn find_ledger(ctx: &Ctx, name: &str) -> R<(Registry, Ledger)> {
    let reg = match Registry::load(ctx)? {
        Some(r) => r,
        None => die!("no ledgers are registered ({} does not exist); af data new makes the first", ledger::REGISTRY_REL),
    };
    match reg.get(name) {
        Some(l) => {
            let l = l.clone();
            Ok((reg, l))
        }
        None => die!("no ledger named '{name}' (registered: {})", reg.names()),
    }
}

/// After a write: refill every registered page. A fault is a warning; the write stands.
fn render_after_write(ctx: &Ctx) {
    for fault in pages::render_registered(ctx) {
        out::err_line(&format!("af: warning: {fault}"));
    }
}

fn header_of(l: &Ledger) -> Vec<String> {
    l.columns.iter().map(|c| c.name.clone()).collect()
}

// ------------------------------------------------------------------ new

fn cmd_new(ctx: &Ctx, a: &NewArgs) -> R<i32> {
    if !valid_name(&a.name) {
        die!("ledger name {:?} must be lower-case letters, digits and underscores, starting with a letter", a.name);
    }
    if !bare_table_name_ok(&a.name) {
        die!("ledger name {:?} is an SQLite keyword or a reserved name; `select * from {}` would not run; pick another", a.name, a.name);
    }
    let file = ledger_file_rel(&a.file)?;
    if a.column.is_empty() {
        die!("give at least one --column name:type");
    }
    let mut columns: Vec<Col> = Vec::new();
    for spec in &a.column {
        let c = parse_col_spec(spec)?;
        if columns.iter().any(|x| x.name == c.name) {
            die!("column '{}' is given twice", c.name);
        }
        columns.push(c);
    }
    let mut key: Vec<String> = Vec::new();
    for k in a.key.iter().map(|k| k.trim()) {
        match columns.iter().find(|c| c.name == k) {
            None => die!("--key names '{k}', which is not one of the columns"),
            Some(c) if !c.required => die!("key column '{k}' must be required: write it as --column {k}:{}!", c.ty.name()),
            _ => {}
        }
        if key.iter().any(|x| x == k) {
            die!("--key names '{k}' twice");
        }
        key.push(k.to_string());
    }
    let new = Ledger { name: a.name.clone(), file, key, columns };
    let csv_path = new.path(ctx);
    let adopted = locked(&csv_path, || {
        locked(&Registry::path(ctx), || {
            let mut reg = Registry::load(ctx)?.unwrap_or_default();
            if reg.get(&new.name).is_some() {
                die!("ledger '{}' is already registered", new.name);
            }
            if let Some(o) = reg.ledgers.iter().find(|o| o.file.eq_ignore_ascii_case(&new.file)) {
                die!("{} is already the file of ledger '{}'", new.file, o.name);
            }
            let existed = pyos::isfile(&csv_path);
            let rows = if existed {
                match load_checked(ctx, &new) {
                    Err(f) => return Err(refuse_faults(&f, "the existing file cannot be adopted; no ledger was registered")),
                    Ok((_, f)) if !f.is_empty() => {
                        return Err(refuse_faults(&f, &format!("the existing file fails its checks ({} faults); no ledger was registered", f.len())))
                    }
                    Ok((raw, _)) => Some(raw.rows.len()),
                }
            } else {
                create_new(&csv_path, &encode(&[header_of(&new)], false))?;
                None
            };
            reg.ledgers.push(new.clone());
            if let Err(e) = reg.save(ctx) {
                if !existed {
                    let _ = std::fs::remove_file(&csv_path);
                }
                return Err(e);
            }
            Ok(rows)
        })
    })?;
    match adopted {
        Some(n) => outln!("adopted {} as ledger '{}' ({n} rows)", new.label(), new.name),
        None => outln!("created {} as ledger '{}'", new.label(), new.name),
    }
    Ok(0)
}

// ------------------------------------------------------------------ column

fn cmd_column(ctx: &Ctx, a: &ColumnArgs) -> R<i32> {
    let col = parse_col_spec(&a.spec)?;
    if col.required {
        die!("a column added to a ledger cannot be required: the rows already there have no value for it");
    }
    let (_, ledger) = find_ledger(ctx, &a.ledger)?;
    let path = ledger.path(ctx);
    let rows = locked(&path, || {
        locked(&Registry::path(ctx), || {
            let (mut reg, mut ledger) = find_ledger(ctx, &a.ledger)?;
            if ledger.col_index(&col.name).is_some() {
                die!("ledger '{}' already has a column '{}'", ledger.name, col.name);
            }
            let (raw, faults) = match load_checked(ctx, &ledger) {
                Ok(x) => x,
                Err(f) => return Err(refuse_faults(&f, "the ledger file cannot be read; no column was added")),
            };
            if !faults.is_empty() {
                return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); no column was added", faults.len())));
            }
            let mut records = vec![header_of(&ledger)];
            records[0].push(col.name.clone());
            for (_, r) in &raw.rows {
                let mut r = r.clone();
                r.push(String::new());
                records.push(r);
            }
            rewrite(&path, &raw, &records)?;
            ledger.columns.push(col.clone());
            if let Some(slot) = reg.ledgers.iter_mut().find(|l| l.name == ledger.name) {
                *slot = ledger;
            }
            reg.save(ctx)?;
            Ok(raw.rows.len())
        })
    })?;
    outln!("added column {} to '{}' ({rows} rows get an empty value)", col.spec(), a.ledger);
    render_after_write(ctx);
    Ok(0)
}

// ------------------------------------------------------------------ add

enum Done {
    Added,
    Replaced,
    Unchanged,
}

/// The row `col=value` arguments make, in column order; every refusal names the column.
fn build_row(l: &Ledger, values: &[String]) -> R<Vec<String>> {
    let mut row = vec![String::new(); l.columns.len()];
    let mut given = vec![false; l.columns.len()];
    for v in values {
        let Some((name, val)) = v.split_once('=') else { die!("{v:?} must be column=value") };
        let Some(i) = l.col_index(name) else { die!("ledger '{}' has no column '{name}' (columns: {})", l.name, header_of(l).join(", ")) };
        if given[i] {
            die!("column '{name}' is given twice");
        }
        given[i] = true;
        if !val.is_empty() {
            if let Err(e) = check_value(l.columns[i].ty, val) {
                die!("{name}: {e}");
            }
        }
        row[i] = val.to_string();
    }
    for (c, v) in l.columns.iter().zip(&row) {
        if c.required && v.is_empty() {
            die!("{} is required for ledger '{}'", c.name, l.name);
        }
    }
    Ok(row)
}

fn cmd_add(ctx: &Ctx, a: &AddArgs) -> R<i32> {
    let (_, first) = find_ledger(ctx, &a.ledger)?;
    if a.values.is_empty() {
        die!("give at least one column=value (columns: {})", header_of(&first).join(", "));
    }
    let path = first.path(ctx);
    let (done, name) = locked(&path, || {
        // The registry is read again under the lock: a column added meanwhile changes the row's shape.
        let (_, l) = find_ledger(ctx, &a.ledger)?;
        let (raw, faults) = match load_checked(ctx, &l) {
            Ok(x) => x,
            Err(f) => return Err(refuse_faults(&f, "the ledger file cannot be read; no row was added")),
        };
        if !faults.is_empty() {
            return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); no row was added", faults.len())));
        }
        let row = build_row(&l, &a.values)?;
        let key = l.key_of(&row);
        let at = raw.rows.iter().position(|(_, r)| l.key_of(r) == key);
        let done = match at {
            None => {
                append_records(&path, &raw, &[row])?;
                Done::Added
            }
            Some(i) if !a.replace => {
                die!("the key {} exists at line {}; --replace replaces that row", l.key_text(&row), raw.rows[i].0)
            }
            Some(i) if raw.rows[i].1 == row => Done::Unchanged,
            Some(i) => {
                let mut records = vec![header_of(&l)];
                records.extend(raw.rows.iter().map(|(_, r)| r.clone()));
                records[i + 1] = row;
                rewrite(&path, &raw, &records)?;
                Done::Replaced
            }
        };
        Ok((done, l.name.clone()))
    })?;
    match done {
        Done::Added => outln!("added 1 row to '{name}'"),
        Done::Replaced => outln!("replaced 1 row in '{name}'"),
        Done::Unchanged => outln!("the row in '{name}' already has these values; nothing changed"),
    }
    render_after_write(ctx);
    Ok(0)
}

// ------------------------------------------------------------------ import

fn cmd_import(ctx: &Ctx, a: &ImportArgs) -> R<i32> {
    let (_, first) = find_ledger(ctx, &a.ledger)?;
    let bytes = match std::fs::read(&a.csv) {
        Ok(b) => b,
        Err(e) => die!("cannot read --csv {}: {e}", a.csv),
    };
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes[..]);
    let mut records = match parse_csv(body) {
        Ok(r) => r,
        Err((line, what)) => return Err(refuse_faults(&[format!("{}:{line}: {what}", a.csv)], "the input is not readable; nothing was written")),
    };
    if records.is_empty() {
        die!("{} is empty; it needs a header row naming the columns it carries", a.csv);
    }
    let (_, header) = records.remove(0);
    let path = first.path(ctx);
    let (added, unchanged, replaced) = locked(&path, || {
        let (_, l) = find_ledger(ctx, &a.ledger)?;
        // Where each input column goes.
        let mut at: Vec<usize> = Vec::new();
        for h in &header {
            match l.col_index(h) {
                None => die!("{} has a column '{h}' that ledger '{}' does not (columns: {})", a.csv, l.name, header_of(&l).join(", ")),
                Some(i) if at.contains(&i) => die!("{} names column '{h}' twice", a.csv),
                Some(i) => at.push(i),
            }
        }
        if let Some(c) = l.columns.iter().enumerate().find(|(i, c)| c.required && !at.contains(i)).map(|(_, c)| c) {
            die!("{} has no column '{}', which ledger '{}' requires", a.csv, c.name, l.name);
        }
        let (raw, faults) = match load_checked(ctx, &l) {
            Ok(x) => x,
            Err(f) => return Err(refuse_faults(&f, "the ledger file cannot be read; nothing was written")),
        };
        if !faults.is_empty() {
            return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); nothing was written", faults.len())));
        }
        let mut rows: Vec<Vec<String>> = raw.rows.iter().map(|(_, r)| r.clone()).collect();
        let existing = rows.len();
        let mut index: HashMap<Vec<String>, usize> = rows.iter().enumerate().map(|(i, r)| (l.key_of(r), i)).collect();
        let (mut added, mut unchanged, mut replaced) = (0usize, 0usize, 0usize);
        let mut bad: Vec<String> = Vec::new();
        for (line, rec) in &records {
            if rec.len() != header.len() {
                bad.push(format!("{}:{line}: has {} fields, the header has {}", a.csv, rec.len(), header.len()));
                continue;
            }
            let mut row = vec![String::new(); l.columns.len()];
            for (v, &i) in rec.iter().zip(&at) {
                row[i] = v.clone();
            }
            let mut ok = true;
            for (c, v) in l.columns.iter().zip(&row) {
                if v.is_empty() {
                    if c.required {
                        bad.push(format!("{}:{line}: {} is required and empty", a.csv, c.name));
                        ok = false;
                    }
                } else if let Err(e) = check_value(c.ty, v) {
                    bad.push(format!("{}:{line}: {}: {e}", a.csv, c.name));
                    ok = false;
                }
            }
            if !ok {
                continue;
            }
            let key = l.key_of(&row);
            match index.get(&key).copied() {
                None => {
                    index.insert(key, rows.len());
                    rows.push(row);
                    added += 1;
                }
                Some(i) if rows[i] == row => unchanged += 1,
                Some(i) if a.replace => {
                    rows[i] = row;
                    replaced += 1;
                }
                Some(i) => {
                    let where_ = if i < existing { format!("is at line {} of the ledger", raw.rows[i].0) } else { "repeats an earlier row of the input".to_string() };
                    bad.push(format!("{}:{line}: the key {} {where_} with different values; --replace replaces it", a.csv, l.key_text(&row)))
                }
            }
        }
        if !bad.is_empty() {
            return Err(refuse_faults(&bad, &format!("{} rows of the input fail; nothing was written", bad.len())));
        }
        if replaced > 0 {
            let mut all = vec![header_of(&l)];
            all.extend(rows);
            rewrite(&path, &raw, &all)?;
        } else if added > 0 {
            append_records(&path, &raw, &rows[existing..])?;
        }
        Ok((added, unchanged, replaced))
    })?;
    outln!("added {added}, unchanged {unchanged}, replaced {replaced}");
    if added + replaced > 0 {
        render_after_write(ctx);
    }
    Ok(0)
}

// ------------------------------------------------------------------ list

fn describe(ctx: &Ctx, l: &Ledger, with_rows: bool) {
    let label = l.label();
    match load_checked(ctx, l) {
        Err(f) => {
            outln!("{}: {label}", l.name);
            for line in shown(&f) {
                outln!("  fault: {line}");
            }
        }
        Ok((raw, faults)) => {
            let dates = date_range(l, &raw);
            let span = dates.map(|(a, b)| format!(", {a} to {b}")).unwrap_or_default();
            outln!("{}: {label}, {} rows{span}", l.name, raw.rows.len());
            for f in shown(&faults) {
                outln!("  fault: {f}");
            }
        }
    }
    let cols: Vec<String> = l.columns.iter().map(Col::spec).collect();
    outln!("  columns: {}", cols.join(", "));
    outln!("  key: {}", l.key.join(", "));
    if with_rows {
        if let Ok((raw, _)) = load_checked(ctx, l) {
            let from = raw.rows.len().saturating_sub(5);
            let table = query::Table {
                headers: header_of(l),
                rows: raw.rows[from..].iter().map(|(_, r)| r.iter().map(|v| query::Cell::Text(v.clone())).collect()).collect(),
            };
            outln!("");
            outln!("last {} rows:", table.rows.len());
            outln!("{}", query::to_markdown(&table));
        }
    }
}

/// The first and last value of the `date` column (the first date-typed column when none is named
/// `date`), ignoring empty or invalid values.
fn date_range(l: &Ledger, raw: &Raw) -> Option<(String, String)> {
    let i = l.columns.iter().position(|c| c.name == "date" && c.ty == Ty::Date).or_else(|| l.columns.iter().position(|c| c.ty == Ty::Date))?;
    let vals: Vec<&String> = raw.rows.iter().filter_map(|(_, r)| r.get(i)).filter(|v| check_value(Ty::Date, v).is_ok()).collect();
    Some((vals.iter().min()?.to_string(), vals.iter().max()?.to_string()))
}

fn cmd_list(ctx: &Ctx, name: Option<&str>) -> R<i32> {
    match name {
        Some(n) => {
            let (_, l) = find_ledger(ctx, n)?;
            describe(ctx, &l, true);
        }
        None => match Registry::load(ctx)? {
            Some(reg) if !reg.ledgers.is_empty() => {
                for l in &reg.ledgers {
                    describe(ctx, l, false);
                }
                if !reg.pages.is_empty() {
                    outln!("pages: {}", reg.pages.join(", "));
                }
            }
            _ => outln!("no ledgers are registered; af data new makes the first"),
        },
    }
    Ok(0)
}

// ------------------------------------------------------------------ sql

fn cmd_sql(ctx: &Ctx, a: &SqlArgs) -> R<i32> {
    let reg = Registry::load(ctx)?.unwrap_or_default();
    let con = match query::load_db(ctx, &reg) {
        Ok(c) => c,
        Err(faults) => {
            return Err(refuse_faults(&faults, &format!("{} fault(s) in the ledger files; nothing was queried: a wrong number is worse than no number", faults.len())))
        }
    };
    let limit = a.limit.map(|l| l as usize).unwrap_or(DEFAULT_LIMIT);
    let (table, total) = match query::run(&con, &a.statement, Some(limit)) {
        Ok(x) => x,
        Err(query::QueryErr::Refused(m)) => die!("{m}"),
        Err(query::QueryErr::Sql(m)) => {
            let e = refuse(format!("SQL error: {m}"));
            for l in query::schema_lines(&reg) {
                out::err_line(&l);
            }
            if m.contains("syntax error") {
                out::err_line("a name that is an SQL keyword (set, group, order) needs double quotes: select \"set\" from ...");
            }
            return Err(e);
        }
    };
    let text = if a.json {
        query::to_json(&table)
    } else if a.csv {
        query::to_csv(&table)
    } else {
        query::to_markdown(&table)
    };
    outln!("{text}");
    if total > table.rows.len() {
        let note = format!("showing {} of {total} rows; --limit shows more", table.rows.len());
        if a.json || a.csv {
            // Stdout stays the data alone, so a reader of it still parses.
            out::err_line(&note);
        } else {
            outln!("");
            outln!("{note}");
        }
    }
    Ok(0)
}

// ------------------------------------------------------------------ render, page

/// Render these pages and say what happened to each; the faults make the exit 1, after every page
/// that could be rendered was.
fn render_pages(ctx: &Ctx, reg: &Registry, pages: &[String]) -> R<i32> {
    let mut faults = 0;
    for rel in pages {
        let o = pages::render_page(ctx, reg, rel);
        for f in &o.faults {
            out::err_line(f);
        }
        faults += o.faults.len();
        if o.faults.is_empty() || o.blocks > 0 {
            let what = if o.changed { "updated" } else { "unchanged" };
            let n = o.blocks;
            outln!("{}: {n} block{}, {what}", pages::label(rel), if n == 1 { "" } else { "s" });
        }
    }
    if faults > 0 {
        die!("{faults} fault(s) while rendering; every block and page that could be rendered was");
    }
    Ok(0)
}

fn cmd_render(ctx: &Ctx, files: &[String]) -> R<i32> {
    let reg = Registry::load(ctx)?.unwrap_or_default();
    let pages: Vec<String> = if files.is_empty() {
        reg.pages.clone()
    } else {
        // Every argument is checked before any page is touched.
        files.iter().map(|f| page_rel(ctx, f)).collect::<R<Vec<_>>>()?
    };
    if pages.is_empty() {
        outln!("no pages to render; af data page <file> registers one");
        return Ok(0);
    }
    render_pages(ctx, &reg, &pages)
}

fn cmd_page(ctx: &Ctx, file: &str) -> R<i32> {
    let rel = page_rel(ctx, file)?;
    let path = crate::life::vpath(ctx, &pages::label(&rel));
    let text = match std::fs::read(&path) {
        Ok(b) => String::from_utf8(b).or_else(|_| -> R<String> { die!("{} is not UTF-8 text", pages::label(&rel)) })?,
        Err(_) => die!("{} does not exist; a registered page has to exist", pages::label(&rel)),
    };
    match pages::find_blocks(&text) {
        Ok(b) if b.is_empty() => die!("{} holds no <!-- af-data: ... --> block; add one first", pages::label(&rel)),
        Ok(_) => {}
        Err((line, what)) => die!("{}:{line}: {what}", pages::label(&rel)),
    }
    let reg = locked(&Registry::path(ctx), || {
        let Some(mut reg) = Registry::load(ctx)? else { die!("no ledgers are registered ({} does not exist); af data new makes the first", ledger::REGISTRY_REL) };
        if !reg.pages.contains(&rel) {
            reg.pages.push(rel.clone());
            reg.save(ctx)?;
        }
        Ok(reg)
    })?;
    render_pages(ctx, &reg, &[rel])
}

// ------------------------------------------------------------------ the doctor

/// What `af doctor` adds for ledgers: (issues, notes). No registry file means silence.
///
/// Issues, at most five lines per ledger and then a count: a file that is missing, a header that
/// differs from the registry, a row that fails a check, a repeated key. Notes: a registered page that
/// is missing, a block that fails, a page whose content differs from what the ledgers give now.
pub fn doctor_checks(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let (mut issues, mut notes) = (Vec::new(), Vec::new());
    let reg = match crate::error::soft(Registry::load(ctx))? {
        None => {
            issues.push(format!("{}: does not parse; see the af data error above", ledger::REGISTRY_REL));
            return Ok((issues, notes));
        }
        Some(None) => return Ok((issues, notes)),
        Some(Some(r)) => r,
    };
    for l in &reg.ledgers {
        let faults = match load_checked(ctx, l) {
            Ok((_, f)) => f,
            Err(f) => f,
        };
        issues.extend(faults.iter().take(ledger::FAULTS_SHOWN).cloned());
        if faults.len() > ledger::FAULTS_SHOWN {
            issues.push(format!("{}: ... and {} more faults in ledger '{}'", l.label(), faults.len() - ledger::FAULTS_SHOWN, l.name));
        }
    }
    notes.extend(pages::doctor_notes(ctx, &reg));
    Ok((issues, notes))
}
