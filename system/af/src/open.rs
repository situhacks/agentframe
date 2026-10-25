//! `workspace/life/open.md`: the open-loop grammar and `af open add | close | due`.
//!
//! `- [ ] 2026-10-24 · text · nudge 2026-10-27 · from: src-2026-10-23-02`
//! A loop is closed by moving it to the month log; nothing closed stays in open.md.

use crate::core::{today_date, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::rx;
use crate::life::{self, vpath, TextFile};
use crate::out;
use crate::outln;
use chrono::NaiveDate;
use clap::Subcommand;
use serde_json::{json, Value};

pub const OPEN_REL: &str = "workspace/life/open.md";
pub const DEFAULT_SECTION: &str = "Waiting on the operator";
pub const DEFAULT_FROM: &str = "main";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loop {
    /// 1-based ordinal among the open loops, in file order (what `af open close <n>` takes).
    pub n: usize,
    pub line_idx: usize,
    pub section: String,
    pub opened: NaiveDate,
    pub text: String,
    pub nudge: Option<NaiveDate>,
    pub from: String,
    pub raw: String,
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// (opened, text, nudge, from) of an open loop line; None when it does not parse.
pub fn parse_open(line: &str) -> Option<(NaiveDate, String, Option<NaiveDate>, String)> {
    let re = rx(r"\A- \[ \] (\d{4}-\d{2}-\d{2}) · (.+?)(?: · nudge (\d{4}-\d{2}-\d{2}))? · from: (\S(?:.*\S)?)\z");
    let c = re.captures(line.trim_end())?;
    let opened = date(&c[1])?;
    let nudge = match c.get(3) {
        Some(m) => Some(date(m.as_str())?),
        None => None,
    };
    Some((opened, c[2].trim().to_string(), nudge, c[4].to_string()))
}

pub fn format_open(opened: NaiveDate, text: &str, nudge: Option<NaiveDate>, from: &str) -> String {
    let mut s = format!("- [ ] {} · {text}", opened.format("%Y-%m-%d"));
    if let Some(n) = nudge {
        s.push_str(&format!(" · nudge {}", n.format("%Y-%m-%d")));
    }
    s.push_str(&format!(" · from: {from}"));
    s
}

fn heading(line: &str) -> Option<(usize, String)> {
    let c = rx(r"\A(#{1,6})\s+(.*?)\s*\z").captures(line)?;
    Some((c[1].len(), c[2].to_string()))
}

pub struct OpenDoc {
    pub path: String,
    pub file: TextFile,
    pub lines: Vec<String>,
    pub loops: Vec<Loop>,
    /// (1-based line number, line, reason) for list lines that break the grammar.
    pub bad: Vec<(usize, String, String)>,
}

impl OpenDoc {
    pub fn load(ctx: &Ctx) -> R<Option<OpenDoc>> {
        let path = vpath(ctx, OPEN_REL);
        Ok(TextFile::load(&path)?.map(|file| OpenDoc::parse(path, file)))
    }

    pub fn parse(path: String, file: TextFile) -> OpenDoc {
        let lines = file.lines();
        let mut loops = Vec::new();
        let mut bad = Vec::new();
        let mut section = String::new();
        let mut in_comment = false;
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if in_comment {
                in_comment = !t.contains("-->");
                continue;
            }
            if t.starts_with("<!--") {
                in_comment = !t.contains("-->");
                continue;
            }
            if let Some((_, h)) = heading(t) {
                section = h;
                continue;
            }
            if !(t.starts_with("- ") || t.starts_with("* ") || t == "-" || t == "*") {
                continue;
            }
            match parse_open(line) {
                Some((opened, text, nudge, from)) => loops.push(Loop {
                    n: loops.len() + 1,
                    line_idx: i,
                    section: section.clone(),
                    opened,
                    text,
                    nudge,
                    from,
                    raw: line.trim_end().to_string(),
                }),
                None => {
                    let why = if t.starts_with("- [x]") || t.starts_with("- [X]") {
                        "closed loop left in open.md (af open close moves it to the month log)"
                    } else {
                        "line does not parse as '- [ ] YYYY-MM-DD · text[ · nudge YYYY-MM-DD] · from: <id>'"
                    };
                    bad.push((i + 1, t.to_string(), why.to_string()));
                }
            }
        }
        OpenDoc { path, file, lines, loops, bad }
    }

    /// Loops whose nudge is today or earlier, oldest first, with their age in days.
    pub fn due(&self, today: NaiveDate) -> Vec<(Loop, i64)> {
        let mut v: Vec<(Loop, i64)> = self
            .loops
            .iter()
            .filter(|l| matches!(l.nudge, Some(n) if n <= today))
            .map(|l| (l.clone(), (today - l.opened).num_days()))
            .collect();
        v.sort_by(|a, b| a.0.opened.cmp(&b.0.opened).then(a.0.n.cmp(&b.0.n)));
        v
    }

    /// Open-loop counts per section, in first-seen order.
    pub fn by_section(&self) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        for l in &self.loops {
            match out.iter_mut().find(|(s, _)| *s == l.section) {
                Some(e) => e.1 += 1,
                None => out.push((l.section.clone(), 1)),
            }
        }
        out
    }

    fn save(&mut self) -> R<()> {
        self.file.set_lines(&self.lines);
        self.file.save(&self.path)
    }
}

