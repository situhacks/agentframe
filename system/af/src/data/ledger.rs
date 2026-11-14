//! The ledger registry (`workspace/life/ledgers.yaml`), the column types and their checks, and
//! reading and writing one ledger's CSV file. Shapes: `library/schema/ledger.md`.

use crate::core::Ctx;
use crate::die;
use crate::error::{refuse, AfError, R};
use crate::life::{vpath, TextFile};
use crate::pyos;
use crate::pyyaml::{safe_load, PyVal};
use std::collections::HashMap;

pub const LIFE_REL: &str = "workspace/life";
pub const REGISTRY_REL: &str = "workspace/life/ledgers.yaml";
/// How many faults one ledger or input prints before a count.
pub const FAULTS_SHOWN: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Date,
    Integer,
    Number,
    Text,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Date => "date",
            Ty::Integer => "integer",
            Ty::Number => "number",
            Ty::Text => "text",
        }
    }

    pub fn parse(s: &str) -> Option<Ty> {
        match s {
            "date" => Some(Ty::Date),
            "integer" => Some(Ty::Integer),
            "number" => Some(Ty::Number),
            "text" => Some(Ty::Text),
            _ => None,
        }
    }

    /// The SQLite column type a ledger column is loaded as.
    pub fn sql(self) -> &'static str {
        match self {
            Ty::Date | Ty::Text => "TEXT",
            Ty::Integer => "INTEGER",
            Ty::Number => "REAL",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Col {
    pub name: String,
    pub ty: Ty,
    pub required: bool,
}

impl Col {
    /// `date:date!`, as `af data new --column` and `af data list` write it.
    pub fn spec(&self) -> String {
        format!("{}:{}{}", self.name, self.ty.name(), if self.required { "!" } else { "" })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    pub name: String,
    /// Relative to `workspace/life/`, forward slashes.
    pub file: String,
    pub key: Vec<String>,
    pub columns: Vec<Col>,
}

impl Ledger {
    pub fn path(&self, ctx: &Ctx) -> String {
        vpath(ctx, &format!("{LIFE_REL}/{}", self.file))
    }

    /// The file as the faults name it: from the vault root.
    pub fn label(&self) -> String {
        format!("{LIFE_REL}/{}", self.file)
    }

    pub fn col_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    pub fn key_of(&self, row: &[String]) -> Vec<String> {
        self.key.iter().filter_map(|k| self.col_index(k)).map(|i| row.get(i).cloned().unwrap_or_default()).collect()
    }

    pub fn key_text(&self, row: &[String]) -> String {
        let parts: Vec<String> = self.key.iter().zip(self.key_of(row)).map(|(k, v)| format!("{k}={v}")).collect();
        format!("({})", parts.join(", "))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub ledgers: Vec<Ledger>,
    /// Relative to `workspace/life/`, forward slashes.
    pub pages: Vec<String>,
}

// ------------------------------------------------------------------ names, paths, values

/// Lower-case letters, digits and underscores, starting with a letter.
pub fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A ledger name has to work bare in `select * from <name>`, so a keyword (`group`, `select`) or a
/// reserved `sqlite_` name is refused. Column names are quoted wherever af uses them, so any valid
/// name serves a column.
pub fn bare_table_name_ok(name: &str) -> bool {
    let Ok(con) = rusqlite::Connection::open_in_memory() else { return false };
    con.execute_batch(&format!("create table {} (x)", quote_ident(name))).is_ok() && con.prepare(&format!("select * from {name}")).is_ok()
}

/// A path under `workspace/life/` as forward-slash components: not empty, not absolute, no `.`, `..`,
/// drive colon or empty component.
fn clean_rel(arg: &str) -> Option<String> {
    let s = arg.replace('\\', "/");
    if s.is_empty() || s.starts_with('/') || pyos::isabs(arg) {
        return None;
    }
    let mut parts = Vec::new();
    for p in s.split('/') {
        if p.is_empty() || p == "." || p == ".." || p.contains(':') || p.ends_with('.') || p.ends_with(' ') {
            return None;
        }
        parts.push(p);
    }
    Some(parts.join("/"))
}

/// A ledger's `--file`: relative to `workspace/life/`, staying inside it, ending in `.csv`.
pub fn ledger_file_rel(arg: &str) -> R<String> {
    match clean_rel(arg) {
        Some(r) if r.to_ascii_lowercase().ends_with(".csv") && r.len() > 4 => Ok(r),
        Some(_) => die!("--file {arg:?} must end in .csv"),
        None => die!("--file {arg:?} must be a path inside workspace/life/ (relative, no `..`, no drive)"),
    }
}

/// A page argument: relative to `workspace/life/` (a leading `workspace/life/` is accepted too), or an
/// absolute path inside it. Anything that leaves the folder is refused.
pub fn page_rel(ctx: &Ctx, arg: &str) -> R<String> {
    let life = vpath(ctx, LIFE_REL);
    let rel = if pyos::isabs(arg) {
        match pyos::relpath(arg, &life) {
            Ok(r) if r != "." && !r.split(['/', '\\']).any(|p| p == "..") && !pyos::isabs(&r) => Some(r.replace('\\', "/")),
            _ => None,
        }
    } else {
        let s = arg.replace('\\', "/");
        let s = s.trim_start_matches("./");
        clean_rel(s.strip_prefix("workspace/life/").unwrap_or(s))
    };
    match rel.and_then(|r| clean_rel(&r)) {
        Some(r) => Ok(r),
        None => die!("{arg:?} is not inside workspace/life/; pages live there"),
    }
}

/// Why a value does not fit its column type, or Ok.
pub fn check_value(ty: Ty, v: &str) -> Result<(), String> {
    match ty {
        Ty::Date => {
            let b = v.as_bytes();
            let shape = b.len() == 10 && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
            let real = shape
                && chrono::NaiveDate::from_ymd_opt(v[0..4].parse().unwrap_or(0), v[5..7].parse().unwrap_or(0), v[8..10].parse().unwrap_or(0)).is_some();
            if real {
                Ok(())
            } else {
                Err(format!("{v:?} is not a real date as YYYY-MM-DD"))
            }
        }
        Ty::Integer => {
            let digits = v.strip_prefix('-').unwrap_or(v);
            if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) && v.parse::<i64>().is_ok() {
                Ok(())
            } else {
                Err(format!("{v:?} is not an integer (digits, optional leading minus)"))
            }
        }
        Ty::Number => {
            let body = v.strip_prefix('-').unwrap_or(v);
            let (whole, frac) = match body.split_once('.') {
                Some((w, f)) => (w, Some(f)),
                None => (body, None),
            };
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
            if digits(whole) && frac.is_none_or(digits) && v.parse::<f64>().is_ok_and(f64::is_finite) {
                Ok(())
            } else {
                Err(format!("{v:?} is not a number (a plain decimal, optional leading minus, no thousands separator)"))
            }
        }
        Ty::Text => {
            if v.contains(['\n', '\r']) {
                Err("text holds a line break".to_string())
            } else {
                Ok(())
            }
        }
    }
}

/// `date:date!` into a column; the `!` marks it required.
pub fn parse_col_spec(spec: &str) -> R<Col> {
    let Some((name, ty)) = spec.split_once(':') else { die!("column {spec:?} must be name:type (date, integer, number, text), with ! after the type when required") };
    let (ty, required) = match ty.strip_suffix('!') {
        Some(t) => (t, true),
        None => (ty, false),
    };
    if !valid_name(name) {
        die!("column name {name:?} must be lower-case letters, digits and underscores, starting with a letter");
    }
    let Some(ty) = Ty::parse(ty) else { die!("column {name}: type {ty:?} is not one of date, integer, number, text") };
    Ok(Col { name: name.to_string(), ty, required })
}

// ------------------------------------------------------------------ the registry file

fn yq(s: &str) -> String {
    let bare = crate::fm::rx(r"\A[A-Za-z0-9_][A-Za-z0-9_./-]*\z").is_match(s)
        && !matches!(s.to_ascii_lowercase().as_str(), "y" | "n" | "no" | "on" | "off" | "yes" | "true" | "false" | "null")
        && s.parse::<f64>().is_err();
    if bare {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn s_of(v: Option<&PyVal>) -> String {
    match v {
        None | Some(PyVal::None) => String::new(),
        Some(x) => pyos::py_strip(&x.py_str()).to_string(),
    }
}

impl Registry {
    pub fn path(ctx: &Ctx) -> String {
        vpath(ctx, REGISTRY_REL)
    }

    /// None when there is no registry yet; a refusal when it does not parse.
    pub fn load(ctx: &Ctx) -> R<Option<Registry>> {
        match TextFile::load(&Registry::path(ctx))? {
            Some(f) => Registry::parse(&f.text).map(Some),
            None => Ok(None),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Ledger> {
        self.ledgers.iter().find(|l| l.name == name)
    }

    pub fn names(&self) -> String {
        if self.ledgers.is_empty() {
            "none registered".to_string()
        } else {
            self.ledgers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ")
        }
    }

    pub fn parse(text: &str) -> R<Registry> {
        let doc = match safe_load(text, REGISTRY_REL, false) {
            Ok(d) => d,
            Err(e) => die!("{REGISTRY_REL} does not parse: {}", e.lines().next().unwrap_or("")),
        };
        let mut reg = Registry::default();
        if let Some(PyVal::List(items)) = doc.get("ledgers") {
            for item in items {
                let name = s_of(item.get("name"));
                if !valid_name(&name) {
                    die!("{REGISTRY_REL}: ledger name {name:?} must be lower-case letters, digits and underscores, starting with a letter");
                }
                if reg.get(&name).is_some() {
                    die!("{REGISTRY_REL}: ledger '{name}' is listed twice");
                }
                let file_raw = s_of(item.get("file"));
                let file = match clean_rel(&file_raw) {
                    Some(f) if f.to_ascii_lowercase().ends_with(".csv") => f,
                    _ => die!("{REGISTRY_REL}: ledger '{name}' file {file_raw:?} must be a .csv path inside workspace/life/"),
                };
                let mut columns: Vec<Col> = Vec::new();
                if let Some(PyVal::List(cols)) = item.get("columns") {
                    for c in cols {
                        let cname = s_of(c.get("name"));
                        let tname = s_of(c.get("type"));
                        if !valid_name(&cname) {
                            die!("{REGISTRY_REL}: ledger '{name}' column name {cname:?} is not a valid name");
                        }
                        let Some(ty) = Ty::parse(&tname) else { die!("{REGISTRY_REL}: ledger '{name}' column '{cname}' type {tname:?} is not one of date, integer, number, text") };
                        if columns.iter().any(|x| x.name == cname) {
                            die!("{REGISTRY_REL}: ledger '{name}' lists column '{cname}' twice");
                        }
                        let required = matches!(c.get("required"), Some(PyVal::Bool(true)));
                        columns.push(Col { name: cname, ty, required });
                    }
                }
                if columns.is_empty() {
                    die!("{REGISTRY_REL}: ledger '{name}' has no columns");
                }
                let key: Vec<String> = match item.get("key") {
                    Some(PyVal::List(ks)) => ks.iter().map(|k| s_of(Some(k))).collect(),
                    Some(PyVal::Str(s)) => s.split(',').map(|k| k.trim().to_string()).collect(),
                    _ => Vec::new(),
                };
                if key.is_empty() {
                    die!("{REGISTRY_REL}: ledger '{name}' has no key");
                }
                for k in &key {
                    match columns.iter().find(|c| &c.name == k) {
                        None => die!("{REGISTRY_REL}: ledger '{name}' key column '{k}' is not one of its columns"),
                        Some(c) if !c.required => die!("{REGISTRY_REL}: ledger '{name}' key column '{k}' must be required"),
                        _ => {}
                    }
                }
                reg.ledgers.push(Ledger { name, file, key, columns });
            }
        }
        if let Some(PyVal::List(items)) = doc.get("pages") {
            for p in items {
                let raw = s_of(Some(p));
                match clean_rel(&raw) {
                    Some(r) => {
                        if !reg.pages.contains(&r) {
                            reg.pages.push(r)
                        }
                    }
                    None => die!("{REGISTRY_REL}: page {raw:?} must be a path inside workspace/life/"),
                }
            }
        }
        Ok(reg)
    }

    pub fn render_yaml(&self) -> String {
        let mut s = String::from("# Written by `af data`; change it through af data new, column and page. Shapes: library/schema/ledger.md\n");
        if self.ledgers.is_empty() {
            s.push_str("ledgers: []\n");
        } else {
            s.push_str("ledgers:\n");
        }
        for l in &self.ledgers {
            s.push_str(&format!("  - name: {}\n    file: {}\n", yq(&l.name), yq(&l.file)));
            let keys: Vec<String> = l.key.iter().map(|k| yq(k)).collect();
            s.push_str(&format!("    key: [{}]\n    columns:\n", keys.join(", ")));
            for c in &l.columns {
                let req = if c.required { ", required: true" } else { "" };
                s.push_str(&format!("      - {{name: {}, type: {}{req}}}\n", yq(&c.name), c.ty.name()));
            }
        }
        if self.pages.is_empty() {
            s.push_str("pages: []\n");
        } else {
            s.push_str("pages:\n");
            for p in &self.pages {
                s.push_str(&format!("  - {}\n", yq(p)));
            }
        }
        s
    }

    pub fn save(&self, ctx: &Ctx) -> R<()> {
        let path = Registry::path(ctx);
        let mut file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
        file.text = self.render_yaml();
        file.save_atomic(&path)
    }
}

// ------------------------------------------------------------------ reading a CSV

/// What a CSV file holds before any check: the records with their starting line numbers, and the
/// file's habits (line ending, BOM, a final newline) so a write can keep them.
#[derive(Clone, Debug, Default)]
pub struct Raw {
    /// The first record and its line.
    pub header: Option<(u64, Vec<String>)>,
    pub rows: Vec<(u64, Vec<String>)>,
    pub crlf: bool,
    pub bom: bool,
    pub ends_with_newline: bool,
    pub empty: bool,
}

/// Parse CSV bytes into records, each with its starting line. Err is (line, what).
pub fn parse_csv(bytes: &[u8]) -> Result<Vec<(u64, Vec<String>)>, (u64, String)> {
    let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(bytes);
    let mut out = Vec::new();
    for rec in rdr.byte_records() {
        match rec {
            Ok(r) => {
                let line = r.position().map(|p| p.line()).unwrap_or(0);
                let mut fields = Vec::with_capacity(r.len());
                for f in r.iter() {
                    match std::str::from_utf8(f) {
                        Ok(s) => fields.push(s.to_string()),
                        Err(_) => return Err((line, "is not UTF-8 text".to_string())),
                    }
                }
                out.push((line, fields));
            }
            Err(e) => {
                let line = e.position().map(|p| p.line()).unwrap_or(0);
                return Err((line, format!("is not readable CSV: {e}")));
            }
        }
    }
    Ok(out)
}

/// Read a CSV file: Err is the fault text for the whole file (`label:line: what`).
pub fn read_raw(path: &str, label: &str) -> Result<Raw, String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(format!("{label}: the file is missing")),
        Err(e) => return Err(format!("{label}: cannot be read: {e}")),
    };
    let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let body = if bom { &bytes[3..] } else { &bytes[..] };
    let mut records = parse_csv(body).map_err(|(line, what)| format!("{label}:{line}: {what}"))?;
    let header = if records.is_empty() { None } else { Some(records.remove(0)) };
    Ok(Raw {
        header,
        rows: records,
        crlf: body.windows(2).any(|w| w == b"\r\n"),
        bom,
        ends_with_newline: body.last().is_some_and(|b| *b == b'\n' || *b == b'\r'),
        empty: body.is_empty(),
    })
}

/// The faults in a ledger file against its definition, in file order, each `label:line: what`: a
/// wrong header, a row that fails a check, a repeated key.
pub fn check(ledger: &Ledger, label: &str, raw: &Raw) -> Vec<String> {
    let mut faults = Vec::new();
    let want: Vec<&str> = ledger.columns.iter().map(|c| c.name.as_str()).collect();
    let Some((hline, header)) = &raw.header else {
        faults.push(format!("{label}:1: the file is empty; it needs the header row {}", want.join(",")));
        return faults;
    };
    if header.iter().map(String::as_str).ne(want.iter().copied()) {
        faults.push(format!("{label}:{hline}: the header is {} but the registry has {}", header.join(","), want.join(",")));
        return faults;
    }
    let mut seen: HashMap<Vec<String>, u64> = HashMap::new();
    for (line, row) in &raw.rows {
        if row.len() != ledger.columns.len() {
            faults.push(format!("{label}:{line}: has {} fields, the header has {}", row.len(), ledger.columns.len()));
            continue;
        }
        let mut bad = false;
        for (c, v) in ledger.columns.iter().zip(row) {
            if v.is_empty() {
                if c.required {
                    faults.push(format!("{label}:{line}: {} is required and empty", c.name));
                    bad = true;
                }
            } else if let Err(e) = check_value(c.ty, v) {
                faults.push(format!("{label}:{line}: {}: {e}", c.name));
                bad = true;
            }
        }
        if bad {
            continue;
        }
        let key = ledger.key_of(row);
        match seen.get(&key) {
            Some(first) => faults.push(format!("{label}:{line}: the key {} repeats line {first}", ledger.key_text(row))),
            None => {
                seen.insert(key, *line);
            }
        }
    }
    faults
}

/// A ledger file read and checked: its raw content and its faults (a missing or unreadable file is one fault).
pub fn load_checked(ctx: &Ctx, ledger: &Ledger) -> Result<(Raw, Vec<String>), Vec<String>> {
    let label = ledger.label();
    match read_raw(&ledger.path(ctx), &label) {
        Ok(raw) => {
            let faults = check(ledger, &label, &raw);
            Ok((raw, faults))
        }
        Err(e) => Err(vec![e]),
    }
}

/// The first faults, then a count of the rest.
pub fn shown(faults: &[String]) -> Vec<String> {
    let mut v: Vec<String> = faults.iter().take(FAULTS_SHOWN).cloned().collect();
    if faults.len() > FAULTS_SHOWN {
        v.push(format!("... and {} more", faults.len() - FAULTS_SHOWN));
    }
    v
}

/// Print the first faults to stderr and refuse with `summary`.
pub fn refuse_faults(faults: &[String], summary: &str) -> AfError {
    for f in shown(faults) {
        crate::out::err_line(&f);
    }
    refuse(summary)
}

// ------------------------------------------------------------------ writing a CSV

/// Records as CSV text: RFC 4180 quoting only where a value needs it, the given line ending.
pub fn encode(records: &[Vec<String>], crlf: bool) -> Vec<u8> {
    let mut w = csv::WriterBuilder::new().has_headers(false).terminator(csv::Terminator::Any(b'\n')).from_writer(Vec::new());
    for r in records {
        let _ = w.write_record(r);
    }
    let bytes = w.into_inner().unwrap_or_default();
    if crlf {
        // No field holds a line break (text is checked), so every `\n` ends a record.
        String::from_utf8_lossy(&bytes).replace('\n', "\r\n").into_bytes()
    } else {
        bytes
    }
}

/// Append records to the end of a file. The lines already there are not touched.
pub fn append_records(path: &str, raw: &Raw, records: &[Vec<String>]) -> R<()> {
    use std::io::Write;
    let eol = if raw.crlf { "\r\n" } else { "\n" };
    let mut data = Vec::new();
    if !raw.empty && !raw.ends_with_newline {
        data.extend_from_slice(eol.as_bytes());
    }
    data.extend(encode(records, raw.crlf));
    let mut f = std::fs::OpenOptions::new().append(true).open(path).map_err(|e| pyos::os_error(&e, path))?;
    f.write_all(&data).and_then(|_| f.flush()).map_err(|e| pyos::os_error(&e, path))
}

/// Replace the whole file with these records (the header first), through a temp file and a rename,
/// in the file's own line ending and BOM.
pub fn rewrite(path: &str, raw: &Raw, records: &[Vec<String>]) -> R<()> {
    let mut bytes = Vec::new();
    if raw.bom {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend(encode(records, raw.crlf));
    let text = String::from_utf8_lossy(&bytes).into_owned();
    TextFile { text, crlf: false, bom: false }.save_atomic(path)
}
