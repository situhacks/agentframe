//! `workspace/life/open.md`: the open-loop grammar and `af open add | close | redate | due`.
//!
//! `- [ ] 2026-10-24 · text · nudge 2026-10-27 · every monthly · from: src-2026-10-23-02`
//! A loop is closed by moving it to the month log; nothing closed stays in open.md. A loop
//! with `every <period>` is logged as closed and re-dated one period on instead. A loop
//! with `need` and `detail` fields is an item on the needs list (`crate::needs`).

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
    /// The nearest heading.
    pub section: String,
    /// The enclosing `##` (or `#`) heading, however deep the nearest one is.
    pub top: String,
    pub opened: NaiveDate,
    pub text: String,
    pub nudge: Option<NaiveDate>,
    /// The recurrence period (`monthly`, `weekly`, or `Nd`); only a nudged loop carries one.
    pub every: Option<String>,
    pub from: String,
    /// Present on a needs-list item; such a loop carries no nudge.
    pub need: Option<crate::needs::Need>,
    pub raw: String,
}

impl Loop {
    /// The day it comes due: its nudge, or the date of a dated needs item.
    pub fn due_on(&self) -> Option<NaiveDate> {
        match &self.need {
            Some(crate::needs::Need { when: crate::needs::When::On(d), .. }) => Some(*d),
            _ => self.nudge,
        }
    }
}

/// One parsed loop line: (opened, text, nudge, every, from).
pub type Parsed = (NaiveDate, String, Option<NaiveDate>, Option<String>, String);

const PERIOD: &str = r"monthly|weekly|[1-9]\d{0,3}d";

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// The parts of an open loop line, its need fields already lifted off (`needs::split_need`);
/// None when it does not parse. Text carrying a ` · nudge `, ` · every `, ` · need `, or
/// ` · detail ` token is a misplaced or misspelt field, not text.
pub fn parse_open(line: &str) -> Option<Parsed> {
    let re = rx(&format!(
        r"\A- \[ \] (\d{{4}}-\d{{2}}-\d{{2}}) · (.+?)(?: · nudge (\d{{4}}-\d{{2}}-\d{{2}})(?: · every ({PERIOD}))?)? · from: (\S(?:.*\S)?)\z"
    ));
    let c = re.captures(line.trim_end())?;
    let opened = date(&c[1])?;
    let nudge = match c.get(3) {
        Some(m) => Some(date(m.as_str())?),
        None => None,
    };
    let text = c[2].trim().to_string();
    if [" · nudge ", " · every ", " · need ", " · detail "].iter().any(|t| text.contains(t)) {
        return None;
    }
    Some((opened, text, nudge, c.get(4).map(|m| m.as_str().to_string()), c[5].to_string()))
}

pub fn format_open(opened: NaiveDate, text: &str, nudge: Option<NaiveDate>, every: Option<&str>, from: &str) -> String {
    let mut s = format!("- [ ] {} · {text}", opened.format("%Y-%m-%d"));
    if let Some(n) = nudge {
        s.push_str(&format!(" · nudge {}", n.format("%Y-%m-%d")));
        if let Some(e) = every {
            s.push_str(&format!(" · every {e}"));
        }
    }
    s.push_str(&format!(" · from: {from}"));
    s
}

/// A valid recurrence period, normalised to lower case.
pub fn parse_every(v: &str) -> Option<String> {
    let v = v.trim().to_lowercase();
    rx(&format!(r"\A(?:{PERIOD})\z")).is_match(&v).then_some(v)
}

