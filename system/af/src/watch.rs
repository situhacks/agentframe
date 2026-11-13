//! `workspace/life/watch.md`: the watch queue and `af watch add | list | result | close | sweep`.
//!
//! One list line per queued check, fields joined by ` · ` (shape: `library/schema/watch.md`):
//! `- [ ] w-20261113-01 · owner af-main · added 2026-11-13 · expires 2026-11-27 · when date 2026-11-20 15:00 · do notify "Return the books" · tier code · checked never · result none · status open`
//! Free text is a JSON string literal that holds no ` · ` and no newline. Chats queue checks with
//! `add`; the batch run (`system/tools/watch_batch.py`) reads `list --due --json`, records what it
//! found with `result`, and runs `sweep` first. Loop conditions read open.md through `crate::open`,
//! so there is one parser for open loops.

use crate::core::{today_date, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::rx;
use crate::life::{self, vpath, TextFile};
use crate::open::{self, Loop, OpenDoc, OPEN_REL};
use crate::out;
use crate::outln;
use crate::sessions;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use clap::{Args, Subcommand};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const WATCH_REL: &str = "workspace/life/watch.md";
const BATCH_LOG_REL: &str = "system/logs/watch-batch.log";
const ENV_REL: &str = "system/tools/.env";
const NOTIFY_REL: &str = "system/tools/notify.py";
pub const PHONE_KEY: &str = "REMINDERS_DISCORD_CHANNEL_ID";

const DEFAULT_DAYS: i64 = 14;
const MAX_DAYS: i64 = 90;
const SAY_MAX: usize = 120;
const THEN_MAX: usize = 160;
/// A query, a statement, a path, or a loop's text.
const FIELD_MAX: usize = 300;
const DETAIL_MAX: usize = 200;
const OVERDUE_DAYS: i64 = 1;
const FIRING_STALE_HOURS: i64 = 24;
const BATCH_STALE_HOURS: i64 = 30;
const SEP: &str = " · ";

// ------------------------------------------------------------------ the row

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    Notify,
    Loop,
    Both,
}

