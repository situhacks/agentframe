//! Pages filled from queries. A block in a markdown page under `workspace/life/`:
//!
//! ```text
//! <!-- af-data: select date, weight from sets order by date desc limit 10 -->
//! (generated table)
//! <!-- /af-data -->
//! ```
//!
//! Rendering replaces what is between the two markers with the query's result and touches no other
//! byte of the page: the page is read and written as bytes-in-a-string, so its line endings stay.

use super::ledger::{Registry, LIFE_REL};
use super::query::{self, Deadline, QueryErr};
use crate::core::Ctx;
use crate::life::{locked, vpath, TextFile};
use regex::Regex;
use rusqlite::Connection;

/// One block: the line of its opening marker, its statement, and the byte range between the markers
/// that rendering replaces.
#[derive(Clone, Debug)]
pub struct Block {
    pub line: usize,
    pub sql: String,
    pub start: usize,
    pub end: usize,
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// The byte ranges of fenced code blocks (``` or ~~~), which a page may use to show an example of a
/// block. An unclosed fence runs to the end of the page, as in markdown.
fn fenced_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, usize)> = None; // (fence character, its length, where it began)
    let mut off = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() <= 3 {
            if let Some(ch) = trimmed.chars().next().filter(|c| *c == '`' || *c == '~') {
                let n = trimmed.chars().take_while(|c| *c == ch).count();
                let rest = &trimmed[n..];
                if n >= 3 {
                    match open {
                        None if ch == '~' || !rest.contains('`') => open = Some((ch, n, off)),
                        Some((c, len, start)) if c == ch && n >= len && rest.trim().is_empty() => {
                            out.push(start..off + line.len());
                            open = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        off += line.len();
    }
    if let Some((_, _, start)) = open {
        out.push(start..text.len());
    }
    out
}

/// The blocks of a page, in order. A fault is `(line, what)` and means the page is not touched: an
/// opening marker that is not ended by `-->` before a closing marker (a typo), or that has no closing
/// marker before the next opening marker or the end of the page. A closing marker alone is ignored, and
/// so is anything inside a fenced code block.
pub fn find_blocks(text: &str) -> Result<Vec<Block>, (usize, String)> {
    let open = Regex::new(r"(?s)<!--\s*af-data:(.*?)-->").expect("regex");
    let close = Regex::new(r"<!--\s*/af-data\s*-->").expect("regex");
    let fences = fenced_ranges(text);
    let fenced = |at: usize| fences.iter().any(|r| r.contains(&at));
    let opens: Vec<_> = open.captures_iter(text).filter_map(|c| c.get(0).map(|m| (m.range(), c[1].to_string()))).filter(|(r, _)| !fenced(r.start)).collect();
    let closes: Vec<_> = close.find_iter(text).map(|m| m.range()).filter(|r| !fenced(r.start)).collect();
    let inner = Regex::new(r"<!--\s*af-data:").expect("regex");
    for (r, sql) in &opens {
        if inner.is_match(sql) {
            return Err((line_of(text, r.start), "this opening marker is not ended by `-->` before the next opening marker; check it for a typo".to_string()));
        }
        if closes.iter().any(|c| c.start >= r.start && c.start < r.end) {
            return Err((line_of(text, r.start), "this opening marker is not ended by `-->` before a closing marker; check it for a typo".to_string()));
        }
    }
    // (offset, is_open, index into its list)
    let mut events: Vec<(usize, bool, usize)> = Vec::new();
    events.extend(opens.iter().enumerate().map(|(i, (r, _))| (r.start, true, i)));
    events.extend(closes.iter().enumerate().map(|(i, r)| (r.start, false, i)));
    events.sort();
    let mut blocks = Vec::new();
    let mut cur: Option<usize> = None;
    for (_, is_open, i) in events {
        if is_open {
            if let Some(prev) = cur {
                return Err((line_of(text, opens[prev].0.start), "this opening marker has no closing marker before the next opening marker".to_string()));
            }
            cur = Some(i);
        } else if let Some(o) = cur.take() {
            let (orange, sql) = &opens[o];
            let c = &closes[i];
            let line_start = text[..c.start].rfind('\n').map(|n| n + 1).unwrap_or(0);
            let own_line = line_start >= orange.end && text[line_start..c.start].chars().all(|ch| ch == ' ' || ch == '\t');
            let end = if own_line { line_start } else { c.start };
            if end < orange.end {
                return Err((line_of(text, orange.start), "this opening marker overlaps its closing marker".to_string()));
            }
            blocks.push(Block { line: line_of(text, orange.start), sql: sql.trim().to_string(), start: orange.end, end });
        }
    }
    if let Some(o) = cur {
        return Err((line_of(text, opens[o].0.start), "this opening marker has no closing marker".to_string()));
    }
    Ok(blocks)
}

/// The most rows a block keeps, and how far past that rows are counted for the note.
pub const BLOCK_ROWS: usize = 500;
pub const BLOCK_COUNT_CAP: usize = 100_000;

/// The page with every block refilled from `con`, and the faults (`line N: what`) of the blocks that
/// could not be: those keep their old content. A block keeps at most 500 rows and ends with one line
/// saying how many of how many are shown.
pub fn render_text(text: &str, blocks: &[Block], con: &Connection, deadline: &Deadline) -> (String, Vec<(usize, String)>) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut faults = Vec::new();
    for b in blocks {
        out.push_str(&text[at..b.start]);
        at = b.end;
        let fill = if b.sql.is_empty() {
            Err("the statement is empty".to_string())
        } else {
            match query::run(con, &b.sql, BLOCK_ROWS, Some(BLOCK_COUNT_CAP), Some(deadline)) {
                Ok((table, total)) => {
                    let mut md = query::to_markdown(&table);
                    if !total.exact || total.n > table.rows.len() {
                        md.push_str(&format!("\n\n(showing {} of {} rows)", table.rows.len(), total.text()));
                    }
                    Ok(format!("{eol}{}{eol}", md.replace('\n', eol)))
                }
                Err(QueryErr::Refused(m)) | Err(QueryErr::Stopped(m)) => Err(m),
                Err(QueryErr::Sql(m)) => Err(format!("SQL error: {m}")),
            }
        };
        match fill {
            Ok(f) => out.push_str(&f),
            Err(m) => {
                faults.push((b.line, m));
                out.push_str(&text[b.start..b.end]);
            }
        }
    }
    out.push_str(&text[at..]);
    (out, faults)
}

/// What rendering one page did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// `file:line: what`
    pub faults: Vec<String>,
    pub blocks: usize,
    pub changed: bool,
}

pub fn label(rel: &str) -> String {
    format!("{LIFE_REL}/{rel}")
}

/// Render one page (relative to `workspace/life/`) and write it when it changed, under a lock on the
/// page, so two renders never leave the older answer last. Nothing is written when the page is
/// missing, has a marker fault, or the ledgers fail their checks.
pub fn render_page(ctx: &Ctx, reg: &Registry, rel: &str, deadline: &Deadline) -> Outcome {
    let label = label(rel);
    let path = vpath(ctx, &label);
    let mut out = Outcome::default();
    if !crate::pyos::isfile(&path) {
        out.faults.push(format!("{label}: the page is missing"));
        return out;
    }
    let done = locked(&path, || {
        let bytes = std::fs::read(&path).map_err(|e| crate::pyos::os_error(&e, &path))?;
        let Ok(text) = String::from_utf8(bytes) else {
            out.faults.push(format!("{label}: is not UTF-8 text"));
            return Ok(());
        };
        let blocks = match find_blocks(&text) {
            Ok(b) => b,
            Err((line, what)) => {
                out.faults.push(format!("{label}:{line}: {what}; the page was not written"));
                return Ok(());
            }
        };
        out.blocks = blocks.len();
        if blocks.is_empty() {
            return Ok(());
        }
        let con = match query::load_db(ctx, reg) {
            Ok(c) => c,
            Err(faults) => {
                out.faults.push(format!("{label}: not rendered, a ledger file fails its checks:"));
                out.faults.extend(super::ledger::shown(&faults));
                return Ok(());
            }
        };
        let (new, faults) = render_text(&text, &blocks, &con, deadline);
        out.faults.extend(faults.into_iter().map(|(line, what)| format!("{label}:{line}: {what}")));
        if new != text {
            TextFile { text: new, crlf: false, bom: false }.save_atomic(&path)?;
            out.changed = true;
        }
        Ok(())
    });
    if let Err(e) = done {
        out.faults.push(format!("{label}: {e}"));
    }
    out
}

/// Every registered page, rendered. The faults of all of them.
pub fn render_registered(ctx: &Ctx) -> Vec<String> {
    let reg = match Registry::load(ctx) {
        Ok(Some(r)) => r,
        Ok(None) => return Vec::new(),
        Err(e) => return vec![e.to_string()],
    };
    // One time budget for every page: a registry full of slow blocks costs one limit, not one per block.
    let deadline = Deadline::new();
    let mut faults = skipped_entries(&reg);
    faults.extend(reg.pages.iter().flat_map(|p| render_page(ctx, &reg, p, &deadline).faults));
    faults
}

/// What to say about the `pages:` entries that cannot be pages: they are never opened.
pub fn skipped_entries(reg: &Registry) -> Vec<String> {
    reg.bad_pages.iter().map(|(raw, why)| format!("{}: pages entry {raw:?} {why}; skipped", super::ledger::REGISTRY_REL)).collect()
}

/// For the doctor: the notes about every registered page, and nothing written. A page that is
/// missing, a block that fails, a page whose content differs from what the ledgers give now. The
/// ledger files themselves are reported by the caller, so a failed load adds nothing here.
pub fn doctor_notes(ctx: &Ctx, reg: &Registry) -> Vec<String> {
    let mut notes = Vec::new();
    if reg.pages.is_empty() {
        return notes;
    }
    let con = query::load_db(ctx, reg).ok();
    let deadline = Deadline::new();
    for rel in &reg.pages {
        let label = label(rel);
        let path = vpath(ctx, &label);
        let text = match std::fs::read(&path) {
            Ok(b) => match String::from_utf8(b) {
                Ok(t) => t,
                Err(_) => {
                    notes.push(format!("{label}: is not UTF-8 text"));
                    continue;
                }
            },
            Err(_) => {
                notes.push(format!("{label}: the registered page is missing; remove it from ledgers.yaml or restore it"));
                continue;
            }
        };
        let blocks = match find_blocks(&text) {
            Ok(b) => b,
            Err((line, what)) => {
                notes.push(format!("{label}:{line}: {what}"));
                continue;
            }
        };
        let Some(con) = &con else { continue };
        let (new, faults) = render_text(&text, &blocks, con, &deadline);
        for (line, what) in &faults {
            notes.push(format!("{label}:{line}: the block fails: {what}"));
        }
        if faults.is_empty() && new != text {
            notes.push(format!("{label}: its rendered content differs from what the ledgers give now; run `af data render`"));
        }
    }
    notes
}