/// The nudge one period after `from`: monthly is the same day next month, clamped to the
/// month's last day; weekly is seven days; `Nd` is N days.
pub fn next_nudge(from: NaiveDate, every: &str) -> Option<NaiveDate> {
    match every {
        "monthly" => from.checked_add_months(chrono::Months::new(1)),
        "weekly" => from.checked_add_signed(chrono::Duration::days(7)),
        d => from.checked_add_signed(chrono::Duration::days(d.strip_suffix('d')?.parse().ok()?)),
    }
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
    /// Every reader of the loops comes through here, and every writer through `save`, so
    /// the needs view follows the date as well as the file.
    pub fn load(ctx: &Ctx) -> R<Option<OpenDoc>> {
        let doc = OpenDoc::load_raw(ctx)?;
        if doc.is_some() {
            crate::needs::refresh(ctx);
        }
        Ok(doc)
    }

    /// `load` without the view: what rendering the view itself reads.
    pub fn load_raw(ctx: &Ctx) -> R<Option<OpenDoc>> {
        let path = vpath(ctx, OPEN_REL);
        Ok(TextFile::load(&path)?.map(|file| OpenDoc::parse(path, file)))
    }

    pub fn parse(path: String, file: TextFile) -> OpenDoc {
        let lines = file.lines();
        let mut loops = Vec::new();
        let mut bad = Vec::new();
        let mut section = String::new();
        let mut top = String::new();
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
            if let Some((level, h)) = heading(t) {
                if level <= 2 {
                    top = h.clone();
                }
                section = h;
                continue;
            }
            if !(t.starts_with("- ") || t.starts_with("* ") || t == "-" || t == "*") {
                continue;
            }
            let (plain, need) = crate::needs::split_need(line.trim_end());
            match parse_open(&plain).filter(|p| need.is_none() || p.2.is_none()) {
                Some((opened, text, nudge, every, from)) => loops.push(Loop {
                    n: loops.len() + 1,
                    line_idx: i,
                    section: section.clone(),
                    top: top.clone(),
                    opened,
                    text,
                    nudge,
                    every,
                    from,
                    need,
                    raw: line.trim_end().to_string(),
                }),
                None => {
                    let why = if t.starts_with("- [x]") || t.starts_with("- [X]") {
                        "closed loop left in open.md (af open close moves it to the month log)"
                    } else {
                        "line does not parse as '- [ ] YYYY-MM-DD · text[ · nudge YYYY-MM-DD[ · every monthly|weekly|Nd] | · need now|on YYYY-MM-DD|when <trigger> · detail needs/<slug>.md] · from: <id>'"
                    };
                    bad.push((i + 1, t.to_string(), why.to_string()));
                }
            }
        }
        OpenDoc { path, file, lines, loops, bad }
    }

    /// Loops due today or earlier, dated needs items among them, oldest first, with their
    /// age in days.
    pub fn due(&self, today: NaiveDate) -> Vec<(Loop, i64)> {
        let mut v: Vec<(Loop, i64)> = self
            .loops
            .iter()
            .filter(|l| matches!(l.due_on(), Some(n) if n <= today))
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

    pub fn save(&mut self, ctx: &Ctx) -> R<()> {
        self.file.set_lines(&self.lines);
        self.file.save(&self.path)?;
        crate::needs::refresh(ctx);
        Ok(())
    }
}

pub fn due_line(l: &Loop, age: i64) -> String {
    format!("{}  ({age}d, #{})", l.raw, l.n)
}

