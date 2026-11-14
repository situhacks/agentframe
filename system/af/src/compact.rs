//! `af session compact <name>`: compact a chat that has no live process, from outside it, with
//! `claude -p "/compact ..." --resume <id>`. The project's `pre-compact` hook adds the standing
//! summary instructions and `post-compact` re-arms the nudges. A chat with a live process is
//! never compacted this way: two processes would write one transcript, so the guard fails
//! closed and is checked again after the run. A running chat is compacted through the VS Code
//! panel instead: the panel driver sends `/compact ...` to it once the operator and the chat are
//! idle, and the transcript must gain a `compact_boundary` line. `--when-idle` starts a detached
//! waiter that does this once the chat finishes its turn (one pending waiter per chat; it chooses its
//! route when it fires, and does nothing if the chat was compacted meanwhile); `--then-file` sends a
//! file's text to the chat as its next message after the compaction. While a panel driver run holds
//! the panel lock for the chat, the chat counts as running, so the closed route never races it; and the
//! closed route holds the same lock for its whole run, so no driver run opens the chat meanwhile.
//! `--cancel` stops a queued `--when-idle` compaction: it writes a cancel marker next to the job file, which
//! the waiter and its driver run look for while they only wait, and says what happened.

use crate::core::Ctx;
use crate::die;
use crate::error::{AfError, R};
use crate::outln;
use crate::panel;
use crate::pyos;
use crate::proc::{claude_bin, run_command, Out};
use crate::reach;
use crate::sessions::{self, Registry};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;
use std::time::{Duration, Instant};

/// A chat under this many context tokens has nothing worth summarising.
pub const MIN_TOKENS: u64 = 50_000;
/// The longest a compaction may run.
const DEADLINE: Duration = Duration::from_secs(15 * 60);
/// Test hook: seconds (fractions allowed) that replace the 15 minute deadline.
const DEADLINE_ENV: &str = "AF_COMPACT_DEADLINE_SECS";
/// How much of a failed command's output the refusal shows.
const OUTPUT_SHOWN: usize = 300;
/// The longest the transcript is given to show a `compact_boundary` line after the panel sent `/compact`.
pub const BOUNDARY_WAIT_SECS: u64 = 10 * 60;
const BOUNDARY_WAIT: Duration = Duration::from_secs(BOUNDARY_WAIT_SECS);
/// How long the closed route waits for the panel lock when no waiter's window says otherwise.
pub const LOCK_WAIT_SECS: u64 = 10 * 60;
/// Test hook: seconds that replace LOCK_WAIT_SECS.
const LOCK_WAIT_ENV: &str = "AF_LOCK_WAIT_SECS";
/// Test hook: seconds (fractions allowed) that replace the 10 minute boundary wait.
const BOUNDARY_ENV: &str = "AF_COMPACT_BOUNDARY_SECS";
/// How long a `--when-idle` waiter waits for the chat and the operator when `--wait-minutes` is not given.
const WHEN_IDLE_MINUTES: f64 = 60.0;

fn k(tokens: u64) -> String {
    format!("{}k", (tokens + 500) / 1000)
}

/// The command's arguments after the program name.
fn command_args(instructions: Option<&str>, session_id: &str, model: &str) -> Vec<String> {
    let prompt = match instructions.map(str::trim).filter(|i| !i.is_empty()) {
        Some(i) => format!("/compact {i}"),
        None => "/compact".to_string(),
    };
    ["-p", &prompt, "--resume", session_id, "--model", model, "--output-format", "json"].iter().map(|s| s.to_string()).collect()
}

fn shown(arg: &str) -> String {
    if arg.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// Whether a `compact_boundary` system line was appended after byte `from`, and the last
/// assistant usage in what was appended.
fn appended_since(path: &str, from: u64) -> (bool, Option<reach::Usage>) {
    let read = || -> Option<String> {
        let mut f = std::fs::File::open(path).ok()?;
        let len = f.metadata().ok()?.len();
        f.seek(SeekFrom::Start(from.min(len))).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
    };
    let text = read().unwrap_or_default();
    let boundary = text.lines().filter(|l| l.contains("compact_boundary")).any(|l| {
        serde_json::from_str::<Value>(l)
            .map(|v| v.get("type").and_then(Value::as_str) == Some("system") && v.get("subtype").and_then(Value::as_str) == Some("compact_boundary"))
            .unwrap_or(false)
    });
    (boundary, reach::last_usage(&text))
}

/// The one JSON object the command printed.
fn parse_result(stdout: &str) -> Option<Value> {
    let whole = stdout.trim();
    serde_json::from_str::<Value>(whole)
        .ok()
        .or_else(|| whole.lines().rev().find(|l| !l.trim().is_empty()).and_then(|l| serde_json::from_str(l.trim()).ok()))
        .filter(Value::is_object)
}

fn head(text: &str) -> String {
    let cut: String = text.trim().chars().take(OUTPUT_SHOWN).collect();
    if cut.is_empty() {
        "(no output)".to_string()
    } else {
        cut
    }
}

fn deadline() -> Duration {
    std::env::var(DEADLINE_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or(DEADLINE)
}

fn span(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= 60.0 && (secs % 60.0) == 0.0 {
        format!("{} minutes", secs / 60.0)
    } else if secs == 1.0 {
        "1 second".to_string()
    } else {
        format!("{secs} seconds")
    }
}

// ------------------------------------------------------------------ is the chat running

/// What the machine says about the chat's process. Only `Closed` allows a compaction.
enum Live {
    Closed,
    /// A process may be writing the transcript, and why we think so.
    Running(String),
    /// No roster, so a process cannot be ruled out.
    Unknown,
}

/// Fail closed: the chat is closed only when no live panel driver run holds the panel lock for it
/// (between its tab's close and reopen the chat has no process, yet it is not closed), its session id
/// is absent from the roster (`claude agents --json --all`; any entry counts, whatever its status)
/// and no record under the config dir's `sessions/` names it. A record that does not parse is ignored.
fn live_check(ctx: &Ctx, session_id: &str) -> Live {
    if let Some(pid) = panel::lock_holder(ctx, session_id) {
        return Live::Running(format!("the panel driver (pid {pid}) holds the panel lock for it and is working on it"));
    }
    let Some(roster) = sessions::claude_roster() else { return Live::Unknown };
    let entry = roster.as_array().into_iter().flatten().find(|e| e.get("sessionId").and_then(Value::as_str) == Some(session_id));
    if let Some(e) = entry {
        let status = e.get("status").and_then(Value::as_str).unwrap_or("none");
        return Live::Running(format!("the claude agents roster lists it with status {status}"));
    }
    let dir = reach::config_dir().map(|c| pyos::join(&c, "sessions"));
    for e in dir.and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten().flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let named = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| v.get("sessionId").and_then(Value::as_str) == Some(session_id));
        if named {
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            return Live::Running(format!("the session record sessions/{file} names it"));
        }
    }
    Live::Closed
}