impl Act {
    fn parse(s: &str) -> Option<Act> {
        match s {
            "notify" => Some(Act::Notify),
            "loop" => Some(Act::Loop),
            "notify+loop" => Some(Act::Both),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Act::Notify => "notify",
            Act::Loop => "loop",
            Act::Both => "notify+loop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Open,
    Firing,
    Met,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        match s {
            "open" => Some(Status::Open),
            "firing" => Some(Status::Firing),
            "met" => Some(Status::Met),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Firing => "firing",
            Status::Met => "met",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cond {
    Date { date: NaiveDate, time: Option<NaiveTime> },
    LoopClosed { text: String, nudge: Option<NaiveDate> },
    LoopOpen { on: NaiveDate, text: String },
    FileExists { path: String },
    FileChanged { path: String, sha: Option<String> },
    Email { query: String, means: String },
}

impl Cond {
    fn kind(&self) -> &'static str {
        match self {
            Cond::Date { .. } => "date",
            Cond::LoopClosed { .. } => "loop-closed",
            Cond::LoopOpen { .. } => "loop-open",
            Cond::FileExists { .. } => "file-exists",
            Cond::FileChanged { .. } => "file-changed",
            Cond::Email { .. } => "email",
        }
    }

    fn tier(&self) -> &'static str {
        if matches!(self, Cond::Email { .. }) {
            "haiku"
        } else {
            "code"
        }
    }

    /// What two items watch for, ignoring state recorded at add time (a nudge or a hash).
    fn identity(&self) -> String {
        match self {
            Cond::Date { date, time } => format!("date {}", fmt_moment(*date, *time)),
            Cond::LoopClosed { text, .. } => format!("loop closed {text}"),
            Cond::LoopOpen { on, text, .. } => format!("loop open-on {} {text}", fmt_date(*on)),
            Cond::FileExists { path } => format!("file exists {path}"),
            Cond::FileChanged { path, .. } => format!("file changed {path}"),
            Cond::Email { query, means } => format!("email {query} means {means}"),
        }
    }

    fn text(&self) -> String {
        match self {
            Cond::Date { date, time } => format!("date {}", fmt_moment(*date, *time)),
            Cond::LoopClosed { text, nudge } => format!("loop closed {} nudge {}", jstr(text), fmt_opt_date(*nudge)),
            Cond::LoopOpen { on, text } => format!("loop open-on {} {}", fmt_date(*on), jstr(text)),
            Cond::FileExists { path } => format!("file exists {}", jstr(path)),
            Cond::FileChanged { path, sha } => {
                format!("file changed {} sha256 {}", jstr(path), sha.as_deref().unwrap_or("none"))
            }
            Cond::Email { query, means } => format!("email {} means {}", jstr(query), jstr(means)),
        }
    }

    /// The date a condition cannot be met before: a queued item must outlive it.
    fn floor(&self) -> Option<NaiveDate> {
        match self {
            Cond::Date { date, .. } => Some(*date),
            Cond::LoopOpen { on, .. } => Some(*on),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    id: String,
    owner: String,
    added: NaiveDate,
    after: Option<NaiveDate>,
    expires: NaiveDate,
    cond: Cond,
    act: Act,
    say: String,
    then: Option<String>,
    checked: Option<NaiveDateTime>,
    result: String,
    status: Status,
    line_idx: usize,
}

impl Item {
    /// The row's fields in order, each as it is written: the unit a round trip is checked against.
    fn fields(&self) -> Vec<(&'static str, String)> {
        let mut v = vec![("id", self.id.clone()), ("owner", format!("owner {}", self.owner)), ("added", format!("added {}", fmt_date(self.added)))];
        if let Some(a) = self.after {
            v.push(("after", format!("after {}", fmt_date(a))));
        }
        v.push(("expires", format!("expires {}", fmt_date(self.expires))));
        v.push(("when", format!("when {}", self.cond.text())));
        v.push(("do", format!("do {} {}", self.act.word(), jstr(&self.say))));
        if let Some(t) = &self.then {
            v.push(("then", format!("then {}", jstr(t))));
        }
        v.push(("tier", format!("tier {}", self.cond.tier())));
        v.push(("checked", format!("checked {}", self.checked.map(fmt_stamp).unwrap_or_else(|| "never".to_string()))));
        v.push(("result", format!("result {}", self.result)));
        v.push(("status", format!("status {}", self.status.word())));
        v
    }

    fn row(&self) -> String {
        format!("- [ ] {}", self.fields().into_iter().map(|(_, f)| f).collect::<Vec<_>>().join(SEP))
    }

    /// Refuse a row that would not read back as this item, naming the field at fault: a write that
    /// parsed wrongly would make the item vanish from `list` and from the runner.
    fn check_roundtrip(&self) -> R<()> {
        for (name, f) in self.fields() {
            if f.contains(SEP) || f.contains('\n') || f.contains('\r') {
                die!("the {name} field would break the row (it holds ' · ' or a newline): {f}");
            }
        }
        let row = self.row();
        match parse_row(&row) {
            Some(mut back) => {
                back.line_idx = self.line_idx;
                match self.first_difference(&back) {
                    None => Ok(()),
                    Some(name) => die!("the row would not read back as written: its {name} field changes ({row})"),
                }
            }
            None => die!("the row would not parse back, so {} would vanish from the queue: {row}", self.id),
        }
    }

    fn first_difference(&self, back: &Item) -> Option<&'static str> {
        let same = [
            ("id", self.id == back.id),
            ("owner", self.owner == back.owner),
            ("added", self.added == back.added),
            ("after", self.after == back.after),
            ("expires", self.expires == back.expires),
            ("when", self.cond == back.cond),
            ("do", self.act == back.act && self.say == back.say),
            ("then", self.then == back.then),
            ("checked", self.checked == back.checked),
            ("result", self.result == back.result),
            ("status", self.status == back.status),
        ];
        same.iter().find(|(_, ok)| !ok).map(|(n, _)| *n)
    }

    /// Open, begun (`after` is today or earlier) and not yet expired; and every firing row.
    fn is_due(&self, today: NaiveDate) -> bool {
        match self.status {
            Status::Firing => true,
            Status::Open => self.after.map_or(true, |a| a <= today) && self.expires >= today,
            Status::Met => false,
        }
    }
}

fn jstr(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn fmt_opt_date(d: Option<NaiveDate>) -> String {
    d.map(fmt_date).unwrap_or_else(|| "none".to_string())
}

fn fmt_moment(d: NaiveDate, t: Option<NaiveTime>) -> String {
    match t {
        Some(t) => format!("{} {}", fmt_date(d), t.format("%H:%M")),
        None => fmt_date(d),
    }
}

fn fmt_stamp(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%d %H:%M").to_string()
}

fn field<'a>(part: &'a str, key: &str) -> Option<&'a str> {
    part.strip_prefix(key)?.strip_prefix(' ')
}

/// The JSON string literal at the start of `s`, and what follows it.
fn take_json(s: &str) -> Option<(String, &str)> {
    if !s.starts_with('"') {
        return None;
    }
    let mut it = serde_json::Deserializer::from_str(s).into_iter::<String>();
    let v = it.next()?.ok()?;
    let at = it.byte_offset();
    Some((v, &s[at..]))
}

/// A string literal that is the whole of `s`.
fn only_json(s: &str) -> Option<String> {
    match take_json(s)? {
        (v, "") => Some(v),
        _ => None,
    }
}

fn opt_date(s: &str) -> Option<Option<NaiveDate>> {
    if s == "none" {
        Some(None)
    } else {
        date(s).map(Some)
    }
}

fn parse_cond(s: &str) -> Option<Cond> {
    if let Some(v) = field(s, "date") {
        let c = rx(r"\A(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}))?\z").captures(v)?;
        let time = match c.get(2) {
            Some(m) => Some(NaiveTime::parse_from_str(m.as_str(), "%H:%M").ok()?),
            None => None,
        };
        return Some(Cond::Date { date: date(&c[1])?, time });
    }
    if let Some(v) = field(s, "loop closed") {
        let (text, rest) = take_json(v)?;
        let nudge = opt_date(rest.strip_prefix(" nudge ")?)?;
        return Some(Cond::LoopClosed { text, nudge });
    }
    if let Some(v) = field(s, "loop open-on") {
        let (d, rest) = v.split_once(' ')?;
        return Some(Cond::LoopOpen { on: date(d)?, text: only_json(rest)? });
    }
    if let Some(v) = field(s, "file exists") {
        return Some(Cond::FileExists { path: only_json(v)? });
    }
    if let Some(v) = field(s, "file changed") {
        let (path, rest) = take_json(v)?;
        let h = rest.strip_prefix(" sha256 ")?;
        let sha = if h == "none" {
            None
        } else if rx(r"\A[0-9a-f]{64}\z").is_match(h) {
            Some(h.to_string())
        } else {
            return None;
        };
        return Some(Cond::FileChanged { path, sha });
    }
    if let Some(v) = field(s, "email") {
        let (query, rest) = take_json(v)?;
        let means = only_json(rest.strip_prefix(" means ")?)?;
        return Some(Cond::Email { query, means });
    }
    None
}

/// One list line; None when it does not parse. `line_idx` is set by the caller.
fn parse_row(line: &str) -> Option<Item> {
    let rest = line.trim_end().strip_prefix("- [ ] ")?;
    let mut parts = rest.split(SEP);
    let id = parts.next()?;
    if !rx(r"\Aw-\d{8}-\d{2,}\z").is_match(id) {
        return None;
    }
    let owner = field(parts.next()?, "owner")?.to_string();
    let added = date(field(parts.next()?, "added")?)?;
    let mut next = parts.next()?;
    let mut after = None;
    if let Some(v) = field(next, "after") {
        after = Some(date(v)?);
        next = parts.next()?;
    }
    let expires = date(field(next, "expires")?)?;
    let cond = parse_cond(field(parts.next()?, "when")?)?;
    let (word, lit) = field(parts.next()?, "do")?.split_once(' ')?;
    let act = Act::parse(word)?;
    let say = only_json(lit)?;
    let mut next = parts.next()?;
    let mut then = None;
    if let Some(v) = field(next, "then") {
        then = Some(only_json(v)?);
        next = parts.next()?;
    }
    if field(next, "tier")? != cond.tier() {
        return None;
    }
    let checked = match field(parts.next()?, "checked")? {
        "never" => None,
        v => Some(NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M").ok()?),
    };
    let result = field(parts.next()?, "result")?.to_string();
    if !rx(r"\A(?:none|not met|met: .+|error: .+|error twice: .+)\z").is_match(&result) {
        return None;
    }
    let status = Status::parse(field(parts.next()?, "status")?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(Item { id: id.to_string(), owner, added, after, expires, cond, act, say, then, checked, result, status, line_idx: 0 })
}

// ------------------------------------------------------------------ the file

struct Doc {
    path: String,
    file: TextFile,
    lines: Vec<String>,
    items: Vec<Item>,
    /// (1-based line number, line) for list lines that do not parse.
    bad: Vec<(usize, String)>,
}

impl Doc {
    /// A missing file is an empty queue.
    fn load(ctx: &Ctx) -> R<Doc> {
        let path = vpath(ctx, WATCH_REL);
        let file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
        Ok(Doc::parse(path, file))
    }

    fn parse(path: String, file: TextFile) -> Doc {
        let lines = file.lines();
        let (mut items, mut bad) = (Vec::new(), Vec::new());
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
            if !(t.starts_with("- ") || t == "-") {
                continue;
            }
            match parse_row(line) {
                Some(mut it) => {
                    it.line_idx = i;
                    items.push(it);
                }
                None => bad.push((i + 1, t.to_string())),
            }
        }
        Doc { path, file, lines, items, bad }
    }

    fn find(&self, id: &str) -> R<Item> {
        match self.items.iter().find(|i| i.id == id.trim()) {
            Some(i) => Ok(i.clone()),
            None => die!("no watch item {} in {WATCH_REL}", id.trim()),
        }
    }

    fn save(&mut self) -> R<()> {
        self.file.set_lines(&self.lines);
        self.file.save_atomic(&self.path)
    }

    /// Add a row at the end. The item must read back as written, or nothing is added.
    fn append(&mut self, item: &Item) -> R<()> {
        let mut item = item.clone();
        item.line_idx = if self.lines.is_empty() { 2 } else { self.lines.len() };
        item.check_roundtrip()?;
        if self.lines.is_empty() {
            self.lines.push("# Watch queue".to_string());
            self.lines.push(String::new());
        }
        self.lines.push(item.row());
        self.items.push(item);
        Ok(())
    }

    /// Rewrite an item's row in place. It must read back as written, or nothing changes.
    fn replace(&mut self, item: &Item) -> R<()> {
        item.check_roundtrip()?;
        self.lines[item.line_idx] = item.row();
        if let Some(slot) = self.items.iter_mut().find(|i| i.id == item.id) {
            *slot = item.clone();
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else {
            return;
        };
        let idx = self.items[pos].line_idx;
        self.lines.remove(idx);
        self.items.remove(pos);
        for i in self.items.iter_mut().filter(|i| i.line_idx > idx) {
            i.line_idx -= 1;
        }
    }
}

/// The month log for the month holding `day`; empty when there is none.
fn log_text_of(ctx: &Ctx, day: NaiveDate) -> String {
    let rel = life::month_log_rel(&day.format("%Y-%m").to_string());
    TextFile::load(&vpath(ctx, &rel)).ok().flatten().map(|t| t.text).unwrap_or_default()
}

fn month_log_text(ctx: &Ctx) -> String {
    log_text_of(ctx, today_date())
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

/// The first day of each month from `from`'s through `to`'s.
fn months(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let (mut m, end) = (first_of_month(from), first_of_month(to));
    let mut out = Vec::new();
    while m <= end {
        out.push(m);
        match m.checked_add_months(chrono::Months::new(1)) {
            Some(n) => m = n,
            None => break,
        }
    }
    out
}

/// Whether `af open close` (or `af needs close`) logged the loop `text` closed on or after `since`:
/// the month log line `- YYYY-MM-DD HH:MM · closed: <text> → <outcome>`, which a recurring loop's
/// close also writes (its outcome ends `(next nudge D)`). Months from `since`'s to this one are read.
fn close_logged(ctx: &Ctx, text: &str, since: NaiveDate) -> bool {
    let re = rx(&format!(r"\A- (\d{{4}}-\d{{2}}-\d{{2}}) \d{{2}}:\d{{2}} · closed: {} → ", crate::fm::esc(&life::one_line(text))));
    months(since, today_date()).into_iter().any(|m| {
        log_text_of(ctx, m).lines().any(|l| re.captures(l).and_then(|c| date(&c[1])).is_some_and(|d| d >= since))
    })
}

fn open_text(ctx: &Ctx) -> String {
    TextFile::load(&vpath(ctx, OPEN_REL)).ok().flatten().map(|t| t.text).unwrap_or_default()
}

/// `w-YYYYMMDD-NN`: one more than the highest NN used today, in the queue or the month log.
fn next_id(ctx: &Ctx, doc: &Doc, today: NaiveDate) -> String {
    let stamp = today.format("%Y%m%d").to_string();
    let re = rx(&format!(r"w-{stamp}-(\d+)"));
    let hi = [doc.file.text.clone(), month_log_text(ctx)]
        .iter()
        .flat_map(|t| re.captures_iter(t).filter_map(|c| c[1].parse::<u64>().ok()).collect::<Vec<_>>())
        .max()
        .unwrap_or(0);
    format!("w-{stamp}-{:02}", hi + 1)
}

fn resolve(ctx: &Ctx, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(&ctx.root).join(p)
    }
}

enum Hash {
    Found(String),
    /// The only read failure that means "no file".
    Missing,
    Unreadable(String),
}

/// The file's sha256, streamed. Only "not found" is the no-file state; a lock, a permission
/// refusal or a directory is unreadable, never a change.
fn sha_of(path: &Path) -> Hash {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Hash::Missing,
        Err(e) => return Hash::Unreadable(e.to_string()),
    };
    let (mut hasher, mut buf) = (Sha256::new(), [0u8; 64 * 1024]);
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) => return Hash::Unreadable(e.to_string()),
        }
    }
    Hash::Found(format!("{:x}", hasher.finalize()))
}

