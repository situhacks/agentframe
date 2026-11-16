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
    let close = rx(r"\A- (\d{4}-\d{2}-\d{2} \d{2}:\d{2}) · closed: (.*?) → as him \[([^\]\s]+)\]: (.*)\z");
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
            } else if let Some(c) = close.captures(line) {
                out.push(Decided { when: c[1].to_string(), question: c[2].to_string(), id: c[3].to_string(), outcome: c[4].to_string() });
            }
        }
    }
    out
}

/// What an `--as-him` outcome must carry: a source id, or `log` and a date.
pub fn names_precedent(outcome: &str) -> bool {
    rx(r"src-\d{4}-\d{2}-\d{2}-\d+").is_match(outcome) || rx(r"(?i)\blog \d{4}-\d{2}-\d{2}").is_match(outcome)
}

/// The log line's outcome for a close made as him, or the refusal that says why not.
pub fn as_him_outcome(ctx: &Ctx, outcome: &str) -> R<String> {
    let outcome = life::one_line(outcome);
    if outcome.is_empty() {
        die!("--outcome needs the result, in a few words");
    }
    let Some(h) = active(ctx) else {
        die!("--as-him needs a handover in force; he starts one in his words with af act start (library/process/act-as-me.md)");
    };
    if !names_precedent(&outcome) {
        die!("--as-him needs the outcome to name the precedent it leaned on: a source id (src-YYYY-MM-DD-NN) or 'log YYYY-MM-DD'; with none, leave the item for him");
    }
    Ok(format!("as him [{}]: {outcome}", h.from))
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

/// Whether `id` is a filed order: a file under `sources/orders/` whose frontmatter has that `id`
/// and `type: order`.
fn is_filed_order(ctx: &Ctx, id: &str) -> bool {
    let dir = vpath(ctx, ORDERS_REL);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return false;
    };
    rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".md")).any(|e| {
        let bytes = std::fs::read(e.path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim_start_matches('\u{feff}');
        fm::split_fm_optional(text).is_some_and(|(f, _)| {
            fm::get_scalar(&f, "id").as_deref() == Some(id) && fm::get_scalar(&f, "type").as_deref() == Some("order")
        })
    })
}

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
    if !rx(r"\Asrc-\d{4}-\d{2}-\d{2}-\d+\z").is_match(from) || !is_filed_order(ctx, from) {
        die!("--from {from} is not a filed order under {ORDERS_REL}/; file his words first (af source add --type order), then start the handover from its id");
    }
    let now = life::now();
    let until = parse_until(now, until)?;
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
        outln!("af act stop: not acting as him");
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