// ------------------------------------------------------------------ the summary model

fn base(id: &str) -> &str {
    id.split('[').next().unwrap_or(id)
}

/// A bracketed model id in a record, whatever kind of record holds it.
fn bracketed(rec: &Value) -> Option<(&str, &str)> {
    let kind = rec.get("type").and_then(Value::as_str).unwrap_or("record");
    [rec.get("requestedModel"), rec.pointer("/attachment/model"), rec.pointer("/attachment/identity/modelId")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|m| m.contains('['))
        .map(|m| (kind, m))
}

/// The id to summarise with and where it came from. The last main-chain reply's own
/// `requestedModel` carries the `[1m]` suffix when the chat runs on the large window (its
/// `message.model` never does). A reply with no `requestedModel` takes the most recent bracketed
/// id of the same base in the transcript tail.
fn default_model(usage: &reach::Usage, tail: &str) -> Option<(String, String)> {
    let answered = usage.model.as_deref().filter(|m| !m.starts_with('<'));
    let want = answered.map(base).or_else(|| usage.requested.as_deref().map(base))?;
    if let Some(r) = usage.requested.as_deref().filter(|r| base(r) == want) {
        return Some((r.to_string(), "requestedModel of its last reply".to_string()));
    }
    for line in tail.rsplit('\n') {
        if !line.contains('[') {
            continue;
        }
        let Ok(rec) = serde_json::from_str::<Value>(line) else { continue };
        if let Some((kind, id)) = bracketed(&rec).filter(|(_, id)| base(id) == want) {
            return Some((id.to_string(), format!("a recent {kind} record")));
        }
    }
    answered.map(|m| (m.to_string(), "the model of its last reply".to_string()))
}

/// What `af session compact` was asked.
pub struct Opts<'a> {
    pub name: &'a str,
    pub model: Option<&'a str>,
    pub instructions: Option<&'a str>,
    pub force: bool,
    pub dry_run: bool,
    pub when_idle: bool,
    pub wait_minutes: Option<f64>,
    pub then_file: Option<&'a str>,
    pub waiter_job: Option<&'a str>,
    /// `--cancel`: stop the chat's queued `--when-idle` compaction.
    pub cancel: bool,
}

pub fn cmd_compact(ctx: &Ctx, o: &Opts) -> R<i32> {
    if let Some(job) = o.waiter_job {
        return run_waiter(ctx, job);
    }
    if o.cancel {
        let (report, code) = cancel_report(ctx, o.name)?;
        outln!("{report}");
        return Ok(code);
    }
    let report = compact_report(ctx, o)?;
    outln!("{report}");
    Ok(0)
}

/// The text of a `--then-file`: read when it is needed, so a file written after the verb started
/// is read as it is then.
fn then_text(path: &str) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(t) if !t.trim().is_empty() => Ok(t),
        Ok(_) => Err(format!("--then-file {path} is empty")),
        Err(e) => Err(format!("cannot read --then-file {path}: {e}")),
    }
}