pub fn loop_json(l: &Loop, age: Option<i64>) -> Value {
    let mut v = json!({
        "n": l.n, "section": l.section, "top": l.top, "opened": l.opened.format("%Y-%m-%d").to_string(), "text": l.text,
        "nudge": l.due_on().map(|n| n.format("%Y-%m-%d").to_string()), "every": l.every, "from": l.from,
        "need": l.need.as_ref().map(|n| n.when.field()), "detail": l.need.as_ref().map(|n| n.detail_rel()),
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
        /// recur: monthly, weekly, or Nd; closing re-dates the loop one period on
        #[arg(long)]
        every: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        section: Option<String>,
    },
    /// Close a loop by number or text prefix and move it to the month log (a recurring loop is re-dated)
    Close {
        target: String,
        #[arg(long)]
        outcome: String,
        /// end a recurring loop for good instead of re-dating it
        #[arg(long = "final")]
        final_: bool,
    },
    /// Change a loop's nudge date in place; its opened date, text, and source stay
    Redate {
        target: String,
        /// a date (YYYY-MM-DD) or +Nd from today
        #[arg(long)]
        nudge: String,
    },
    /// Loops whose nudge date is today or earlier, and dated needs items from their date, oldest first
    Due {
        #[arg(long)]
        json: bool,
    },
    /// Mark the loop watch's report as read; its session-start line stays quiet until the next run
    WatchReviewed,
}

pub fn run(ctx: &Ctx, cmd: OpenCmd) -> R<i32> {
    match cmd {
        OpenCmd::Add { text, nudge, every, from, section } => {
            cmd_add(ctx, &text, nudge.as_deref(), every.as_deref(), from.as_deref(), section.as_deref())
        }
        OpenCmd::Close { target, outcome, final_ } => cmd_close(ctx, &target, &outcome, final_),
        OpenCmd::Redate { target, nudge } => cmd_redate(ctx, &target, &nudge),
        OpenCmd::Due { json } => cmd_due(ctx, json),
        OpenCmd::WatchReviewed => crate::loop_watch::cmd_reviewed(ctx),
    }
}

pub fn parse_nudge(v: &str, today: NaiveDate) -> Option<NaiveDate> {
    if let Some(c) = rx(r"\A\+(\d{1,4})d\z").captures(v.trim()) {
        return today.checked_add_signed(chrono::Duration::days(c[1].parse().ok()?));
    }
    date(v.trim())
}

pub fn cmd_add(
    ctx: &Ctx,
    text: &str,
    nudge: Option<&str>,
    every: Option<&str>,
    from: Option<&str>,
    section: Option<&str>,
) -> R<i32> {
    let line = add_loop(ctx, text, nudge, every, from, section)?;
    outln!("{line}");
    Ok(0)
}

/// `af open add` without the printing: the line it wrote. `af watch sweep` raises its loops here.
pub fn add_loop(
    ctx: &Ctx,
    text: &str,
    nudge: Option<&str>,
    every: Option<&str>,
    from: Option<&str>,
    section: Option<&str>,
) -> R<String> {
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
    let every = match every {
        Some(v) => match parse_every(v) {
            Some(_) if nudge.is_none() => die!("--every needs --nudge: the first date the loop comes due"),
            Some(e) => Some(e),
            None => die!("--every takes monthly, weekly, or Nd (N days, 1 or more), got '{v}'"),
        },
        None => None,
    };
    let from = life::one_line(from.unwrap_or(DEFAULT_FROM));
    let section = life::one_line(section.unwrap_or(DEFAULT_SECTION));
    if section.is_empty() || from.is_empty() {
        die!("--section and --from may not be empty");
    }
    let line = format_open(today, &text, nudge, every.as_deref(), &from);
    match parse_open(&line) {
        Some((_, t, n, e, f)) if t == text && n == nudge && e == every && f == from => {}
        _ => die!("the loop text breaks the open.md grammar (it may not contain ' · nudge ', ' · every ', ' · need ', ' · detail ', or ' · from: '): {text}"),
    }
    let path = vpath(ctx, OPEN_REL);
    let file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
    let mut doc = OpenDoc::parse(path, file);
    insert_in_section(&mut doc.lines, &section, &line);
    doc.save(ctx)?;
    Ok(line)
}

/// Add `line` at the end of its section, creating the section at the file's end if needed.
pub fn insert_in_section(lines: &mut Vec<String>, section: &str, line: &str) {
    let h = lines.iter().position(|l| heading(l.trim()).map(|(lv, t)| lv >= 2 && t == section).unwrap_or(false));
    match h {
        Some(h) => {
            let level = heading(lines[h].trim()).map(|(lv, _)| lv).unwrap_or(2);
            let end = (h + 1..lines.len())
                .find(|&k| heading(lines[k].trim()).map(|(lv, _)| lv <= level).unwrap_or(false))
                .unwrap_or(lines.len());
            match (h + 1..end).rev().find(|&k| !lines[k].trim().is_empty()) {
                Some(last) => lines.insert(last + 1, line.to_string()),
                None => {
                    lines.insert(h + 1, String::new());
                    lines.insert(h + 2, line.to_string());
                }
            }
        }
        None => {
            if lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            lines.push(format!("## {section}"));
            lines.push(String::new());
            lines.push(line.to_string());
        }
    }
}

/// The one open loop `target` names: its number, or a text prefix (an exact match wins
/// over longer loops that share the prefix).
fn find_target(doc: &OpenDoc, target: &str) -> R<Loop> {
    find_target_in(doc, target, |_| true, "open loop")
}

/// `find_target` among the loops `keep` admits; `what` names them in a refusal. A number
/// keeps its place in the whole file, so it is the number `af open close` takes too.
pub fn find_target_in(doc: &OpenDoc, target: &str, keep: impl Fn(&Loop) -> bool, what: &str) -> R<Loop> {
    let t = target.trim();
    let hit: Loop = if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = t.parse().unwrap_or(0);
        match doc.loops.iter().find(|l| l.n == n && keep(l)) {
            Some(l) => l.clone(),
            None => die!("no {what} #{t} ({} open)", doc.loops.len()),
        }
    } else {
        let p = t.to_lowercase();
        if p.is_empty() {
            die!("give a loop number or a text prefix");
        }
        let cands: Vec<&Loop> = doc.loops.iter().filter(|l| keep(l) && l.text.to_lowercase().starts_with(&p)).collect();
        let exact: Vec<&&Loop> = cands.iter().filter(|l| l.text.to_lowercase() == p).collect();
        match (cands.len(), exact.len()) {
            (0, _) => die!("no {what} starts with '{t}'"),
            (1, _) => cands[0].clone(),
            (_, 1) => (*exact[0]).clone(),
            (k, _) => {
                for l in &cands {
                    outln!("  #{} {}", l.n, l.raw);
                }
                die!("ambiguous prefix '{t}' matches {k} {what}s; give the number or a longer prefix")
            }
        }
    };
    Ok(hit)
}