// ------------------------------------------------------------------ verbs

#[derive(Subcommand, Debug)]
pub enum WatchCmd {
    /// Queue one check: a date, a loop closing or staying open, a file, or an email
    Add(AddArgs),
    /// List queued checks, as rows or JSON with `code_met`
    List {
        #[arg(long)]
        owner: Option<String>,
        /// Open items whose `after` has come and whose expiry has not passed, and every firing item
        #[arg(long)]
        due: bool,
        #[arg(long)]
        json: bool,
    },
    /// Record what the batch run found (for the runner only)
    Result {
        id: String,
        #[arg(value_parser = ["not-met", "error", "error-twice", "met", "fired"])]
        outcome: String,
        #[arg(long)]
        detail: Option<String>,
    },
    /// Remove an item by hand and log it
    Close {
        id: String,
        #[arg(long)]
        outcome: String,
    },
    /// Expire items past their date (a loop each), and clear items already met
    Sweep {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Debug)]
#[command(group(clap::ArgGroup::new("condition").required(true).multiple(false)
    .args(["date", "loop_closed", "loop_open", "file_exists", "file_changed", "email"])))]
pub struct AddArgs {
    /// Met at the first run at or after YYYY-MM-DD[THH:MM]
    #[arg(long)]
    date: Option<String>,
    /// Met when the open loop (number or text prefix) is gone, or its nudge has moved on
    #[arg(long = "loop-closed")]
    loop_closed: Option<String>,
    /// Met on --on when the open loop (number or text prefix) is still open and unchanged
    #[arg(long = "loop-open", requires = "on")]
    loop_open: Option<String>,
    #[arg(long, requires = "loop_open")]
    on: Option<String>,
    /// Met when the path exists (absolute, or relative to the vault)
    #[arg(long = "file-exists")]
    file_exists: Option<String>,
    /// Met when the file's sha256 differs from now
    #[arg(long = "file-changed")]
    file_changed: Option<String>,
    /// A Gmail search; met when a thread proves --means
    #[arg(long, requires = "means")]
    email: Option<String>,
    #[arg(long, requires = "email")]
    means: Option<String>,
    /// notify, loop, or notify+loop
    #[arg(long = "do")]
    do_: String,
    /// The phone line or the loop text, 120 characters at most
    #[arg(long)]
    say: String,
    /// A date or +Nd; default 14 days out (a later date: that date plus a day); 90 days at most
    #[arg(long)]
    expires: Option<String>,
    /// A date or +Nd: not checked before it
    #[arg(long)]
    after: Option<String>,
    /// A follow-up for main when it fires, 160 characters at most
    #[arg(long)]
    then: Option<String>,
    /// The chat that queued it (default af-main; a side chat passes its own name)
    #[arg(long)]
    owner: Option<String>,
}