fn compact_report(ctx: &Ctx, o: &Opts) -> R<String> {
    let name = o.name;
    let reg = Registry::load(ctx)?;
    let row = reach::row_or_refuse(&reg, name)?;
    if let Some(tf) = o.then_file {
        if let Err(m) = then_text(tf) {
            die!("{name}: {m}");
        }
    }

    let running = match live_check(ctx, &row.session_id) {
        Live::Closed => None,
        Live::Unknown => die!("{name}: cannot tell whether it is running, the claude agents roster is unavailable; nothing was compacted"),
        Live::Running(why) => Some(why),
    };

    let Some(path) = reach::transcript_path(&row.session_id) else {
        die!("{name}: no transcript found for session {}; nothing to compact", row.session_id);
    };
    let mut after_compact = false;
    let usage = match reach::context_size(&path) {
        reach::ContextSize::Known(u) => Some(u),
        reach::ContextSize::AfterCompact if !o.force => {
            return Ok(format!("{name}: already compact. Its transcript ends at a compact_boundary with no reply since, so nothing new exists to compact. --force runs it anyway."));
        }
        reach::ContextSize::AfterCompact => {
            after_compact = true;
            None
        }
        reach::ContextSize::Unknown => None,
    };
    let tokens = usage.as_ref().map(|u| u.tokens);
    match tokens {
        Some(t) if t < MIN_TOKENS && !o.force => {
            return Ok(format!("{name}: context is {}, under {}; nothing worth compacting (--force overrides).", k(t), k(MIN_TOKENS)));
        }
        None if !o.force => die!("{name}: its transcript has no assistant usage line, so its size is unknown; --force compacts anyway"),
        _ => {}
    }

    if let Some(why) = running {
        return through_panel(ctx, o, &row, &path, tokens, &why);
    }

    let chosen = match o.model.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => Some((m.to_string(), "--model".to_string())),
        None => {
            let tail = reach::tail(&path).unwrap_or_default();
            // After a compaction the size is unknown, but the reply before it still names the model.
            let named = if after_compact { reach::last_usage(&tail) } else { usage.clone() };
            named.as_ref().and_then(|u| default_model(u, &tail))
        }
    };
    let Some((model, source)) = chosen else {
        die!("{name}: its transcript records no model; pass --model <id>");
    };
    if model.to_lowercase().contains("haiku") && !o.force {
        die!("{name}: {model} would write the summary, and a summary is the chat's whole memory; pass a larger --model, or --force");
    }

    let args = command_args(o.instructions, &row.session_id, &model);
    let before = tokens.map(k).unwrap_or_else(|| "unknown".to_string());
    let bin = claude_bin();
    if o.dry_run {
        let line: Vec<String> = args.iter().map(|a| shown(a)).collect();
        let mut text = format!("{name}: would run from {}: {} {}\nmodel {model} (from {source}), context before {before} tokens.", ctx.root, shown(&bin), line.join(" "));
        if let Some(tf) = o.then_file {
            text.push_str(&format!("\nthen it would send the text of {tf} to the chat through the panel driver, as its next message."));
        }
        return Ok(text);
    }

    // The panel lock, for the whole run: for about a second after `claude -p` starts there is no
    // session record, and a driver run that opened the chat then would start a second process on it.
    let lock = match panel::acquire_lock_within(ctx, &row.session_id, "headless-compact", lock_budget()) {
        Ok(l) => l,
        Err(m) => die!("{name}: nothing was compacted: {m}"),
    };
    // Checked again under the lock: the chat may have been opened since the first check.
    match live_check(ctx, &row.session_id) {
        Live::Closed => {}
        Live::Unknown => die!("{name}: cannot tell whether it is running, the claude agents roster is unavailable; nothing was compacted"),
        Live::Running(why) => die!("{name}: it is running now ({why}); nothing was compacted"),
    }
    // From here the compaction changes things: a queued cancel is honoured only before this point.
    if let Err(m) = panel::begin_changes() {
        die!("{name}: {m}; nothing was compacted");
    }
    let len_before = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let limit = window_cap(deadline());
    let mut cmd = Command::new(&bin);
    cmd.args(&args).current_dir(&ctx.root);
    let out = run_command(cmd, "claude", Some(Instant::now() + limit), true);
    let result = judge(&out, &path, len_before, limit);
    let note = after_note(ctx, &result, &row.session_id, name)?;
    // Let go before the --then-file send, which is a driver run and needs the lock.
    drop(lock);
    match result {
        Ok((cost, after)) => {
            let after = match after {
                Some(u) => format!("{} tokens", k(u.tokens)),
                None => "shown at its next turn".to_string(),
            };
            let mut report = format!("{name}: compacted. Before {before} tokens, model {model} (from {source}), cost {cost}; context after: {after}.{note}");
            report.push_str(&send_then(ctx, o, &row.session_id)?);
            Ok(report)
        }
        Err(why) => die!("{name}: compaction failed, {why}"),
    }
}

/// After a confirmed compaction: the `--then-file` text as the chat's next message, under the
/// driver's guards. The sentence to append to the report, empty with no `--then-file`.
fn send_then(ctx: &Ctx, o: &Opts, session_id: &str) -> R<String> {
    let Some(tf) = o.then_file else { return Ok(String::new()) };
    let name = o.name;
    let text = match then_text(tf) {
        Ok(t) => t,
        Err(m) => die!("{name}: compacted, but the next message was not sent: {m}"),
    };
    let wait = match driver_wait(o.wait_minutes) {
        Ok(w) => w,
        Err(m) => die!("{name}: compacted, but the next message ({tf}) was not sent: {m}"),
    };
    match panel::send(ctx, session_id, &text, wait) {
        Ok(a) => Ok(format!(" Sent the text of {tf} as its next message.{}", a.notes_sentence())),
        Err(m) => die!("{name}: compacted, but the next message ({tf}) was not sent: {m}"),
    }
}

// ------------------------------------------------------------------ a running chat, through the panel

