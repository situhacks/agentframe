//! `af session warm <name>`: a chat keeps its own prompt cache warm through a long wait.
//!
//! Claude Code writes a chat's cache with a 1-hour TTL, and any request that reads it resets the hour.
//! A chat cannot start a turn in itself, but a background command it started wakes it when the command
//! exits. So the chat runs this verb with `run_in_background`: it checks the chat is worth warming, waits
//! until the chat has been idle for `--minutes`, prints what to do next, and exits. That exit is the wake,
//! and the wake turn reads the whole prefix from cache for a small fraction of what a rebuild writes.
//!
//! The wait is for idleness, not wall time: every poll reads the timestamp of the chat's last main-chain
//! API response, so any activity (he typed, an agent finished) already refreshed the cache and pushes the
//! fire time back by itself. One lock file per session (`system/logs/warm/<session id>.json`) allows one
//! live timer; a dead process's lock is taken over, and the lock is removed on every exit path of the
//! process that took it. Each run appends one line to `system/logs/warm/warm.log`.
//!
//! Test hook, not for use: `AF_WARM_MINUTE_SECONDS` is the length of a minute in seconds (default 60). It
//! scales the minute, the 30-second poll and the 12-hour cap together, so a test runs in seconds.

use crate::core::Ctx;
use crate::error::{refuse, AfError, R};
use crate::life::vpath;
use crate::outln;
use crate::panel;
use crate::reach::{self, CacheKind, ContextSize};
use crate::sessions::Registry;
use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_MINUTES: u64 = 55;
pub const MIN_MINUTES: u64 = 5;
pub const MAX_MINUTES: u64 = 58;
pub const DEFAULT_MAX_WAKES: u32 = 12;
pub const DEFAULT_MIN_TOKENS: u64 = 150_000;
/// Seconds between two reads of the transcript.
const POLL_SECS: f64 = 30.0;
/// Minutes after which an armed timer ends by itself.
const ARMED_CAP_MINUTES: f64 = 12.0 * 60.0;
/// Test hook: seconds in a minute.
const MINUTE_ENV: &str = "AF_WARM_MINUTE_SECONDS";

/// What `af session warm` was asked.
pub struct Opts<'a> {
    pub name: &'a str,
    pub wake: u32,
    pub minutes: u64,
    pub max_wakes: u32,
    pub min_tokens: u64,
}

fn minute_secs() -> f64 {
    std::env::var(MINUTE_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|s| *s > 0.0 && s.is_finite()).unwrap_or(60.0)
}

fn k(tokens: u64) -> String {
    format!("{}k", (tokens + 500) / 1000)
}

// ------------------------------------------------------------------ the lock

fn warm_dir(ctx: &Ctx) -> PathBuf {
    PathBuf::from(vpath(ctx, "system/logs/warm"))
}

fn lock_path(ctx: &Ctx, session_id: &str) -> PathBuf {
    let safe: String = session_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    warm_dir(ctx).join(format!("{safe}.json"))
}

/// This process's claim on a chat. Dropping it removes the file, but only while it still names this process.
struct Lock {
    path: PathBuf,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let mine = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("pid").and_then(Value::as_u64) == Some(std::process::id() as u64));
        if mine {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

enum Take {
    Got(Lock),
    /// A live timer holds it: its pid and when it was armed.
    Held(u32, String),
}

fn live_holder(text: &str) -> Option<(u32, String)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let pid = u32::try_from(v.get("pid").and_then(Value::as_u64)?).ok()?;
    if !panel::process_is(pid, panel::start_of(&v, "pid_start")) {
        return None;
    }
    Some((pid, v.get("armed").and_then(Value::as_str).unwrap_or("").to_string()))
}

