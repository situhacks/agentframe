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
    valid_name, written, Col, Ledger, Raw, Registry, Ty,
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
    /// Remove every row matching all the given col=value (compared by value); other lines stay as they are
    Delete(DeleteArgs),
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
pub struct DeleteArgs {
    ledger: String,
    /// col=value, one per condition; a row goes only when it matches every one
    conditions: Vec<String>,
    /// Say how many rows would go and write nothing
    #[arg(long)]
    dry_run: bool,
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
        DataCmd::Delete(a) => cmd_delete(ctx, &a),
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
    // Nothing in the re-render may change the exit code: the write stands. Even a panic is a warning.
    let faults = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pages::render_registered(ctx)))
        .unwrap_or_else(|_| vec!["re-rendering the registered pages failed unexpectedly".to_string()]);
    for fault in faults {
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
                match ledger::load_checked_unlocked(ctx, &new) {
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

// ------------------------------------------------------------------ writes: shared

/// The ledger file for a write, read and checked while the caller holds its lock. A refusal says what
/// was not done, and when the file and the registry are one column apart, the command that finishes it.
fn load_for_write(ctx: &Ctx, l: &Ledger, not_done: &str) -> R<Raw> {
    let (raw, faults) = match ledger::load_checked_unlocked(ctx, l) {
        Ok(x) => x,
        Err(f) => return Err(refuse_faults(&f, &format!("the ledger file cannot be read; {not_done}"))),
    };
    if !faults.is_empty() {
        let hint = ledger::half_state_hint(l, &raw).map(|h| format!(" {h}")).unwrap_or_default();
        return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); {not_done}.{hint}", faults.len())));
    }
    Ok(raw)
}

/// Put a file's bytes back as they were (after a write that could not be finished).
fn restore(path: &str, original: &[u8]) {
    let text = String::from_utf8_lossy(original).into_owned();
    let _ = crate::life::TextFile { text, crlf: false, bom: false }.save_atomic(path);
}

/// `col=value` arguments as the written form of each column given (None: not given). Every refusal names
/// the column.
fn parse_given(l: &Ledger, values: &[String]) -> R<Vec<Option<String>>> {
    let mut given: Vec<Option<String>> = vec![None; l.columns.len()];
    for v in values {
        let Some((name, val)) = v.split_once('=') else { die!("{v:?} must be column=value") };
        let Some(i) = l.col_index(name) else { die!("ledger '{}' has no column '{name}' (columns: {})", l.name, header_of(l).join(", ")) };
        if given[i].is_some() {
            die!("column '{name}' is given twice");
        }
        match written(l.columns[i].ty, val) {
            Ok(w) => given[i] = Some(w),
            Err(e) => die!("{name}: {e}"),
        }
    }
    Ok(given)
}

/// A new row from the columns given: those not given are empty, and a required one may not be.
fn new_row(l: &Ledger, given: &[Option<String>]) -> R<Vec<String>> {
    let row: Vec<String> = given.iter().map(|g| g.clone().unwrap_or_default()).collect();
    for (c, v) in l.columns.iter().zip(&row) {
        if c.required && v.is_empty() {
            die!("{} is required for ledger '{}'", c.name, l.name);
        }
    }
    Ok(row)
}

/// The existing row with the given columns set; a required column may not end up empty.
fn merged_row(l: &Ledger, existing: &[String], given: &[Option<String>]) -> R<Vec<String>> {
    let mut row = existing.to_vec();
    for (i, g) in given.iter().enumerate() {
        if let Some(v) = g {
            row[i] = v.clone();
        }
    }
    for (c, v) in l.columns.iter().zip(&row) {
        if c.required && v.is_empty() {
            die!("{} is required for ledger '{}'", c.name, l.name);
        }
    }
    Ok(row)
}

fn given_cols(given: &[Option<String>]) -> Vec<usize> {
    given.iter().enumerate().filter(|(_, g)| g.is_some()).map(|(i, _)| i).collect()
}

/// The given columns as a row, for reading its key.
fn given_row(given: &[Option<String>]) -> Vec<String> {
    given.iter().map(|g| g.clone().unwrap_or_default()).collect()
}

fn require_key(l: &Ledger, given: &[Option<String>]) -> R<()> {
    for k in &l.key {
        if let Some(i) = l.col_index(k) {
            if given[i].as_deref().is_none_or(str::is_empty) {
                die!("{k} is required for ledger '{}'", l.name);
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ column

/// Writing the column's rows or the registry entry can be cut short, leaving the file and the registry one
/// column apart. Running the same `af data column` again finishes whichever half is missing.
/// How a `column` ended: all of it, or the half a crash had left.
enum How {
    Plain,
    RegistryAhead,
    FileAhead,
}

fn cmd_column(ctx: &Ctx, a: &ColumnArgs) -> R<i32> {
    let col = parse_col_spec(&a.spec)?;
    if col.required {
        die!("a column added to a ledger cannot be required: the rows already there have no value for it");
    }
    let (_, first) = find_ledger(ctx, &a.ledger)?;
    let path = first.path(ctx);
    let (rows, how) = locked(&path, || {
        locked(&Registry::path(ctx), || {
            let (mut reg, mut ledger) = find_ledger(ctx, &a.ledger)?;
            let label = ledger.label();
            let raw = match ledger::read_raw(&path, &label) {
                Ok(r) => r,
                Err(e) => return Err(refuse_faults(&[e], "the ledger file cannot be read; no column was added")),
            };
            let names = header_of(&ledger);
            let header: Vec<String> = raw.header.as_ref().map(|h| h.1.clone()).unwrap_or_default();
            let save_ledger = |reg: &mut Registry, ledger: Ledger| -> R<()> {
                if let Some(slot) = reg.ledgers.iter_mut().find(|l| l.name == ledger.name) {
                    *slot = ledger;
                }
                reg.save(ctx)
            };
            // The registry already has the column.
            if let Some(i) = ledger.col_index(&col.name) {
                let registered = ledger.columns[i].clone();
                if i + 1 != names.len() || header != names[..i] {
                    die!("ledger '{}' already has a column '{}'", ledger.name, col.name);
                }
                if registered.ty != col.ty {
                    die!("the registry has column '{}' as {}; run af data column {} {}:{} to finish adding it", col.name, registered.ty.name(), ledger.name, col.name, registered.ty.name());
                }
                // The file lacks it: give every row an empty value.
                let mut base = ledger.clone();
                base.columns.truncate(i);
                let faults = ledger::check(&base, &label, &raw);
                if !faults.is_empty() {
                    return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); no column was added", faults.len())));
                }
                let mut records = vec![names.clone()];
                records.extend(raw.rows.iter().map(|(_, r)| r.iter().cloned().chain(std::iter::once(String::new())).collect::<Vec<_>>()));
                rewrite(&path, &raw, &records)?;
                return Ok((raw.rows.len(), How::RegistryAhead));
            }
            // The file already has it and the registry lacks it.
            if header.len() == names.len() + 1 && header[..names.len()] == names[..] && header[names.len()] == col.name {
                let mut extended = ledger.clone();
                extended.columns.push(col.clone());
                let faults = ledger::check(&extended, &label, &raw);
                if !faults.is_empty() {
                    return Err(refuse_faults(&faults, &format!("the ledger file fails its checks ({} faults); no column was added", faults.len())));
                }
                save_ledger(&mut reg, extended)?;
                return Ok((raw.rows.len(), How::FileAhead));
            }
            // The ordinary case.
            let raw = load_for_write(ctx, &ledger, "no column was added")?;
            let original = std::fs::read(&path).map_err(|e| io_err(&e, &path))?;
            let mut records = vec![names.clone()];
            records[0].push(col.name.clone());
            for (_, r) in &raw.rows {
                let mut r = r.clone();
                r.push(String::new());
                records.push(r);
            }
            rewrite(&path, &raw, &records)?;
            ledger.columns.push(col.clone());
            if let Err(e) = save_ledger(&mut reg, ledger) {
                // The registry could not be written: the file goes back as it was, so nothing is half done.
                restore(&path, &original);
                return Err(e);
            }
            Ok((raw.rows.len(), How::Plain))
        })
    })?;
    match how {
        How::Plain => outln!("added column {} to '{}' ({rows} rows get an empty value)", col.spec(), a.ledger),
        How::RegistryAhead => outln!("the registry already had column {}; added it to the file ({rows} rows get an empty value)", col.spec()),
        How::FileAhead => outln!("the file already had column {}; registered it ({rows} rows keep their values)", col.spec()),
    }
    render_after_write(ctx);
    Ok(0)
}

// ------------------------------------------------------------------ add

enum Done {
    Added,
    Replaced,
    Unchanged,
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
        let raw = load_for_write(ctx, &l, "no row was added")?;
        let given = parse_given(&l, &a.values)?;
        require_key(&l, &given)?;
        let key_row = given_row(&given);
        let key = l.key_of(&key_row);
        let at = raw.rows.iter().position(|(_, r)| l.key_of(r) == key);
        let done = match at {
            None => {
                let row = new_row(&l, &given)?;
                append_records(&path, &raw, &[row])?;
                Done::Added
            }
            Some(i) if !a.replace => {
                die!("the key {} exists at line {}; --replace replaces that row", l.key_text(&key_row), raw.rows[i].0)
            }
            Some(i) => {
                let existing = &raw.rows[i].1;
                if l.same_in(existing, &key_row, &given_cols(&given)) {
                    Done::Unchanged
                } else {
                    let row = merged_row(&l, existing, &given)?;
                    let mut records = vec![header_of(&l)];
                    records.extend(raw.rows.iter().map(|(_, r)| r.clone()));
                    records[i + 1] = row;
                    rewrite(&path, &raw, &records)?;
                    Done::Replaced
                }
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

// ------------------------------------------------------------------ delete

/// Remove the matching rows by cutting their lines out of the file (every other byte stays), through a
/// temporary file and a rename, under the ledger's lock. Where `add --replace` and `column` rewrite the
/// file with the CSV writer (needless quotes and blank lines go; values stay exactly), this does not.
/// Removed rows `delete` lists, as they stood, before it counts the rest.
const DELETE_SHOWN: usize = 20;

fn cmd_delete(ctx: &Ctx, a: &DeleteArgs) -> R<i32> {
    let (_, first) = find_ledger(ctx, &a.ledger)?;
    if a.conditions.is_empty() {
        die!("give at least one column=value to match (columns: {})", header_of(&first).join(", "));
    }
    let path = first.path(ctx);
    let n = locked(&path, || {
        let (_, l) = find_ledger(ctx, &a.ledger)?;
        let raw = load_for_write(ctx, &l, "no row was deleted")?;
        let given = parse_given(&l, &a.conditions)?;
        let (cols, probe) = (given_cols(&given), given_row(&given));
        let hits: Vec<u64> = raw.rows.iter().filter(|(_, r)| l.same_in(r, &probe, &cols)).map(|(line, _)| *line).collect();
        if hits.is_empty() {
            die!("no row matches");
        }
        // What goes, as the lines stood, taken before anything is cut.
        let gone = ledger::lines_text(&path, &hits);
        if !a.dry_run {
            let kept: Vec<Vec<String>> = raw.rows.iter().filter(|(line, _)| !hits.contains(line)).map(|(_, r)| r.clone()).collect();
            ledger::delete_lines(&path, &l, &hits, &kept)?;
        }
        Ok((hits.len(), gone))
    })?;
    let (n, gone) = n;
    outln!("{} {n}", if a.dry_run { "would delete" } else { "deleted" });
    for line in gone.iter().take(DELETE_SHOWN) {
        outln!("{line}");
    }
    if gone.len() > DELETE_SHOWN {
        outln!("... and {} more", gone.len() - DELETE_SHOWN);
    }
    if !a.dry_run {
        render_after_write(ctx);
    }
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
        // The key says which row a line is about, so it must be there; another required column may be
        // left out, as long as no line needs it for a new row (checked per line below).
        if let Some(k) = l.key.iter().find(|k| l.col_index(k).is_some_and(|i| !at.contains(&i))) {
            die!("{} has no column '{k}', which ledger '{}' requires: it is part of the key", a.csv, l.name);
        }
        let raw = load_for_write(ctx, &l, "nothing was written")?;
        let mut rows: Vec<Vec<String>> = raw.rows.iter().map(|(_, r)| r.clone()).collect();
        let existing = rows.len();
        let index: HashMap<Vec<String>, usize> = rows.iter().enumerate().map(|(i, r)| (l.key_of(r), i)).collect();
        let (mut added, mut unchanged, mut replaced) = (0usize, 0usize, 0usize);
        let mut bad: Vec<String> = Vec::new();
        let mut seen: HashMap<Vec<String>, u64> = HashMap::new();
        for (line, rec) in &records {
            if rec.len() != header.len() {
                bad.push(format!("{}:{line}: has {} fields, the header has {}", a.csv, rec.len(), header.len()));
                continue;
            }
            // The columns this input carries, in their written form.
            let mut given: Vec<Option<String>> = vec![None; l.columns.len()];
            let mut ok = true;
            for (v, &i) in rec.iter().zip(&at) {
                match written(l.columns[i].ty, v) {
                    Ok(w) if w.is_empty() && l.columns[i].required => {
                        bad.push(format!("{}:{line}: {} is required and empty", a.csv, l.columns[i].name));
                        ok = false;
                    }
                    Ok(w) => given[i] = Some(w),
                    Err(e) => {
                        bad.push(format!("{}:{line}: {}: {e}", a.csv, l.columns[i].name));
                        ok = false;
                    }
                }
            }
            if !ok {
                continue;
            }
            let probe = given_row(&given);
            let key = l.key_of(&probe);
            if let Some(first_line) = seen.get(&key) {
                bad.push(format!("{}:{line}: the key {} repeats line {first_line} of the input", a.csv, l.key_text(&probe)));
                continue;
            }
            seen.insert(key.clone(), *line);
            match index.get(&key).copied() {
                None => {
                    match l.columns.iter().zip(&probe).find(|(c, v)| c.required && v.is_empty()) {
                        Some((c, _)) => bad.push(format!("{}:{line}: {} is required for a new row, and this input has no column for it", a.csv, c.name)),
                        None => {
                            rows.push(probe);
                            added += 1;
                        }
                    }
                }
                Some(i) if l.same_in(&rows[i], &probe, &given_cols(&given)) => unchanged += 1,
                Some(i) if a.replace => {
                    let merged = merged_row(&l, &rows[i], &given)?;
                    rows[i] = merged;
                    replaced += 1;
                }
                Some(i) => bad.push(format!(
                    "{}:{line}: the key {} is at line {} of the ledger with different values; --replace replaces it",
                    a.csv,
                    l.key_text(&probe),
                    raw.rows[i].0
                )),
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
    render_after_write(ctx);
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
    let (table, total) = match query::run(&con, &a.statement, limit, None, None) {
        Ok(x) => x,
        Err(query::QueryErr::Refused(m)) | Err(query::QueryErr::Stopped(m)) => die!("{m}"),
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
    if !total.exact || total.n > table.rows.len() {
        let note = format!("showing {} of {} rows; --limit shows more", table.rows.len(), total.text());
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
    // One time budget for the whole command, shared by every block of every page.
    let deadline = query::Deadline::new();
    for rel in pages {
        let o = pages::render_page(ctx, reg, rel, &deadline);
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
        for w in pages::skipped_entries(&reg) {
            out::err_line(&format!("af: warning: {w}"));
        }
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
    let reg = match Registry::load(ctx) {
        Ok(Some(r)) => r,
        Ok(None) => return Ok((issues, notes)),
        Err(crate::error::AfError::Refusal(why)) => {
            issues.push(why);
            return Ok((issues, notes));
        }
        Err(e) => return Err(e),
    };
    for (raw, why) in &reg.bad_pages {
        issues.push(format!("{}: pages entry {raw:?} {why}; remove it from ledgers.yaml", ledger::REGISTRY_REL));
    }
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