pub fn run(ctx: &Ctx, cmd: WatchCmd) -> R<i32> {
    match cmd {
        WatchCmd::Add(a) => cmd_add(ctx, &a),
        WatchCmd::List { owner, due, json } => cmd_list(ctx, owner.as_deref(), due, json),
        WatchCmd::Result { id, outcome, detail } => cmd_result(ctx, &id, &outcome, detail.as_deref()),
        WatchCmd::Close { id, outcome } => cmd_close(ctx, &id, &outcome),
        WatchCmd::Sweep { json } => cmd_sweep(ctx, json),
    }
}

/// Free text for a row: one line, no ` · `, not empty, at most `max` characters.
fn check_text(what: &str, v: &str, max: usize) -> R<String> {
    let t = v.trim();
    if t.is_empty() {
        die!("{what} may not be empty");
    }
    if t.contains('\n') || t.contains('\r') {
        die!("{what} may not contain a newline");
    }
    if t.contains(SEP) {
        die!("{what} may not contain ' · ' (it separates the row's fields): {t}");
    }
    let n = t.chars().count();
    if n > max {
        die!("{what} is {n} characters; the limit is {max}");
    }
    Ok(t.to_string())
}

fn day_arg(what: &str, v: &str, today: NaiveDate) -> R<NaiveDate> {
    match open::parse_nudge(v, today) {
        Some(d) => Ok(d),
        None => die!("{what} takes YYYY-MM-DD or +Nd, got '{v}'"),
    }
}