/// `af open close <target> --outcome <text> [--final]`: a one-off loop moves to the month
/// log; a recurring one is logged and re-dated one period on, unless `--final` ends it.
pub fn cmd_close(ctx: &Ctx, target: &str, outcome: &str, final_: bool) -> R<i32> {
    let mut doc = match OpenDoc::load(ctx)? {
        Some(d) => d,
        None => die!("{OPEN_REL} not found"),
    };
    let hit = find_target(&doc, target)?;
    close_loop(ctx, &mut doc, &hit, outcome, final_, "af open close")
}

/// The one way a loop closes, whichever verb named it. A needs item's detail file moves to
/// `needs/closed/` and the view is rewritten.
pub fn close_loop(ctx: &Ctx, doc: &mut OpenDoc, hit: &Loop, outcome: &str, final_: bool, verb: &str) -> R<i32> {
    let outcome = life::one_line(outcome);
    if outcome.is_empty() {
        die!("--outcome needs the result, in a few words");
    }
    let next = match (hit.nudge, hit.every.as_deref()) {
        (_, Some(_)) if final_ => None,
        (Some(n), Some(e)) => match next_nudge(n, e) {
            Some(d) => Some(d),
            None => die!("cannot compute the next nudge for '{}' (every {e} from {n})", hit.text),
        },
        _ => None,
    };
    // Log first: a failure after the append leaves the loop open, never lost.
    if let Some(next) = next {
        let next_s = next.format("%Y-%m-%d").to_string();
        let rel = life::append_log(ctx, "closed", &format!("{} → {outcome} (next nudge {next_s})", hit.text))?;
        doc.lines[hit.line_idx] = format_open(hit.opened, &hit.text, Some(next), hit.every.as_deref(), &hit.from);
        doc.save(ctx)?;
        outln!("{verb}: #{} {} → {outcome} (logged in {rel}; recurs, next nudge {next_s})", hit.n, hit.text);
        return Ok(0);
    }
    let fin = if hit.every.is_some() { " (final)" } else { "" };
    let archive = hit.need.as_ref().and_then(|n| crate::needs::archive_target(ctx, n).map(|to| (n, to)));
    let kept = archive.as_ref().map(|(_, to)| format!(" (detail: {to})")).unwrap_or_default();
    // The log, then the line, then the file. A failed log write changes nothing. A failed line
    // write leaves the loop open with a closed entry in the log, and a retry logs it again. A
    // failed move leaves the close done with the notes still in `needs/`, where doctor names them.
    let rel = life::append_log(ctx, "closed", &format!("{} → {outcome}{fin}{kept}", hit.text))?;
    doc.lines.remove(hit.line_idx);
    doc.save(ctx)?;
    if let Some((n, to)) = &archive {
        crate::needs::move_detail(ctx, n, to)?;
    }
    outln!("{verb}: #{} {} → {outcome}{fin}{kept} (moved to {rel})", hit.n, hit.text);
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
    if hit.need.is_some() {
        die!("'{}' is on the needs list and carries no nudge; use af needs move", hit.text);
    }
    let was = hit.nudge.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "none".to_string());
    let line = format_open(hit.opened, &hit.text, Some(new), hit.every.as_deref(), &hit.from);
    doc.lines[hit.line_idx] = line.clone();
    doc.save(ctx)?;
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