fn boundary_wait() -> Duration {
    std::env::var(BOUNDARY_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or(BOUNDARY_WAIT)
}

/// Compact a chat that is running: the panel driver sends `/compact ...` to it, and the
/// transcript has to show the boundary.
fn through_panel(ctx: &Ctx, o: &Opts, row: &sessions::Row, path: &str, tokens: Option<u64>, why: &str) -> R<String> {
    let name = o.name;
    let text = match o.instructions.map(str::trim).filter(|i| !i.is_empty()) {
        Some(i) => format!("/compact {i}"),
        None => "/compact".to_string(),
    };
    let before = tokens.map(k).unwrap_or_else(|| "unknown".to_string());
    let note = if o.model.is_some() { " --model has no effect here: the chat summarises on its own model." } else { "" };
    let wait = if o.when_idle { Some(o.wait_minutes.unwrap_or(WHEN_IDLE_MINUTES)) } else { o.wait_minutes };
    if o.dry_run {
        let d = panel::driver(ctx)?;
        let call = panel::Call {
            action: "send",
            session: Some(&row.session_id),
            text_file: Some(std::path::Path::new("<a file holding the text>")),
            wait_minutes: wait,
            ..panel::Call::default()
        };
        let mut lines = vec![format!("{name}: running ({why}), so it is compacted through the panel, not from outside.")];
        if o.when_idle {
            lines.push(format!("would start a detached waiter, which runs this once the chat is idle, within {} minutes (to cancel it: af session compact --cancel {name}):", wait.unwrap_or(WHEN_IDLE_MINUTES)));
        } else {
            lines.push("would run now:".to_string());
        }
        lines.push(format!("{}  (the file holds {:?})", d.shown(&call), text));
        lines.push(format!("then it would wait up to {} for a compact_boundary line in the transcript; context before {before} tokens.{note}", span(boundary_wait())));
        if let Some(tf) = o.then_file {
            lines.push(format!("then it would send the text of {tf} to the chat as its next message."));
        }
        return Ok(lines.join("\n"));
    }
    if o.when_idle {
        return start_waiter(ctx, o, row, path, wait.unwrap_or(WHEN_IDLE_MINUTES), why);
    }

    let len_before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let wait = match driver_wait(wait) {
        Ok(w) => w,
        Err(m) => die!("{name}: nothing was sent: {m}"),
    };
    let sent = match panel::send(ctx, &row.session_id, &text, wait) {
        Ok(a) => a,
        Err(m) => die!("{name}: {m}"),
    };
    // The compaction was sent: a cancel no longer applies to anything after it (the --then-file send).
    panel::cancel_off();
    let limit = window_cap(boundary_wait());
    let started = Instant::now();
    let (mut boundary, mut after) = appended_since(path, len_before);
    while !boundary && started.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(200));
        (boundary, after) = appended_since(path, len_before);
    }
    if !boundary {
        die!("{name}: /compact was sent to the chat, but its transcript gained no compact_boundary line within {}; look at the chat before trusting it", span(limit));
    }
    let after = match after {
        Some(u) => format!("{} tokens", k(u.tokens)),
        None => "shown at its next turn".to_string(),
    };
    let mut report = format!("{name}: compacted through the panel while it was running. Before {before} tokens; context after: {after}.{note}{}", sent.notes_sentence());
    report.push_str(&send_then(ctx, o, &row.session_id)?);
    Ok(report)
}

/// How long `--cancel` waits for the waiter to stop.
const CANCEL_WAIT_SECS: f64 = 20.0;
/// Test hook: seconds that replace CANCEL_WAIT_SECS.
const CANCEL_WAIT_ENV: &str = "AF_CANCEL_WAIT_SECS";

fn cancel_wait() -> Duration {
    Duration::from_secs_f64(std::env::var(CANCEL_WAIT_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|s| *s > 0.0).unwrap_or(CANCEL_WAIT_SECS))
}

/// `af session compact --cancel <name>`: write the cancel marker next to the chat's job file, wait up to 20
/// seconds for the job file to go, and say exactly what happened. The report and the exit code: 0 when the
/// compaction was cancelled or none was queued, 1 for every outcome where it was not cancelled. No process is
/// ever killed.
fn cancel_report(ctx: &Ctx, name: &str) -> R<(String, i32)> {
    let reg = Registry::load(ctx)?;
    let row = reach::row_or_refuse(&reg, name)?;
    let job = waiter_job_path(ctx, &row.session_id);
    let marker = panel::cancel_marker(&job);
    let sending = panel::sending_marker(&job);
    let log = panel::waiter_log_path();
    let Some((pid, queued)) = pending_waiter(&job) else {
        let left = if job.exists() { " A job file is there, but the waiter it names is gone, so it stops nothing and was left alone." } else { "" };
        return Ok((format!("{name}: no --when-idle compaction is queued for this chat, so there was nothing to cancel.{left}"), 0));
    };
    let note = json!({"requested": crate::core::now_iso(), "by": caller_session()}).to_string();
    if let Err(e) = std::fs::write(&marker, note) {
        die!("{name}: cannot write the cancel marker {}: {e}; nothing was cancelled", marker.display());
    }
    let until = Instant::now() + cancel_wait();
    let gone = || !job.exists() || pending_waiter(&job).is_none();
    while !gone() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    if gone() {
        // A cancelled waiter removes the marker before the job file; a marker that is still there means the
        // waiter ended some other way and never looked at it.
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
            let _ = std::fs::remove_file(&sending);
            die!("{name}: the --when-idle waiter queued at {queued} had already ended before it looked at the cancel, so nothing was cancelled. Its outcome is in {log}");
        }
        return Ok((format!("{name}: cancelled. The --when-idle compaction queued at {queued} stopped while it was only waiting; nothing was changed, nothing was sent, and no --then-file step ran. The waiter logged it in {log}."), 0));
    }
    let waiter = pending_waiter(&job).and_then(|(p, _)| p).or(pid).map(|p| format!("the waiter (pid {p})")).unwrap_or_else(|| "the waiter".to_string());
    let driver = panel::lock_holder(ctx, &row.session_id).map(|p| format!(" and the panel driver run (pid {p})")).unwrap_or_default();
    if sending.exists() {
        let _ = std::fs::remove_file(&marker);
        die!("{name}: the compaction was already being sent when the cancel arrived ({waiter}{driver}), so it was not cancelled and will finish or fail on its own. Its outcome goes to {log}.");
    }
    die!(
        "{name}: {waiter}{driver} did not stop within {} seconds of the cancel marker being written, so it is not known to be cancelled. Nothing was killed. The marker {} is still there, and each stops by itself when it next looks at it (the driver looks every 2 seconds while it waits for the operator); look at {log}, or end those processes yourself if they are stuck.",
        cancel_wait().as_secs_f64(),
        marker.display()
    )
}

