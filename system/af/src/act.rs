//! `af act`: his handover of decisions to the chats ("act as me"). The state is one private file,
//! `workspace/life/act-as-me.md`, frontmatter only: `from` (the filed order that handed over),
//! `scope` (his words), `started`, `until`. While it is in force every chat is told at session
//! start, and `af needs close --as-him` marks a decision made in his name in the month log. The
//! month log is the record: an `as him` close line is "decided as you" until an `act_review` line
//! follows it. The rules a chat follows are `library/process/act-as-me.md`.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::fm::{self, rx};
use crate::life::{self, vpath, TextFile};
use crate::outln;
use crate::pyos;
use chrono::{Duration, NaiveDateTime};
use clap::Subcommand;

pub const STATE_REL: &str = "workspace/life/act-as-me.md";
pub const ORDERS_REL: &str = "workspace/life/sources/orders";
pub const RULES_REL: &str = "library/process/act-as-me.md";
const LOG_DIR_REL: &str = "workspace/life/log";
const TIME_FMT: &str = "%Y-%m-%dT%H:%M";

/// A handover as the state file holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handover {
    pub from: String,
    pub scope: String,
    pub started: NaiveDateTime,
    pub until: NaiveDateTime,
}

impl Handover {
    pub fn in_force(&self, now: NaiveDateTime) -> bool {
        now < self.until
    }

    /// `[src-...] "scope"`, the way the log and the status name it.
    fn tag(&self) -> String {
        format!("[{}] \"{}\"", self.from, self.scope)
    }

    fn file_text(&self) -> String {
        let scope = serde_json::to_string(&self.scope).unwrap_or_else(|_| format!("\"{}\"", self.scope));
        format!(
            "---\nfrom: {}\nscope: {scope}\nstarted: {}\nuntil: {}\n---\n",
            self.from,
            life::fmt_min(self.started),
            life::fmt_min(self.until)
        )
    }
}

/// The handover on disk; None when there is no file or it does not read as one.
pub fn load(ctx: &Ctx) -> Option<Handover> {
    let tf = TextFile::load(&vpath(ctx, STATE_REL)).ok()??;
    let (front, _) = fm::split_fm_optional(&tf.text)?;
    let from = fm::get_scalar(&front, "from").filter(|v| !v.is_empty())?;
    let raw = rx(r"(?m)^scope:[ \t]*(.*?)\s*$").captures(&front).map(|c| c[1].to_string())?;
    let scope = if raw.starts_with('"') { serde_json::from_str::<String>(&raw).ok()? } else { raw };
    let time = |key: &str| fm::get_scalar(&front, key).and_then(|v| NaiveDateTime::parse_from_str(&v, TIME_FMT).ok());
    Some(Handover { from, scope, started: time("started")?, until: time("until")? })
}

/// The handover in force right now, if any.
pub fn active(ctx: &Ctx) -> Option<Handover> {
    load(ctx).filter(|h| h.in_force(life::now()))
}

/// The line every non-worker chat gets at session start while a handover is in force.
pub fn session_line(ctx: &Ctx) -> Option<String> {
    let h = active(ctx)?;
    Some(format!("Acting as him until {}: \"{}\" [{}]. Rules: {RULES_REL}", life::fmt_min(h.until), h.scope, h.from))
}

/// An expired handover is logged once as `act_as_me_ended: expired` and its file removed. Called
/// by every write of the loops, so it needs no clock of its own; it never fails the write.
pub fn settle_expired(ctx: &Ctx) {
    let Some(h) = load(ctx) else {
        return;
    };
    if h.in_force(life::now()) {
        return;
    }
    if life::append_log(ctx, "act_as_me_ended", &format!("expired {}", h.tag())).is_ok() {
        let _ = std::fs::remove_file(vpath(ctx, STATE_REL));
    }
}

/// A state file that exists but does not read as a handover is removed and logged; the log's rel
/// path, or None when there was no file or it reads fine.
fn discard_unreadable(ctx: &Ctx) -> R<Option<String>> {
    if load(ctx).is_some() || !pyos::isfile(&vpath(ctx, STATE_REL)) {
        return Ok(None);
    }
    let rel = life::append_log(ctx, "act_as_me_ended", &format!("removed unreadable handover file {STATE_REL}"))?;
    std::fs::remove_file(vpath(ctx, STATE_REL)).map_err(|e| pyos::os_error(&e, STATE_REL))?;
    Ok(Some(rel))
}

// ------------------------------------------------------------------ decided as him

/// One close made with `--as-him`, read back from the month log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    /// `YYYY-MM-DD HH:MM`
    pub when: String,
    pub question: String,
    pub id: String,
    pub outcome: String,
}