fn load_open(ctx: &Ctx) -> R<OpenDoc> {
    match OpenDoc::load_raw(ctx)? {
        Some(d) => Ok(d),
        None => die!("{OPEN_REL} not found"),
    }
}

fn loop_target(ctx: &Ctx, target: &str) -> R<Loop> {
    let doc = load_open(ctx)?;
    let hit = open::find_target_in(&doc, target, |_| true, "open loop")?;
    check_text("the loop's text", &hit.text, FIELD_MAX)?;
    Ok(hit)
}

fn cmd_add(ctx: &Ctx, a: &AddArgs) -> R<i32> {
    let today = today_date();
    let now = life::now();

    let (owner, defaulted) = match &a.owner {
        Some(o) => (o.trim().to_string(), false),
        None => (sessions::MAIN_NAME.to_string(), true),
    };
    if !sessions::valid_name(&owner) {
        die!("--owner '{owner}' is not a chat name ({})", sessions::NAME_GRAMMAR);
    }
    if owner.starts_with("worker/") {
        die!("a worker chat ends with its job and cannot own a watch item; queue it as af-main or the chat that reads the result");
    }
    if owner != sessions::MAIN_NAME && sessions::Registry::load(ctx)?.find_name(&owner).is_none() {
        die!("--owner '{owner}' is not in the session registry; af session list shows the chat names, or leave --owner off to queue it as {}", sessions::MAIN_NAME);
    }
    if a.means.is_some() && a.email.is_none() {
        die!("--means goes with --email: the statement a thread must prove");
    }
    if a.on.is_some() && a.loop_open.is_none() {
        die!("--on goes with --loop-open: the day the loop is checked");
    }
    let act = match Act::parse(a.do_.trim()) {
        Some(x) => x,
        None => die!("--do takes notify, loop, or notify+loop, got '{}'", a.do_),
    };
    let say = check_text("--say", &a.say, SAY_MAX)?;
    let then = match &a.then {
        Some(t) => Some(check_text("--then", t, THEN_MAX)?),
        None => None,
    };
    let after = match &a.after {
        Some(v) => Some(day_arg("--after", v, today)?),
        None => None,
    };
    if after.is_some() && a.date.is_some() {
        die!("--after cannot go with --date: a date item is already met only from its own moment");
    }

    let cond = if let Some(v) = &a.date {
        let bad = || format!("--date takes YYYY-MM-DD or YYYY-MM-DDTHH:MM, got '{v}'");
        let Some(c) = rx(r"\A(\d{4}-\d{2}-\d{2})(?:T(\d{2}:\d{2}))?\z").captures(v.trim()) else {
            die!("{}", bad());
        };
        let Some(d) = date(&c[1]) else {
            die!("{}", bad());
        };
        let time = match c.get(2) {
            Some(m) => match NaiveTime::parse_from_str(m.as_str(), "%H:%M") {
                Ok(t) => Some(t),
                Err(_) => die!("{}", bad()),
            },
            None => None,
        };
        let passed = match time {
            Some(t) => d.and_time(t) <= now,
            None => d < today,
        };
        if passed {
            die!("--date {v} has already passed");
        }
        Cond::Date { date: d, time }
    } else if let Some(t) = &a.loop_closed {
        let hit = loop_target(ctx, t)?;
        Cond::LoopClosed { text: hit.text.clone(), nudge: hit.due_on() }
    } else if let Some(t) = &a.loop_open {
        let on = day_arg("--on", a.on.as_deref().unwrap_or(""), today)?;
        if on < today {
            die!("--on {} has already passed", fmt_date(on));
        }
        let hit = loop_target(ctx, t)?;
        Cond::LoopOpen { on, text: hit.text.clone() }
    } else if let Some(p) = &a.file_exists {
        let path = check_text("--file-exists", p, FIELD_MAX)?;
        if resolve(ctx, &path).exists() {
            die!("{path} already exists, so the condition is already met");
        }
        Cond::FileExists { path }
    } else if let Some(p) = &a.file_changed {
        let path = check_text("--file-changed", p, FIELD_MAX)?;
        let full = resolve(ctx, &path);
        let sha = match sha_of(&full) {
            Hash::Found(h) => Some(h),
            Hash::Missing => None,
            Hash::Unreadable(why) => die!("{path} cannot be read ({why}); --file-changed takes a readable file, or one that does not exist yet"),
        };
        Cond::FileChanged { sha, path }
    } else if let (Some(q), Some(m)) = (&a.email, &a.means) {
        Cond::Email { query: check_text("--email", q, FIELD_MAX)?, means: check_text("--means", m, FIELD_MAX)? }
    } else {
        die!("give exactly one condition: --date, --loop-closed, --loop-open with --on, --file-exists, --file-changed, or --email with --means");
    };

    let latest = today + Duration::days(MAX_DAYS);
    if let Some(f) = cond.floor().filter(|f| *f > latest) {
        die!("the condition's date {} is more than {MAX_DAYS} days out (latest {}); a date that far out belongs in an open loop with a nudge", fmt_date(f), fmt_date(latest));
    }
    let mut clamped = false;
    let expires = match &a.expires {
        Some(v) => day_arg("--expires", v, today)?,
        None => {
            let base = after.map_or(today, |x| x.max(today)) + Duration::days(DEFAULT_DAYS);
            let want = cond.floor().map_or(base, |f| base.max(f + Duration::days(1)));
            clamped = want > latest;
            want.min(latest)
        }
    };
    if expires < today {
        die!("expiry {} is in the past", fmt_date(expires));
    }
    if expires > latest {
        die!("expiry {} is more than {MAX_DAYS} days out (latest {}); a date that far out belongs in an open loop with a nudge", fmt_date(expires), fmt_date(latest));
    }
    if let Some(x) = after.filter(|x| expires < *x) {
        die!("expiry {} is before --after {}", fmt_date(expires), fmt_date(x));
    }
    if let Some(f) = cond.floor().filter(|f| expires < *f) {
        die!("expiry {} is before the condition's date {}; it would expire before it could fire", fmt_date(expires), fmt_date(f));
    }

    let row = life::locked(&vpath(ctx, WATCH_REL), || {
        let mut doc = Doc::load(ctx)?;
        if let Some(dup) = doc.items.iter().find(|i| i.status == Status::Open && i.owner == owner && i.cond.identity() == cond.identity()) {
            die!("{} already watches that for {owner}; close it or change the condition", dup.id);
        }
        let item = Item {
            id: next_id(ctx, &doc, today),
            owner: owner.clone(),
            added: today,
            after,
            expires,
            cond,
            act,
            say,
            then,
            checked: None,
            result: "none".to_string(),
            status: Status::Open,
            line_idx: 0,
        };
        doc.append(&item)?;
        doc.save()?;
        Ok(item.row())
    })?;
    outln!("{row}");
    if clamped {
        outln!("expiry clamped to {} (the queue holds nothing past {MAX_DAYS} days); pass --expires to choose", fmt_date(expires));
    }
    if defaulted {
        outln!("owner {}; a side chat passes --owner <its name>", sessions::MAIN_NAME);
    }
    Ok(0)
}