/// The job file of the one waiter a chat may have: `system/logs/claude-panel/waiter-<session id>.json`.
fn waiter_job_path(ctx: &Ctx, session_id: &str) -> std::path::PathBuf {
    let safe: String = session_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    std::path::PathBuf::from(&ctx.root).join("system").join("logs").join("claude-panel").join(format!("waiter-{safe}.json"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// How long a job that names no pid yet counts as pending: the verb that wrote it is starting its waiter.
const STARTING_MS: u64 = 120_000;
/// The shortest wait a driver run a waiter starts is given, for the operator to be idle.
pub const STAGE_MIN_WAIT_SECS: u64 = 60;
/// What a waiter may still do after its wait ends: two driver runs (the compaction and the --then-file
/// message, each its shortest wait, the driver's work time and af's margin), the boundary wait, and two
/// minutes for af's own steps between them. The waiter cuts every stage to end by its queue time plus its
/// wait plus this, and a job counts as pending until then (never expired by time while it can still run).
pub const WAITER_RUN_SECS: u64 = 2 * (STAGE_MIN_WAIT_SECS + panel::DRIVER_WORK_SECS + panel::DRIVER_MARGIN_SECS) + BOUNDARY_WAIT_SECS + 120;

/// The latest a waiter queued then, with this wait, may legitimately still run.
pub fn waiter_end_ms(queued_ms: u64, wait_minutes: f64) -> u64 {
    queued_ms + (wait_minutes.max(0.0) * 60_000.0) as u64 + WAITER_RUN_SECS * 1000
}

/// The -WaitMinutes for a driver run a waiter starts at `now_ms`: what is left of its wait (at least
/// STAGE_MIN_WAIT_SECS), cut so the run (that wait, the driver's work time and af's margin) ends by
/// `end_ms`. None when no run could end in time. Rounded down to a hundredth of a minute.
pub fn stage_wait_minutes(now_ms: u64, until_ms: u64, end_ms: u64) -> Option<f64> {
    let left = until_ms.saturating_sub(now_ms).max(STAGE_MIN_WAIT_SECS * 1000);
    let run = (panel::DRIVER_WORK_SECS + panel::DRIVER_MARGIN_SECS) * 1000;
    let room = end_ms.checked_sub(now_ms)?.checked_sub(run)?;
    let wait = left.min(room);
    if wait < STAGE_MIN_WAIT_SECS * 1000 {
        return None;
    }
    Some((wait / 600) as f64 / 100.0)
}

/// In a waiter process: the end of its wait and its own end (ms since the epoch), set once as it fires.
static WINDOW: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();

/// The wait for a driver run: as asked, or in a waiter, what fits its window.
fn driver_wait(asked: Option<f64>) -> Result<Option<f64>, String> {
    match WINDOW.get() {
        None => Ok(asked),
        Some(&(until, end)) => stage_wait_minutes(now_ms(), until, end)
            .map(Some)
            .ok_or_else(|| "the waiter's time is up: no driver run could end before the waiter's own end".to_string()),
    }
}

/// A step's limit, cut in a waiter to what is left before its end.
fn window_cap(limit: Duration) -> Duration {
    match WINDOW.get() {
        None => limit,
        Some(&(_, end)) => limit.min(Duration::from_millis(end.saturating_sub(now_ms()))),
    }
}

/// How long the closed route waits for the panel lock: in a waiter, what is left of its wait (at least
/// five seconds); otherwise LOCK_WAIT_SECS.
fn lock_budget() -> Duration {
    match WINDOW.get() {
        Some(&(until, _)) => Duration::from_millis(until.saturating_sub(now_ms()).max(5_000)),
        None => Duration::from_secs(std::env::var(LOCK_WAIT_ENV).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(LOCK_WAIT_SECS)),
    }
}
/// Test hook: milliseconds between a waiting waiter's looks at the chat (default 3000).
const POLL_ENV: &str = "AF_WAITER_POLL_MS";

/// The waiter already pending for a chat, from its job file's text: its pid (None while the verb that
/// queued it is still starting it) and when it was queued. None when the job is left over: past the latest
/// a waiter can run (`waiter_end_ms`, which the waiter itself keeps to), or its pid is not the waiter that
/// wrote it (dead, or another process started at another time).
fn pending_in(text: &str) -> Option<(Option<u32>, String)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let pid = v.get("pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok());
    let queued_ms = v.get("queued_ms").and_then(Value::as_u64).unwrap_or(0);
    let wait = v.get("wait_minutes").and_then(Value::as_f64).unwrap_or(WHEN_IDLE_MINUTES).max(0.0);
    if now_ms() >= waiter_end_ms(queued_ms, wait) {
        return None;
    }
    let live = match pid {
        Some(p) => panel::process_is(p, panel::start_of(&v, "pid_start")),
        None => now_ms().saturating_sub(queued_ms) < STARTING_MS,
    };
    live.then(|| (pid, v.get("queued").and_then(Value::as_str).unwrap_or("an unknown time").to_string()))
}

fn pending_waiter(job: &std::path::Path) -> Option<(Option<u32>, String)> {
    pending_in(&std::fs::read_to_string(job).ok()?)
}

/// What claiming the chat's one job file came to.
enum Claim {
    Created,
    Pending(Option<u32>, String),
}

/// Create the job file only when absent. A left-over job is replaced only by a compare-and-rename: it
/// is moved aside under a name of this process's own, and removed only when what was moved is what was
/// judged left over; a job written in between is put back. Nothing is ever deleted first, so of two
/// verbs started at the same moment exactly one creates the job.
fn claim_job(job: &std::path::Path, body: &Value) -> Result<Claim, String> {
    use std::io::Write;
    for _ in 0..4 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(job) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(body.to_string().as_bytes()) {
                    drop(f);
                    let _ = std::fs::remove_file(job);
                    return Err(format!("cannot write {}: {e}", job.display()));
                }
                return Ok(Claim::Created);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("cannot write {}: {e}", job.display())),
        }
        let Ok(seen) = std::fs::read_to_string(job) else { continue };
        if let Some((pid, queued)) = pending_in(&seen) {
            return Ok(Claim::Pending(pid, queued));
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let aside = job.with_extension(format!("stale-{}-{nanos}", std::process::id()));
        if std::fs::rename(job, &aside).is_err() {
            continue;
        }
        let moved = std::fs::read_to_string(&aside).unwrap_or_default();
        if moved != seen {
            // Not the job judged left over: put it back unless a newer one is already there.
            let _ = std::fs::hard_link(&aside, job);
        }
        let _ = std::fs::remove_file(&aside);
    }
    match pending_waiter(job) {
        Some((pid, queued)) => Ok(Claim::Pending(pid, queued)),
        None => Err(format!("could not claim {}: it kept changing", job.display())),
    }
}

fn already_pending(name: &str, pid: Option<u32>, queued: &str) -> String {
    format!(
        "{name}: a --when-idle waiter{} queued at {queued} is already pending for this chat, so nothing new was started. When it fires it checks again whether the chat still needs compacting.",
        pid.map(|p| format!(" (pid {p})")).unwrap_or_default()
    )
}

/// Write a job file whole: to a temp name, then renamed over it.
fn write_job(job: &std::path::Path, body: &Value) {
    let tmp = job.with_extension("json.tmp");
    if std::fs::write(&tmp, body.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, job);
    }
}