pub fn due_line(l: &Loop, age: i64) -> String {
    format!("{}  ({age}d, #{})", l.raw, l.n)
}

pub fn loop_json(l: &Loop, age: Option<i64>) -> Value {
    let mut v = json!({
        "n": l.n, "section": l.section, "opened": l.opened.format("%Y-%m-%d").to_string(), "text": l.text,
        "nudge": l.nudge.map(|n| n.format("%Y-%m-%d").to_string()), "from": l.from,
    });
    if let Some(a) = age {
        v["age_days"] = json!(a);
    }
    v
}

// ------------------------------------------------------------------ verbs

#[derive(Subcommand, Debug)]
pub enum OpenCmd {
    /// Append an open loop under a section (default "Waiting on the operator")
    Add {
        text: String,
        /// a date (YYYY-MM-DD) or +Nd from today
        #[arg(long)]
        nudge: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        section: Option<String>,
    },
    /// Close a loop by number or text prefix and move it to the month log
    Close {
        target: String,
        #[arg(long)]
        outcome: String,
    },
    /// Change a loop's nudge date in place; its opened date, text, and source stay
    Redate {
        target: String,
        /// a date (YYYY-MM-DD) or +Nd from today
        #[arg(long)]
        nudge: String,
    },
    /// Loops whose nudge date is today or earlier, oldest first
    Due {
        #[arg(long)]
        json: bool,
    },
}

pub fn run(ctx: &Ctx, cmd: OpenCmd) -> R<i32> {
    match cmd {
        OpenCmd::Add { text, nudge, from, section } => {
            cmd_add(ctx, &text, nudge.as_deref(), from.as_deref(), section.as_deref())
        }
        OpenCmd::Close { target, outcome } => cmd_close(ctx, &target, &outcome),
        OpenCmd::Redate { target, nudge } => cmd_redate(ctx, &target, &nudge),
        OpenCmd::Due { json } => cmd_due(ctx, json),
    }
}

pub fn parse_nudge(v: &str, today: NaiveDate) -> Option<NaiveDate> {
    if let Some(c) = rx(r"\A\+(\d{1,4})d\z").captures(v.trim()) {
        return today.checked_add_signed(chrono::Duration::days(c[1].parse().ok()?));
    }
    date(v.trim())
}

pub fn cmd_add(ctx: &Ctx, text: &str, nudge: Option<&str>, from: Option<&str>, section: Option<&str>) -> R<i32> {
    let today = today_date();
    let text = life::one_line(text);
    if text.is_empty() {
        die!("an open loop needs text");
    }
    let nudge = match nudge {
        Some(v) => match parse_nudge(v, today) {
            Some(d) => Some(d),
            None => die!("--nudge takes YYYY-MM-DD or +Nd, got '{v}'"),
        },
        None => None,
    };
    let from = life::one_line(from.unwrap_or(DEFAULT_FROM));
    let section = life::one_line(section.unwrap_or(DEFAULT_SECTION));
    if section.is_empty() || from.is_empty() {
        die!("--section and --from may not be empty");
    }
    let line = format_open(today, &text, nudge, &from);
    match parse_open(&line) {
        Some((_, t, n, f)) if t == text && n == nudge && f == from => {}
        _ => die!("the loop text breaks the open.md grammar (it may not contain ' · nudge ' or ' · from: '): {text}"),
    }
    let path = vpath(ctx, OPEN_REL);
    let file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
    let mut doc = OpenDoc::parse(path, file);
    let lines = &mut doc.lines;
    let h = lines.iter().position(|l| heading(l.trim()).map(|(lv, t)| lv >= 2 && t == section).unwrap_or(false));
    match h {
        Some(h) => {
            let level = heading(lines[h].trim()).map(|(lv, _)| lv).unwrap_or(2);
            let end = (h + 1..lines.len())
                .find(|&k| heading(lines[k].trim()).map(|(lv, _)| lv <= level).unwrap_or(false))
                .unwrap_or(lines.len());
            match (h + 1..end).rev().find(|&k| !lines[k].trim().is_empty()) {
                Some(last) => lines.insert(last + 1, line.clone()),
                None => {
                    lines.insert(h + 1, String::new());
                    lines.insert(h + 2, line.clone());
                }
            }
        }
        None => {
            if lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            lines.push(format!("## {section}"));
            lines.push(String::new());
            lines.push(line.clone());
        }
    }
    doc.save()?;
    outln!("{line}");
    Ok(0)
}