// ------------------------------------------------------------------ list

/// Whether a code condition holds now; None for an email condition, which the runner judges.
/// A loop is closed only on evidence of a close in the month log: a loop that was re-dated,
/// reworded or moved to the needs list was not closed.
fn code_met(ctx: &Ctx, it: &Item, open: Option<&OpenDoc>, now: NaiveDateTime) -> Option<bool> {
    let today = now.date();
    match &it.cond {
        Cond::Date { date, time } => Some(now >= date.and_time(time.unwrap_or(NaiveTime::MIN))),
        Cond::LoopClosed { text, .. } => Some(close_logged(ctx, text, it.added)),
        Cond::LoopOpen { on, text } => Some(today >= *on && open.is_some_and(|d| d.loops.iter().any(|l| l.text == *text))),
        Cond::FileExists { path } => Some(resolve(ctx, path).exists()),
        Cond::FileChanged { path, sha } => Some(match sha_of(&resolve(ctx, path)) {
            Hash::Found(h) => Some(&h) != sha.as_ref(),
            Hash::Missing => sha.is_some(),
            Hash::Unreadable(_) => false,
        }),
        Cond::Email { .. } => None,
    }
}

fn item_json(it: &Item, code_met: Option<bool>) -> Value {
    let (query, means) = match &it.cond {
        Cond::Email { query, means } => (Some(query.clone()), Some(means.clone())),
        _ => (None, None),
    };
    json!({
        "id": it.id, "owner": it.owner, "added": fmt_date(it.added), "after": it.after.map(fmt_date),
        "expires": fmt_date(it.expires), "kind": it.cond.kind(), "when": it.cond.text(), "query": query, "means": means,
        "do": it.act.word(), "say": it.say, "then": it.then, "tier": it.cond.tier(),
        "checked": it.checked.map(fmt_stamp), "result": it.result, "status": it.status.word(), "code_met": code_met,
    })
}

fn cmd_list(ctx: &Ctx, owner: Option<&str>, due: bool, as_json: bool) -> R<i32> {
    let today = today_date();
    let doc = Doc::load(ctx)?;
    if !doc.bad.is_empty() {
        out::err_line(&format!(
            "af watch list: {} row(s) in {WATCH_REL} do not parse and are not listed; af doctor names them",
            doc.bad.len()
        ));
    }
    let rows: Vec<&Item> = doc
        .items
        .iter()
        .filter(|i| owner.map_or(true, |o| i.owner == o.trim()))
        .filter(|i| !due || i.is_due(today))
        .collect();
    if !as_json {
        for it in rows {
            out::line(&it.row());
        }
        return Ok(0);
    }
    let open = OpenDoc::load_raw(ctx).ok().flatten();
    let now = life::now();
    let arr: Vec<Value> = rows.iter().map(|i| item_json(i, code_met(ctx, i, open.as_ref(), now))).collect();
    out::line(&serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_default());
    Ok(0)
}

// ------------------------------------------------------------------ result

/// A detail as the row stores it: one line, no `·` at all, bounded.
fn clean_detail(s: &str) -> String {
    let one = life::one_line(s).replace('·', ";");
    one.chars().take(DETAIL_MAX).collect::<String>().trim().to_string()
}

fn cmd_result(ctx: &Ctx, id: &str, outcome: &str, detail: Option<&str>) -> R<i32> {
    life::locked(&vpath(ctx, WATCH_REL), || cmd_result_unlocked(ctx, id, outcome, detail))
}

fn cmd_result_unlocked(ctx: &Ctx, id: &str, outcome: &str, detail: Option<&str>) -> R<i32> {
    let mut doc = Doc::load(ctx)?;
    let mut it = doc.find(id)?;
    let detail = detail.map(clean_detail).filter(|d| !d.is_empty());
    if outcome == "fired" {
        if it.status != Status::Firing {
            die!("{} is {}, not firing; only a firing item is marked fired", it.id, it.status.word());
        }
        it.status = Status::Met;
    } else {
        if it.status != Status::Open {
            die!("{} is {}, not open; only an open item takes a result", it.id, it.status.word());
        }
        let or_none = |d: Option<String>| d.unwrap_or_else(|| "no detail".to_string());
        match outcome {
            "not-met" => it.result = "not met".to_string(),
            "error" => match detail {
                Some(d) => it.result = format!("error: {d}"),
                None => die!("result error needs --detail: what went wrong"),
            },
            "error-twice" => it.result = format!("error twice: {}", or_none(detail)),
            "met" => {
                it.result = format!("met: {}", or_none(detail));
                it.status = Status::Firing;
            }
            _ => die!("unknown outcome '{outcome}'"),
        }
        it.checked = Some(life::now());
    }
    doc.replace(&it)?;
    doc.save()?;
    outln!("{}", it.row());
    Ok(0)
}