/// Start the detached waiter: the same verb again, with the job in the chat's one job file, outliving
/// this process. A chat has at most one pending waiter; a second `--when-idle` says so and starts none.
fn start_waiter(ctx: &Ctx, o: &Opts, row: &sessions::Row, path: &str, wait: f64, why: &str) -> R<String> {
    let name = o.name;
    let job = waiter_job_path(ctx, &row.session_id);
    if let Some(dir) = job.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            die!("cannot create {}: {e}", dir.display());
        }
    }
    let then_file = o.then_file.map(|t| std::fs::canonicalize(t).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| t.to_string()));
    let body = json!({
        "name": name,
        "session_id": row.session_id,
        "instructions": o.instructions,
        "force": o.force,
        "then_file": then_file,
        "wait_minutes": wait,
        "queued_ms": now_ms(),
        "queued": crate::core::now_iso(),
        "transcript_len": std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        "queued_by": caller_session(),
        "pid": Value::Null,
    });
    match claim_job(&job, &body) {
        Ok(Claim::Created) => {
            // A marker left by an earlier job (a cancel that came too late) must not stop this one.
            let _ = std::fs::remove_file(panel::cancel_marker(&job));
            let _ = std::fs::remove_file(panel::sending_marker(&job));
        }
        Ok(Claim::Pending(pid, queued)) => return Ok(already_pending(name, pid, &queued)),
        Err(m) => die!("{m}"),
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            let _ = std::fs::remove_file(&job);
            die!("cannot find the af program to start the waiter: {e}")
        }
    };
    let args: Vec<String> = ["session", "compact", name, "--waiter-job"].iter().map(|s| s.to_string()).chain([job.to_string_lossy().into_owned()]).collect();
    match panel::spawn_detached(&exe, &args, &ctx.root) {
        // The waiter writes its own pid into the job as it starts; until then the job counts as pending
        // for STARTING_MS. (Writing it from here could recreate a job the waiter already finished.)
        Ok(pid) => {
            Ok(format!(
                "{name}: running ({why}). A detached waiter (pid {pid}) compacts it once the chat and the operator are idle, within {wait} minutes{}. It decides when it fires: if the chat was compacted meanwhile it does nothing. Its outcome is appended to {}; a failure is also shown once, at the next tool call or stop, to the chat that queued it (to this chat when af cannot tell which chat that is). To cancel it: af session compact --cancel {name}",
                if o.then_file.is_some() { ", then sends the --then-file text as the next message" } else { "" },
                panel::waiter_log_path()
            ))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&job);
            die!("{name}: could not start the waiter: {e}")
        }
    }
}