/// Create the lock file only when absent. A file left by a dead process is removed and the creation
/// retried, so of two verbs started at the same moment exactly one creates it.
fn take_lock(ctx: &Ctx, session_id: &str, wake: u32) -> Result<Take, String> {
    use std::io::Write;
    let path = lock_path(ctx, session_id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let me = std::process::id();
    let body = json!({"pid": me, "pid_start": panel::pid_start(me), "armed": crate::core::now_iso(), "wake": wake}).to_string();
    for _ in 0..4 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(body.as_bytes()) {
                    drop(f);
                    let _ = std::fs::remove_file(&path);
                    return Err(format!("cannot write {}: {e}", path.display()));
                }
                return Ok(Take::Got(Lock { path }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("cannot write {}: {e}", path.display())),
        }
        let Ok(seen) = std::fs::read_to_string(&path) else { continue };
        if let Some((pid, armed)) = live_holder(&seen) {
            return Ok(Take::Held(pid, armed));
        }
        // Left over: removed only when it is still what was judged left over.
        if std::fs::read_to_string(&path).is_ok_and(|now| now == seen) {
            let _ = std::fs::remove_file(&path);
        }
    }
    Err(format!("cannot take {}: it keeps changing", path.display()))
}

/// `HH:MM` local time of an RFC 3339 stamp, or the stamp as written.
fn clock(stamp: &str) -> String {
    DateTime::parse_from_rfc3339(stamp).map(|t| t.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_else(|_| stamp.to_string())
}

// ------------------------------------------------------------------ the log

/// One line per run in `system/logs/warm/warm.log`: time, name, wake, outcome, idle minutes, context tokens.
fn log(ctx: &Ctx, o: &Opts, outcome: &str, idle: Option<u64>, tokens: Option<u64>) {
    use std::io::Write;
    let dir = warm_dir(ctx);
    let _ = std::fs::create_dir_all(&dir);
    let flat = outcome.split_whitespace().collect::<Vec<_>>().join(" ");
    let show = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
    let line = format!("{} name={} wake={} outcome={flat} idle={} context={}\n", crate::core::now_iso(), o.name, o.wake, show(idle), show(tokens));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("warm.log")) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// A stop: its line is printed, logged, and the run ends with exit 0 (the chat reads it on its next turn).
fn stop(ctx: &Ctx, o: &Opts, reason: &str, idle: Option<u64>, tokens: Option<u64>) -> R<i32> {
    outln!("stop: {reason}");
    log(ctx, o, reason, idle, tokens);
    Ok(0)
}

// ------------------------------------------------------------------ reading the transcript

/// Run `find` over a growing tail of the transcript until it answers.
fn find_in_tail<T>(path: &str, find: impl Fn(&str) -> Option<T>) -> Option<T> {
    for size in reach::TAIL_SIZES {
        if let Some(v) = reach::tail_of(path, size).and_then(|t| find(&t)) {
            return Some(v);
        }
    }
    None
}

/// When the chat's cache was last refreshed: its newest main-chain reply, else the file's own mtime.
/// None when the file is gone.
fn last_activity(path: &str) -> Option<DateTime<Utc>> {
    let meta = std::fs::metadata(path).ok()?;
    Some(find_in_tail(path, reach::last_reply_at).or_else(|| meta.modified().ok().map(DateTime::<Utc>::from)).unwrap_or_else(Utc::now))
}

/// The command that re-arms the timer, carrying every option that is not the default.
fn rearm_command(o: &Opts) -> String {
    let mut c = format!("af session warm {} --wake {}", o.name, o.wake + 1);
    if o.minutes != DEFAULT_MINUTES {
        c.push_str(&format!(" --minutes {}", o.minutes));
    }
    if o.max_wakes != DEFAULT_MAX_WAKES {
        c.push_str(&format!(" --max-wakes {}", o.max_wakes));
    }
    if o.min_tokens != DEFAULT_MIN_TOKENS {
        c.push_str(&format!(" --min-tokens {}", o.min_tokens));
    }
    c
}

fn usage_error(msg: String) -> AfError {
    AfError::Usage(refuse(msg).to_string())
}

// ------------------------------------------------------------------ the verb

pub fn cmd_warm(ctx: &Ctx, o: &Opts) -> R<i32> {
    if !(MIN_MINUTES..=MAX_MINUTES).contains(&o.minutes) {
        return Err(usage_error(format!(
            "--minutes is {MIN_MINUTES} to {MAX_MINUTES}, got {}: the cache lives one hour, so the wake must come inside it",
            o.minutes
        )));
    }
    if o.wake < 1 {
        return Err(usage_error("--wake counts from 1".to_string()));
    }
    let reg = Registry::load(ctx)?;
    let Some(row) = reg.find_name(o.name) else {
        return Err(usage_error(format!(
            "no registry row for {}. af session list shows the names",
            o.name
        )));
    };
    let name = o.name;

    if o.wake > o.max_wakes {
        return stop(ctx, o, &format!("{} wakes reached; let the cache lapse, or compact if the task is done", o.max_wakes), None, None);
    }

    let floor = k(o.min_tokens);
    let known = reach::transcript_path(&row.session_id).and_then(|p| match reach::context_size(&p) {
        ContextSize::Known(u) => Some((p, u.tokens)),
        _ => None,
    });
    let Some((path, tokens)) = known else {
        let why = format!("context is unknown (no reply with usage yet, or compacted since the last one), below {floor}: a cold restart rewrites little; not warming");
        return stop(ctx, o, &why, None, None);
    };
    if tokens < o.min_tokens {
        return stop(ctx, o, &format!("context is {}, below {floor}: a cold restart rewrites little; not warming", k(tokens)), None, Some(tokens));
    }

    let cache = find_in_tail(&path, |t| match reach::cache_kind(t) {
        CacheKind::Unknown => None,
        known => Some(known),
    })
    .unwrap_or(CacheKind::Unknown);
    if cache == CacheKind::FiveMinute {
        return stop(
            ctx,
            o,
            "this chat is on the 5-minute cache (usage overage); warming would need a wake every 4 minutes. If the task in hand is done, compact instead.",
            None,
            Some(tokens),
        );
    }

    let _lock = match take_lock(ctx, &row.session_id, o.wake) {
        Ok(Take::Got(l)) => l,
        Ok(Take::Held(pid, armed)) => {
            return stop(ctx, o, &format!("already armed since {} (pid {pid}); nothing started", clock(&armed)), None, Some(tokens));
        }
        Err(m) => {
            log(ctx, o, &m, None, Some(tokens));
            return Err(refuse(format!("{name}: {m}")));
        }
    };

    // From here the lock is held, and it goes with this function's return, whatever the outcome.
    let scale = minute_secs();
    let poll = Duration::from_secs_f64(POLL_SECS * scale / 60.0);
    let armed_at = Utc::now();
    let armed_len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let fire_after = o.minutes as f64 * scale;
    let cap = ARMED_CAP_MINUTES * scale;
    loop {
        let Some(last) = last_activity(&path) else {
            log(ctx, o, "its transcript is gone", None, Some(tokens));
            return Err(refuse(format!("{name}: its transcript {path} is gone; stopped warming")));
        };
        let now = Utc::now();
        let idle = (now - last).num_milliseconds() as f64 / 1000.0;
        let idle_min = (idle / scale).floor().max(0.0) as u64;
        if crate::compact::appended_since(&path, armed_len).0 {
            return stop(ctx, o, "this chat compacted; nothing to keep warm", Some(idle_min), Some(tokens));
        }
        if idle >= fire_after {
            // The size now: the chat may have grown while it waited.
            let tokens = reach::context_tokens(&path).unwrap_or(tokens);
            let assumed = if cache == CacheKind::OneHour { "cache 1h" } else { "cache 1h (assumed: no cache_creation data)" };
            outln!("warm: wake {} of {} for {name} · idle {idle_min} min · context {} · {assumed}", o.wake, o.max_wakes, k(tokens));
            outln!("Still waiting on work you will return to? Re-arm in the background: {}", rearm_command(o));
            outln!("Task in hand done? Compact instead: af session compact {name} --when-idle");
            log(ctx, o, "fired", Some(idle_min), Some(tokens));
            return Ok(0);
        }
        if (now - armed_at).num_milliseconds() as f64 / 1000.0 >= cap {
            return stop(ctx, o, "armed 12 hours; ended", Some(idle_min), Some(tokens));
        }
        std::thread::sleep(poll);
    }
}