// ------------------------------------------------------------------ close and sweep

fn cmd_close(ctx: &Ctx, id: &str, outcome: &str) -> R<i32> {
    life::locked(&vpath(ctx, WATCH_REL), || cmd_close_unlocked(ctx, id, outcome))
}

fn cmd_close_unlocked(ctx: &Ctx, id: &str, outcome: &str) -> R<i32> {
    let mut doc = Doc::load(ctx)?;
    let it = doc.find(id)?;
    if it.status == Status::Firing {
        die!("{} is firing: the batch run is about to act on it; close it after it is marked fired", it.id);
    }
    let outcome = life::one_line(outcome);
    if outcome.is_empty() {
        die!("--outcome needs the result, in a few words");
    }
    // Log first: a failure after the append leaves the item queued, never lost.
    let rel = life::append_log(ctx, "event", &format!("watch {} closed: {} → {outcome}", it.id, it.say))?;
    doc.remove(&it.id);
    doc.save()?;
    outln!("af watch close: {} {} → {outcome} (logged in {rel})", it.id, it.say);
    Ok(0)
}

/// Log an event unless this month's log or last month's already holds it, so a sweep that crashed
/// across a month boundary does not log it twice.
fn log_once(ctx: &Ctx, text: &str) -> R<()> {
    let last_month = first_of_month(today_date()) - Duration::days(1);
    let needle = format!("event: {text}");
    if !month_log_text(ctx).contains(&needle) && !log_text_of(ctx, last_month).contains(&needle) {
        life::append_log(ctx, "event", text)?;
    }
    Ok(())
}

fn cmd_sweep(ctx: &Ctx, as_json: bool) -> R<i32> {
    life::locked(&vpath(ctx, WATCH_REL), || cmd_sweep_unlocked(ctx, as_json))
}

fn cmd_sweep_unlocked(ctx: &Ctx, as_json: bool) -> R<i32> {
    let today = today_date();
    let mut doc = Doc::load(ctx)?;
    let (mut expired, mut met, mut raised, mut firing) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for it in doc.items.clone() {
        match it.status {
            Status::Open if it.expires < today => {
                log_once(ctx, &format!("watch {} expired", it.id))?;
                if !open_text(ctx).contains(&format!("[{}]", it.id)) {
                    let text = format!("Watch {} expired without firing for {}: {} [{}]", it.id, it.owner, it.say, it.id);
                    open::add_loop(ctx, &text, Some(&fmt_date(today)), None, Some("watch"), Some("Watch"))?;
                    raised.push(it.id.clone());
                }
                doc.remove(&it.id);
                doc.save()?;
                expired.push(it.id);
            }
            Status::Met => {
                log_once(ctx, &format!("watch {} met", it.id))?;
                doc.remove(&it.id);
                doc.save()?;
                met.push(it.id);
            }
            Status::Firing => firing.push(it.id),
            Status::Open => {}
        }
    }
    if as_json {
        let v = json!({"expired": expired, "met": met, "loops_raised": raised, "firing": firing});
        out::line(&serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(0);
    }
    for id in &expired {
        outln!("af watch sweep: {id} expired{}", if raised.contains(id) { " (loop raised)" } else { " (loop already raised)" });
    }
    for id in &met {
        outln!("af watch sweep: {id} met, removed");
    }
    outln!("af watch sweep: {} expired, {} met removed, {} firing left", expired.len(), met.len(), firing.len());
    Ok(0)
}

// ------------------------------------------------------------------ doctor and session start

enum Batch {
    Fine,
    /// The log's last write, over `BATCH_STALE_HOURS` ago.
    Stale(NaiveDateTime),
    /// Items are queued and no run has ever logged.
    NeverRan,
}

struct Health {
    bad: Vec<(usize, String)>,
    overdue: Vec<(String, i64)>,
    stuck: Vec<(String, NaiveDateTime)>,
    batch: Batch,
}

impl Health {
    fn quiet(&self) -> bool {
        self.bad.is_empty() && self.overdue.is_empty() && self.stuck.is_empty() && matches!(self.batch, Batch::Fine)
    }
}

/// A damaged queue must never stop session start or doctor, so a read failure is an empty queue.
fn health(ctx: &Ctx) -> Health {
    let (today, now) = (today_date(), life::now());
    let doc = Doc::load(ctx).unwrap_or_else(|_| Doc::parse(vpath(ctx, WATCH_REL), TextFile::new("")));
    let overdue = doc
        .items
        .iter()
        .filter(|i| i.status == Status::Open && (today - i.expires).num_days() > OVERDUE_DAYS)
        .map(|i| (i.id.clone(), (today - i.expires).num_days()))
        .collect();
    let stuck = doc
        .items
        .iter()
        .filter(|i| i.status == Status::Firing)
        .filter_map(|i| i.checked.filter(|c| now - *c > Duration::hours(FIRING_STALE_HOURS)).map(|c| (i.id.clone(), c)))
        .collect();
    let batch = match life::mtime_min(&vpath(ctx, BATCH_LOG_REL)) {
        Some(m) if now - m > Duration::hours(BATCH_STALE_HOURS) => Batch::Stale(m),
        Some(_) => Batch::Fine,
        None if doc.items.iter().any(|i| i.status != Status::Met) => Batch::NeverRan,
        None => Batch::Fine,
    };
    Health { bad: doc.bad, overdue, stuck, batch }
}

fn env_has(env: &std::collections::HashMap<String, String>, key: &str) -> bool {
    env.get(key).is_some_and(|v| !v.is_empty()) || std::env::var(key).is_ok_and(|v| !v.is_empty())
}

/// Names only: the `.env` values are secrets and are never kept or printed.
fn phone_note(ctx: &Ctx) -> Option<String> {
    if !crate::pyos::isfile(&vpath(ctx, NOTIFY_REL)) {
        return None;
    }
    let text = TextFile::load(&vpath(ctx, ENV_REL)).ok().flatten().map(|t| t.text).unwrap_or_default();
    let env: std::collections::HashMap<String, String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches(|c| c == '"' || c == '\'').to_string()))
        .collect();
    let bot = env_has(&env, PHONE_KEY) && env_has(&env, "DISCORD_BOT_TOKEN");
    if bot || env_has(&env, "REMINDERS_DISCORD_WEBHOOK") {
        return None;
    }
    Some(format!("watch: the phone path is not configured; add {PHONE_KEY} (with DISCORD_BOT_TOKEN) to {ENV_REL}, or the batch run sends no phone message"))
}