/// Why the chat is busy now, or None: a live driver run holds the panel lock for it, or a session record
/// for it whose process is alive says anything but `status: idle` (no status is not idle).
fn busy_now(ctx: &Ctx, session_id: &str) -> Option<String> {
    if let Some(pid) = panel::lock_holder(ctx, session_id) {
        return Some(format!("the panel driver (pid {pid}) is working on it"));
    }
    let dir = reach::config_dir().map(|c| pyos::join(&c, "sessions"))?;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Some(v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        if v.get("sessionId").and_then(Value::as_str) != Some(session_id) {
            continue;
        }
        if let Some(pid) = v.get("pid").and_then(Value::as_u64) {
            if !panel::pid_alive(u32::try_from(pid).unwrap_or(0)) {
                continue;
            }
        }
        let status = v.get("status").and_then(Value::as_str).unwrap_or("");
        if status != "idle" {
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            return Some(format!("its session record sessions/{file} says {}", if status.is_empty() { "no status" } else { status }));
        }
    }
    None
}

/// The detached process. It waits for the chat itself and decides what to do when it fires, not when
/// it was queued: nothing when the transcript gained a `compact_boundary` after its queue time (the
/// chat was compacted meanwhile), else the route the chat has then. Its outcome goes to the waiter log;
/// a failure also leaves a one-line result that the context hook shows the chat once.
fn run_waiter(ctx: &Ctx, job_path: &str) -> R<i32> {
    let text = match std::fs::read_to_string(job_path) {
        Ok(t) => t,
        Err(e) => die!("cannot read the waiter job {job_path}: {e}"),
    };
    let mut v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_file(job_path);
            die!("the waiter job {job_path} is not JSON: {e}")
        }
    };
    // The job file is this waiter's claim on the chat: it names this process, and when it started, while
    // it waits.
    let me = std::process::id();
    v["pid"] = json!(me);
    v["pid_start"] = json!(panel::pid_start(me));
    write_job(std::path::Path::new(job_path), &v);
    // Its waits, and the driver runs it starts, stop for this marker while nothing has been changed.
    let marker = panel::cancel_marker(std::path::Path::new(job_path));
    let sending = panel::sending_marker(std::path::Path::new(job_path));
    panel::set_cancel_file(marker.clone());
    let name = v.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let outcome = fire_waiter(ctx, &v, &name);
    let cancelled = outcome.is_err() && panel::was_cancelled();
    // The marker that says the run began changing things is this waiter's own, whatever the outcome. A
    // cancel marker is removed only for a cancelled job (it is the cancel's proof); after any other outcome
    // it stays until the cancel verb or the next queued job removes it. The job file goes last: its
    // disappearance is what `--cancel` waits for.
    let _ = std::fs::remove_file(&sending);
    if cancelled {
        let _ = std::fs::remove_file(&marker);
    }
    // Removed only while it is still this waiter's job.
    let still_mine = std::fs::read_to_string(job_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|j| j.get("pid").and_then(Value::as_u64) == Some(me as u64));
    if still_mine {
        let _ = std::fs::remove_file(job_path);
    }
    let session_id = v.get("session_id").and_then(Value::as_str).map(str::to_string).or_else(|| Registry::load(ctx).ok().and_then(|r| r.find_name(&name)).map(|r| r.session_id));
    match outcome {
        Ok(report) => {
            panel::waiter_log(ctx, &format!("{name} compact ok: {report}"));
            Ok(0)
        }
        // Cancelled by the operator's own command: logged, never shown to a chat as a failure, and the
        // --then-file step was not run.
        Err(e) if cancelled => {
            panel::waiter_log(ctx, &format!("{name} compact cancelled: {e}"));
            Ok(0)
        }
        Err(e) => {
            panel::waiter_log(ctx, &format!("{name} compact failed: {e}"));
            let queued_by = v.get("queued_by").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
            if let Some((sid, line)) = failure_line(&name, session_id.as_deref(), queued_by.as_deref(), &e.to_string()) {
                crate::context::write_waiter_result(ctx, &sid, &line);
            }
            Err(e)
        }
    }
}