/// The `as him` closes since the last `af act review`, oldest first. The month logs are read in
/// order and a review line empties what came before it.
pub fn unseen(ctx: &Ctx) -> Vec<Decided> {
    // The exact form the button writes: the question is what comes before the first ` → `, and
    // the outcome after it opens `as him [src-...]: `. Anything else is an ordinary close.
    let head = rx(r"\A- (\d{4}-\d{2}-\d{2} \d{2}:\d{2}) · closed: (.*)\z");
    let marked = rx(r"\Aas him \[(src-\d{4}-\d{2}-\d{2}-\d+)\]: (.+)\z");
    let review = rx(r"\A- \d{4}-\d{2}-\d{2} \d{2}:\d{2} · act_review: ");
    let dir = vpath(ctx, LOG_DIR_REL);
    let mut files: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".md")).collect(),
        Err(_) => return Vec::new(),
    };
    files.sort();
    let mut out = Vec::new();
    for name in files {
        let bytes = std::fs::read(pyos::join(&dir, &name)).unwrap_or_default();
        for line in String::from_utf8_lossy(&bytes).lines() {
            let line = line.trim_end();
            if review.is_match(line) {
                out.clear();
            } else if let Some(c) = head.captures(line) {
                let rest = &c[2];
                if let Some((question, after)) = rest.split_once(" → ") {
                    if let Some(m) = marked.captures(after) {
                        out.push(Decided { when: c[1].to_string(), question: question.to_string(), id: m[1].to_string(), outcome: m[2].to_string() });
                    }
                }
            }
        }
    }
    out
}

/// The text only `--as-him` may write: the mark in the month log. Refused anywhere a caller's
/// words reach the log or the loops.
pub fn refuse_mark(what: &str, text: &str) -> R<()> {
    if rx(r"(?i)as\s+him\s*\[").is_match(text) {
        die!("{what} may not contain 'as him [' (only af needs close --as-him writes that mark); reword it: {}", life::one_line(text));
    }
    Ok(())
}

fn src_ids(text: &str) -> Vec<String> {
    rx(r"src-\d{4}-\d{2}-\d{2}-\d+").find_iter(text).map(|m| m.as_str().to_string()).collect()
}

fn log_dates(text: &str) -> Vec<String> {
    rx(r"(?i)\blog (\d{4}-\d{2}-\d{2})").captures_iter(text).map(|c| c[1].to_string()).collect()
}

/// A filed source under `workspace/life/sources/`: a `.md` file with that frontmatter `id`, or a
/// row in `sources/INDEX.md` (a binary source has only the row).
fn source_exists(ctx: &Ctx, id: &str) -> bool {
    let top = vpath(ctx, crate::life::SOURCES_REL);
    let row = format!("`{id}`");
    let mut found = false;
    pyos::walk(&top, &mut |dirpath, _, filenames| {
        for f in filenames {
            if found || !f.ends_with(".md") {
                continue;
            }
            let bytes = std::fs::read(pyos::join(dirpath, f)).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            let text = text.trim_start_matches('\u{feff}');
            if f == "INDEX.md" && dirpath == top {
                found = text.contains(&row);
            } else if let Some((front, _)) = fm::split_fm_optional(text) {
                found = fm::get_scalar(&front, "id").as_deref() == Some(id);
            }
        }
    });
    found
}

/// Whether the month logs hold a line dated `date` (`YYYY-MM-DD`) that is not a handover's own.
fn log_has_day(ctx: &Ctx, date: &str) -> bool {
    let Some(month) = date.get(..7) else {
        return false;
    };
    let bytes = std::fs::read(vpath(ctx, &life::month_log_rel(month))).unwrap_or_default();
    let prefix = format!("- {date} ");
    String::from_utf8_lossy(&bytes)
        .lines()
        .any(|l| l.starts_with(&prefix) && !l.contains(" · act_as_me_ended: ") && !l.contains(" · act_review: "))
}

/// The log line's outcome for a close made as him, or the refusal that says why not. The outcome
/// must lean on at least one real precedent (a filed source, or a day the month logs cover), and
/// never on the handover's own order.
pub fn as_him_outcome(ctx: &Ctx, outcome: &str) -> R<String> {
    let outcome = life::one_line(outcome);
    if outcome.is_empty() {
        die!("--outcome needs the result, in a few words");
    }
    refuse_mark("--outcome", &outcome)?;
    let Some(h) = active(ctx) else {
        die!("--as-him needs a handover in force; he starts one in his words with af act start (library/process/act-as-me.md)");
    };
    let (srcs, days) = (src_ids(&outcome), log_dates(&outcome));
    if srcs.is_empty() && days.is_empty() {
        die!("--as-him needs the outcome to name the precedent it leaned on: a source id (src-YYYY-MM-DD-NN) or 'log YYYY-MM-DD'; with none, leave the item for him");
    }
    for s in &srcs {
        if *s == h.from {
            die!("--as-him may not cite {s}: that is the order that handed over, not a past decision; cite the earlier one it leaned on");
        }
        if !source_exists(ctx, s) {
            die!("--as-him cites {s}, which is not a filed source; name a precedent that exists");
        }
    }
    for d in &days {
        if !log_has_day(ctx, d) {
            die!("--as-him cites 'log {d}', but no month-log line is dated {d}; name a precedent that exists");
        }
    }
    Ok(format!("as him [{}]: {outcome}", h.from))
}