/// The one open loop `target` names: its number, or a text prefix (an exact match wins
/// over longer loops that share the prefix).
fn find_target(doc: &OpenDoc, target: &str) -> R<Loop> {
    let t = target.trim();
    let hit: Loop = if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = t.parse().unwrap_or(0);
        match doc.loops.iter().find(|l| l.n == n) {
            Some(l) => l.clone(),
            None => die!("no open loop #{t} ({} open)", doc.loops.len()),
        }
    } else {
        let p = t.to_lowercase();
        if p.is_empty() {
            die!("give a loop number or a text prefix");
        }
        let cands: Vec<&Loop> = doc.loops.iter().filter(|l| l.text.to_lowercase().starts_with(&p)).collect();
        let exact: Vec<&&Loop> = cands.iter().filter(|l| l.text.to_lowercase() == p).collect();
        match (cands.len(), exact.len()) {
            (0, _) => die!("no open loop starts with '{t}'"),
            (1, _) => cands[0].clone(),
            (_, 1) => (*exact[0]).clone(),
            (k, _) => {
                for l in &cands {
                    outln!("  #{} {}", l.n, l.raw);
                }
                die!("ambiguous prefix '{t}' matches {k} open loops; give the number or a longer prefix")
            }
        }
    };
    Ok(hit)
}

pub fn cmd_close(ctx: &Ctx, target: &str, outcome: &str) -> R<i32> {
    let outcome = life::one_line(outcome);
    if outcome.is_empty() {
        die!("--outcome needs the result, in a few words");
    }
    let mut doc = match OpenDoc::load(ctx)? {
        Some(d) => d,
        None => die!("{OPEN_REL} not found"),
    };
    let hit = find_target(&doc, target)?;
    // Log first: a failure after the append leaves the loop open, never lost.
    let rel = life::append_log(ctx, "closed", &format!("{} → {outcome}", hit.text))?;
    doc.lines.remove(hit.line_idx);
    doc.save()?;
    outln!("af open close: #{} {} → {outcome} (moved to {rel})", hit.n, hit.text);
    Ok(0)
}

/// `af open redate <target> --nudge <date|+Nd>`: the loop keeps its opened date, text,
/// section, and source; only the nudge changes. The month log records the re-date.
pub fn cmd_redate(ctx: &Ctx, target: &str, nudge: &str) -> R<i32> {
    let today = crate::core::today_date();
    let new = match parse_nudge(nudge, today) {
        Some(d) => d,
        None => die!("--nudge needs a date (YYYY-MM-DD) or +Nd, not '{nudge}'"),
    };
    let mut doc = match OpenDoc::load(ctx)? {
        Some(d) => d,
        None => die!("{OPEN_REL} not found"),
    };
    let hit = find_target(&doc, target)?;
    let was = hit.nudge.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "none".to_string());
    let line = format_open(hit.opened, &hit.text, Some(new), &hit.from);
    doc.lines[hit.line_idx] = line.clone();
    doc.save()?;
    let rel = life::append_log(ctx, "redated", &format!("{} → nudge {} (was {was})", hit.text, new.format("%Y-%m-%d")))?;
    outln!("af open redate: #{} {} → nudge {} (was {was}; logged in {rel})", hit.n, hit.text, new.format("%Y-%m-%d"));
    Ok(0)
}

pub fn cmd_due(ctx: &Ctx, as_json: bool) -> R<i32> {
    let due = match OpenDoc::load(ctx) {
        Ok(Some(d)) => d.due(today_date()),
        _ => Vec::new(),
    };
    if as_json {
        let arr: Vec<Value> = due.iter().map(|(l, a)| loop_json(l, Some(*a))).collect();
        out::line(&serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_default());
        return Ok(0);
    }
    for (l, age) in &due {
        out::line(&due_line(l, *age));
    }
    Ok(0)
}