fn clip(s: &str) -> String {
    if s.chars().count() > 80 {
        format!("{}...", s.chars().take(80).collect::<String>())
    } else {
        s.to_string()
    }
}

/// What `af doctor` notes: a row that does not parse, an item past its expiry, a firing item the
/// run never finished, a batch run that stopped, and a missing phone key.
pub fn doctor_notes(ctx: &Ctx) -> Vec<String> {
    let h = health(ctx);
    let mut notes = Vec::new();
    for (n, line) in &h.bad {
        notes.push(format!("{WATCH_REL}:{n}: row does not parse (shape: library/schema/watch.md): {}", clip(line)));
    }
    for (id, days) in &h.overdue {
        notes.push(format!("{WATCH_REL}: {id} is {days} days past its expiry; run af watch sweep, or check the batch run"));
    }
    for (id, since) in &h.stuck {
        notes.push(format!("{WATCH_REL}: {id} has been firing since {}; the batch run did not finish it. Check that its loop exists in open.md, then af watch result {id} fired", fmt_stamp(*since)));
    }
    match h.batch {
        Batch::Stale(m) => notes.push(format!("{BATCH_LOG_REL}: no batch run since {}, over {BATCH_STALE_HOURS} hours; check the scheduled task 'AgentFrame batch'", fmt_stamp(m))),
        Batch::NeverRan => notes.push(format!("{BATCH_LOG_REL} does not exist and items are queued; the batch run has never logged, so check the scheduled task 'AgentFrame batch'")),
        Batch::Fine => {}
    }
    notes.extend(phone_note(ctx));
    notes
}

/// The session-start line for a main chat: counts only, never item text. None when the queue is
/// sound and the batch run is alive.
pub fn bundle_line(ctx: &Ctx) -> Option<String> {
    let h = health(ctx);
    if h.quiet() {
        return None;
    }
    let mut parts = Vec::new();
    if !h.bad.is_empty() {
        parts.push(format!("{} row(s) do not parse", h.bad.len()));
    }
    if !h.overdue.is_empty() {
        parts.push(format!("{} past expiry", h.overdue.len()));
    }
    if !h.stuck.is_empty() {
        parts.push(format!("{} firing for over a day", h.stuck.len()));
    }
    match h.batch {
        Batch::Stale(m) => parts.push(format!("no batch run since {}", fmt_stamp(m))),
        Batch::NeverRan => parts.push("the batch run has never logged".to_string()),
        Batch::Fine => {}
    }
    Some(format!("watch queue: {}; check the scheduled task 'AgentFrame batch' and run af doctor ({WATCH_REL})", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> Item {
        parse_row(
            "- [ ] w-20261113-01 · owner af-main · added 2026-11-13 · expires 2026-11-27 · when file exists \"a\" · do notify \"Say\" · tier code · checked never · result none · status open",
        )
        .unwrap()
    }

    #[test]
    fn a_sound_item_reads_back_as_written() {
        assert!(item().check_roundtrip().is_ok());
    }

    #[test]
    fn a_field_holding_the_separator_is_refused_by_name_before_any_write() {
        let mut it = item();
        it.say = "a · b".to_string();
        match it.check_roundtrip() {
            Err(crate::error::AfError::Refusal(m)) => assert!(m.contains("the do field"), "{m}"),
            other => panic!("expected a refusal naming the do field, got {other:?}"),
        }
        let mut doc = Doc::parse("unused".to_string(), TextFile::new(""));
        assert!(doc.append(&it).is_err());
        assert!(doc.lines.is_empty() && doc.items.is_empty(), "a refused row is not added");
        let mut it = item();
        it.result = "met: x · y".to_string();
        match it.check_roundtrip() {
            Err(crate::error::AfError::Refusal(m)) => assert!(m.contains("the result field"), "{m}"),
            other => panic!("expected a refusal naming the result field, got {other:?}"),
        }
        let mut it = item();
        it.result = "bogus".to_string();
        assert!(it.check_roundtrip().is_err(), "a result outside the grammar would not parse back");
    }

    #[test]
    fn the_difference_finder_names_the_first_field_that_changed() {
        let a = item();
        let mut b = a.clone();
        b.owner = "life/travel".to_string();
        b.status = Status::Met;
        assert_eq!(a.first_difference(&b), Some("owner"));
        assert_eq!(a.first_difference(&a), None);
    }
}