/// The outcome of a plain `af needs close`. The mark text is refused. While a handover is in force
/// a plain close is his own answer, so it must cite an order filed after the handover started
/// (not the order that handed over); otherwise it is a decision for `--as-him` or for him.
pub fn plain_close_outcome(ctx: &Ctx, outcome: &str) -> R<String> {
    let outcome = life::one_line(outcome);
    refuse_mark("--outcome", &outcome)?;
    if let Some(h) = active(ctx) {
        let answered = src_ids(&outcome).iter().any(|id| {
            *id != h.from
                && find_order(ctx, id).is_some_and(|o| {
                    std::fs::metadata(&o.path)
                        .and_then(|m| m.modified())
                        .map(|t| chrono::DateTime::<chrono::Local>::from(t).naive_local() >= h.started)
                        .unwrap_or(false)
                })
        });
        if !answered {
            die!("a handover is in force: a close without --as-him must cite (src-...) an order he filed after it started ({}), his own answer. Use --as-him with a precedent, or file his answer first", life::fmt_min(h.started));
        }
    }
    Ok(outcome)
}

// ------------------------------------------------------------------ verbs

#[derive(Subcommand, Debug)]
pub enum ActCmd {
    /// Start a handover: act as him within the scope he gave, until a time
    Start {
        /// the filed order in which he handed over (src-YYYY-MM-DD-NN)
        #[arg(long)]
        from: String,
        /// what he handed over, in his words
        #[arg(long)]
        scope: String,
        /// when it ends: YYYY-MM-DDTHH:MM or +Nh; default the next 09:00
        #[arg(long)]
        until: Option<String>,
    },
    /// End the handover now and log it
    Stop {
        #[arg(long)]
        outcome: Option<String>,
    },
    /// Say whether a handover is in force
    Status,
    /// Mark the decisions made as him as seen
    Review,
}

pub fn run(ctx: &Ctx, cmd: ActCmd) -> R<i32> {
    match cmd {
        ActCmd::Start { from, scope, until } => cmd_start(ctx, &from, &scope, until.as_deref()),
        ActCmd::Stop { outcome } => cmd_stop(ctx, outcome.as_deref()),
        ActCmd::Status => cmd_status(ctx),
        ActCmd::Review => cmd_review(ctx),
    }
}

/// A filed order: a file under `sources/orders/` whose frontmatter has that `id` and `type: order`.
struct Order {
    path: String,
    /// The frontmatter `date`, when it has one.
    date: Option<String>,
    body: String,
}

fn find_order(ctx: &Ctx, id: &str) -> Option<Order> {
    let dir = vpath(ctx, ORDERS_REL);
    let rd = std::fs::read_dir(&dir).ok()?;
    for e in rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".md")) {
        let bytes = std::fs::read(e.path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim_start_matches('\u{feff}');
        let Some((f, body)) = fm::split_fm_optional(text) else {
            continue;
        };
        if fm::get_scalar(&f, "id").as_deref() == Some(id) && fm::get_scalar(&f, "type").as_deref() == Some("order") {
            return Some(Order { path: e.path().to_string_lossy().into_owned(), date: fm::get_scalar(&f, "date"), body });
        }
    }
    None
}

/// Lower case, whitespace collapsed, surrounding quotes stripped: how a scope is compared to an order.
fn norm_words(s: &str) -> String {
    let one = life::one_line(s).to_lowercase();
    one.trim_matches(|c: char| matches!(c, '"' | '\'' | '\u{201c}' | '\u{201d}' | '\u{2018}' | '\u{2019}')).trim().to_string()
}

/// The order must be recent and must contain the scope in his words.
fn check_order(ctx: &Ctx, id: &str, scope: &str) -> R<()> {
    let Some(order) = find_order(ctx, id) else {
        die!("--from {id} is not a filed order under {ORDERS_REL}/; file his words first (af source add --type order), then start the handover from its id");
    };
    let today = crate::core::today_date();
    let recent = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok_and(|d| d == today || d == today - Duration::days(1));
    let id_date = id.strip_prefix("src-").and_then(|r| r.get(..10)).unwrap_or("");
    if !(recent(id_date) || order.date.as_deref().is_some_and(recent)) {
        die!("--from {id} is not from today or yesterday; a handover starts from the order in which he just handed over, so file his words first");
    }
    if !norm_words(&order.body).contains(&norm_words(scope)) {
        die!("--scope \"{scope}\" does not appear in {id}; the scope is his words from that order, quoted");
    }
    Ok(())
}