/// The session that ran this af, when Claude Code says so (`CLAUDE_CODE_SESSION_ID` in a chat's tools).
pub fn caller_session() -> Option<String> {
    std::env::var("CLAUDE_CODE_SESSION_ID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Where a failed waiter's line goes and what it says: to the chat that queued it, worded for who queued
/// what; to the compacted chat when the caller is not known.
fn failure_line(name: &str, compacted: Option<&str>, queued_by: Option<&str>, why: &str) -> Option<(String, String)> {
    match (queued_by, compacted) {
        (Some(q), Some(c)) if q == c => Some((c.to_string(), format!("The --when-idle compaction this chat queued for itself failed: {why}"))),
        (Some(q), _) => Some((q.to_string(), format!("The --when-idle compaction you queued for {name} failed: {why}"))),
        (None, Some(c)) => Some((c.to_string(), format!("A --when-idle compaction of this chat failed; it was queued by a caller af could not identify (maybe a terminal or a script): {why}"))),
        (None, None) => None,
    }
}

fn fire_waiter(ctx: &Ctx, v: &Value, name: &str) -> R<String> {
    let wait = v.get("wait_minutes").and_then(Value::as_f64).unwrap_or(WHEN_IDLE_MINUTES);
    let queued_ms = v.get("queued_ms").and_then(Value::as_u64).unwrap_or_else(now_ms);
    let deadline_ms = queued_ms + (wait.max(0.0) * 60_000.0) as u64;
    // Every stage from here is cut to end by the waiter's own end, the one the job's expiry uses.
    let _ = WINDOW.set((deadline_ms, waiter_end_ms(queued_ms, wait)));
    let session_id = match v.get("session_id").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => reach::row_or_refuse(&Registry::load(ctx)?, name)?.session_id,
    };
    let len = v.get("transcript_len").and_then(Value::as_u64).unwrap_or(0);
    let poll = std::env::var(POLL_ENV).ok().and_then(|p| p.trim().parse::<u64>().ok()).unwrap_or(3000);
    loop {
        if panel::cancel_requested() {
            panel::note_cancelled();
            return Err(AfError::Refusal(format!("{name}: cancelled while it was waiting for the chat and the operator; nothing was changed and nothing was sent")));
        }
        if let Some(path) = reach::transcript_path(&session_id) {
            if appended_since(&path, len).0 {
                return Ok(format!("{name}: nothing to do; its transcript gained a compact_boundary line after this waiter was queued, so it was compacted meanwhile."));
            }
        }
        match busy_now(ctx, &session_id) {
            None => break,
            Some(why) if now_ms() >= deadline_ms => die!("{name}: the chat stayed busy ({why}) for the waiter's whole wait of {wait} minutes; nothing was sent"),
            Some(_) => std::thread::sleep(Duration::from_millis(poll)),
        }
    }
    // What is left of the wait goes to the driver, for the operator to be idle; at least a minute.
    let left = (deadline_ms.saturating_sub(now_ms()) as f64 / 60_000.0).max(1.0);
    let left = (left * 100.0).round() / 100.0;
    let instructions = v.get("instructions").and_then(Value::as_str).map(str::to_string);
    let then_file = v.get("then_file").and_then(Value::as_str).map(str::to_string);
    let o = Opts {
        name,
        model: None,
        instructions: instructions.as_deref(),
        force: v.get("force").and_then(Value::as_bool).unwrap_or(false),
        dry_run: false,
        when_idle: false,
        wait_minutes: Some(left),
        then_file: then_file.as_deref(),
        waiter_job: None,
        cancel: false,
    };
    compact_report(ctx, &o)
}

type Judged = Result<(String, Option<reach::Usage>), String>;

/// The verdict on a finished run: the cost and the context after, or what failed.
fn judge(out: &Out, path: &str, len_before: u64, limit: Duration) -> Judged {
    let (boundary, after) = appended_since(path, len_before);
    if out.timed_out() {
        let changed = std::fs::metadata(path).map(|m| m.len() != len_before).unwrap_or(false);
        return Err(format!(
            "the command ran past {} and was stopped (with the processes it started, on Windows); the transcript {} and {}.",
            span(limit),
            if changed { "changed" } else { "did not change" },
            if boundary { "has a compact_boundary line, so check it before trusting it" } else { "has no compact_boundary line" }
        ));
    }
    let fail = |why: &str| -> Judged { Err(format!("{why}. Output: {}", head(if out.stdout.trim().is_empty() { &out.stderr } else { &out.stdout }))) };
    if out.code.is_none() {
        return fail(&out.why());
    }
    let result = parse_result(&out.stdout);
    match result.as_ref().and_then(|r| r.get("is_error")).and_then(Value::as_bool) {
        Some(false) => {}
        Some(true) => return fail("the command reported is_error: true"),
        None => return fail("the command printed no JSON result with is_error"),
    }
    if !out.ok() {
        return fail(&format!("the command exited {}", out.code.unwrap_or(1)));
    }
    if !boundary {
        return fail("the transcript gained no compact_boundary line");
    }
    let cost = result
        .as_ref()
        .and_then(|r| r.get("total_cost_usd"))
        .and_then(Value::as_f64)
        .map(|c| format!("${c:.2}"))
        .unwrap_or_else(|| "unknown".to_string());
    Ok((cost, after))
}

/// The roster and session records read again once the command is done. A chat that is live now
/// was opened during the compaction, which is an error whatever the compaction did. Otherwise
/// the sentence to append to a success report, empty when all is well.
fn after_note(ctx: &Ctx, result: &Judged, session_id: &str, name: &str) -> R<String> {
    match live_check(ctx, session_id) {
        Live::Closed => Ok(String::new()),
        Live::Unknown if result.is_ok() => Ok(" Could not check afterwards that nothing opened the chat during the run.".to_string()),
        Live::Unknown => Ok(String::new()),
        Live::Running(why) => {
            let did = match result {
                Ok(_) => "The compaction itself succeeded.".to_string(),
                Err(why) => format!("The compaction itself failed: {why}"),
            };
            die!("WARNING {name} is live now ({why}): it was opened during the compaction, so two processes may have written its transcript. Close that chat and reopen it. {did}")
        }
    }
}