pub const MAX_DAYS: i64 = 7;

/// `--until`: an ISO minute or `+Nh`; the default is the next 09:00. Always in the future.
fn parse_until(now: NaiveDateTime, until: Option<&str>) -> R<NaiveDateTime> {
    let t = match until.map(str::trim) {
        None => {
            let nine = now.date().and_hms_opt(9, 0, 0).unwrap_or(now);
            if now < nine {
                nine
            } else {
                nine + Duration::days(1)
            }
        }
        Some(v) => {
            if let Some(c) = rx(r"\A\+(\d{1,4})h\z").captures(v) {
                now + Duration::hours(c[1].parse::<i64>().unwrap_or(0))
            } else if let Ok(t) = NaiveDateTime::parse_from_str(v, TIME_FMT) {
                t
            } else {
                die!("--until takes YYYY-MM-DDTHH:MM or +Nh, got '{v}'");
            }
        }
    };
    if t <= now {
        die!("--until {} is not in the future (it is {} now)", life::fmt_min(t), life::fmt_min(now));
    }
    Ok(t)
}

fn cmd_start(ctx: &Ctx, from: &str, scope: &str, until: Option<&str>) -> R<i32> {
    let scope = life::one_line(scope);
    if scope.is_empty() {
        die!("--scope needs what he handed over, in his words");
    }
    let from = from.trim();
    if !rx(r"\Asrc-\d{4}-\d{2}-\d{2}-\d+\z").is_match(from) {
        die!("--from {from} is not a filed order under {ORDERS_REL}/; file his words first (af source add --type order), then start the handover from its id");
    }
    check_order(ctx, from, &scope)?;
    let now = life::now();
    let until = parse_until(now, until)?;
    if until - now > Duration::days(MAX_DAYS) {
        die!("--until {} is more than {MAX_DAYS} days away; the cap is {MAX_DAYS} days from now ({}). Start a new handover when it ends", life::fmt_min(until), life::fmt_min(now));
    }
    // An expired or unreadable handover is logged as ended before the new one replaces it.
    settle_expired(ctx);
    discard_unreadable(ctx)?;
    let new = Handover { from: from.to_string(), scope, started: now, until };
    let old = active(ctx);
    if let Some(o) = &old {
        life::append_log(ctx, "act_as_me_ended", &format!("replaced {}", o.tag()))?;
    }
    TextFile::new(&new.file_text()).save_atomic(&vpath(ctx, STATE_REL))?;
    if let Some(o) = old {
        outln!("af act start: replaced the handover {} that ran until {}", o.tag(), life::fmt_min(o.until));
    }
    outln!("af act start: acting as him until {}, scope: \"{}\" [{}]", life::fmt_min(until), new.scope, new.from);
    Ok(0)
}

fn cmd_stop(ctx: &Ctx, outcome: Option<&str>) -> R<i32> {
    let Some(h) = load(ctx) else {
        match discard_unreadable(ctx)? {
            Some(rel) => outln!("af act stop: removed a handover file that could not be read (logged in {rel})"),
            None => outln!("af act stop: not acting as him"),
        }
        return Ok(0);
    };
    let how = if h.in_force(life::now()) { "stopped" } else { "expired" };
    let mut text = format!("{how} {}", h.tag());
    if let Some(o) = outcome.map(life::one_line).filter(|o| !o.is_empty()) {
        text.push_str(&format!(" → {o}"));
    }
    let rel = life::append_log(ctx, "act_as_me_ended", &text)?;
    std::fs::remove_file(vpath(ctx, STATE_REL)).map_err(|e| pyos::os_error(&e, STATE_REL))?;
    outln!("af act stop: no longer acting as him ({how} {}; logged in {rel})", h.tag());
    Ok(0)
}

fn cmd_status(ctx: &Ctx) -> R<i32> {
    match active(ctx) {
        Some(h) => outln!("acting as him until {}, scope: \"{}\" [{}]", life::fmt_min(h.until), h.scope, h.from),
        None => outln!("not acting as him"),
    }
    Ok(0)
}

fn cmd_review(ctx: &Ctx) -> R<i32> {
    let seen = unseen(ctx);
    if seen.is_empty() {
        outln!("af act review: nothing decided as him is waiting");
        return Ok(0);
    }
    let rel = life::append_log(ctx, "act_review", &format!("{} decided as him seen", seen.len()))?;
    crate::needs::refresh(ctx);
    outln!("af act review: {} decided as him marked seen (logged in {rel})", seen.len());
    Ok(0)
}
