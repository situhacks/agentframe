//! `af session compact <name>`: compact a chat that has no live process, from outside it, with
//! `claude -p "/compact ..." --resume <id>`. The project's `pre-compact` hook adds the standing
//! summary instructions and `post-compact` re-arms the nudges. A chat with a live process is
//! never compacted this way: two processes would write one transcript, so the guard fails
//! closed and is checked again after the run. A running chat is compacted through the VS Code
//! panel instead: the panel driver types ` /compact ...` into it once the operator and the chat are
//! idle, and the transcript must gain a `compact_boundary` line. `--when-idle` starts a detached
//! waiter that does this once the chat finishes its turn; `--then-file` sends a file's text to the
//! chat as its next message after the compaction.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
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
const BOUNDARY_WAIT: Duration = Duration::from_secs(10 * 60);
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

/// Fail closed: the chat is closed only when its session id is absent from the roster
/// (`claude agents --json --all`; any entry counts, whatever its status) and no record under
/// the config dir's `sessions/` names it. A record that does not parse is ignored.
fn live_check(session_id: &str) -> Live {
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
}

pub fn cmd_compact(ctx: &Ctx, o: &Opts) -> R<i32> {
    if let Some(job) = o.waiter_job {
        return run_waiter(ctx, job);
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

    let running = match live_check(&row.session_id) {
        Live::Closed => None,
        Live::Unknown => die!("{name}: cannot tell whether it is running, the claude agents roster is unavailable; nothing was compacted"),
        Live::Running(why) => Some(why),
    };

    let Some(path) = reach::transcript_path(&row.session_id) else {
        die!("{name}: no transcript found for session {}; nothing to compact", row.session_id);
    };
    let usage = reach::context_usage(&path);
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
        None => usage.as_ref().and_then(|u| default_model(u, &reach::tail(&path).unwrap_or_default())),
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

    let len_before = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let limit = deadline();
    let mut cmd = Command::new(&bin);
    cmd.args(&args).current_dir(&ctx.root);
    let out = run_command(cmd, "claude", Some(Instant::now() + limit), true);
    let result = judge(&out, &path, len_before, limit);
    let note = after_note(&result, &row.session_id, name)?;
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
    match panel::send(ctx, session_id, &text, o.wait_minutes) {
        Ok(()) => Ok(format!(" Sent the text of {tf} as its next message.")),
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

/// Compact a chat that is running: the panel driver types ` /compact ...` into it, and the
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
            lines.push(format!("would start a detached waiter, which runs this once the chat is idle, within {} minutes:", wait.unwrap_or(WHEN_IDLE_MINUTES)));
        } else {
            lines.push("would run now:".to_string());
        }
        lines.push(format!("{}  (the file holds {:?})", d.shown(&call), panel::typed(&text)));
        lines.push(format!("then it would wait up to {} for a compact_boundary line in the transcript; context before {before} tokens.{note}", span(boundary_wait())));
        if let Some(tf) = o.then_file {
            lines.push(format!("then it would send the text of {tf} to the chat as its next message."));
        }
        return Ok(lines.join("\n"));
    }
    if o.when_idle {
        return start_waiter(ctx, o, wait.unwrap_or(WHEN_IDLE_MINUTES), why);
    }

    let len_before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if let Err(m) = panel::send(ctx, &row.session_id, &text, wait) {
        die!("{name}: {m}");
    }
    let limit = boundary_wait();
    let started = Instant::now();
    let (mut boundary, mut after) = appended_since(path, len_before);
    while !boundary && started.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(200));
        (boundary, after) = appended_since(path, len_before);
    }
    if !boundary {
        die!("{name}: /compact was typed into the chat, but its transcript gained no compact_boundary line within {}; look at the chat before trusting it", span(limit));
    }
    let after = match after {
        Some(u) => format!("{} tokens", k(u.tokens)),
        None => "shown at its next turn".to_string(),
    };
    let mut report = format!("{name}: compacted through the panel while it was running. Before {before} tokens; context after: {after}.{note}");
    report.push_str(&send_then(ctx, o, &row.session_id)?);
    Ok(report)
}

/// Start the detached waiter: the same verb again, with the job in a file, outliving this process.
fn start_waiter(ctx: &Ctx, o: &Opts, wait: f64, why: &str) -> R<String> {
    let name = o.name;
    let dir = std::path::PathBuf::from(&ctx.root).join("system").join("logs").join("claude-panel");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        die!("cannot create {}: {e}", dir.display());
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let job = dir.join(format!("waiter-{}-{nanos}.json", std::process::id()));
    let then_file = o.then_file.map(|t| std::fs::canonicalize(t).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| t.to_string()));
    let body = json!({"name": name, "instructions": o.instructions, "force": o.force, "then_file": then_file, "wait_minutes": wait});
    if let Err(e) = std::fs::write(&job, body.to_string()) {
        die!("cannot write {}: {e}", job.display());
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => die!("cannot find the af program to start the waiter: {e}"),
    };
    let args: Vec<String> = ["session", "compact", name, "--waiter-job"].iter().map(|s| s.to_string()).chain([job.to_string_lossy().into_owned()]).collect();
    match panel::spawn_detached(&exe, &args, &ctx.root) {
        Ok(pid) => Ok(format!(
            "{name}: running ({why}). A detached waiter (pid {pid}) compacts it once the chat and the operator are idle, within {wait} minutes{}. Its outcome is appended to {}.",
            if o.then_file.is_some() { ", then sends the --then-file text as the next message" } else { "" },
            panel::waiter_log_path()
        )),
        Err(e) => {
            let _ = std::fs::remove_file(&job);
            die!("{name}: could not start the waiter: {e}")
        }
    }
}

/// The detached process: runs the compaction the job names (the chat may have closed meanwhile,
/// so the route is chosen again) and appends how it ended to the waiter log.
fn run_waiter(ctx: &Ctx, job_path: &str) -> R<i32> {
    let text = match std::fs::read_to_string(job_path) {
        Ok(t) => t,
        Err(e) => die!("cannot read the waiter job {job_path}: {e}"),
    };
    let _ = std::fs::remove_file(job_path);
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => die!("the waiter job {job_path} is not JSON: {e}"),
    };
    let name = v.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let instructions = v.get("instructions").and_then(Value::as_str).map(str::to_string);
    let then_file = v.get("then_file").and_then(Value::as_str).map(str::to_string);
    let o = Opts {
        name: &name,
        model: None,
        instructions: instructions.as_deref(),
        force: v.get("force").and_then(Value::as_bool).unwrap_or(false),
        dry_run: false,
        when_idle: false,
        wait_minutes: v.get("wait_minutes").and_then(Value::as_f64),
        then_file: then_file.as_deref(),
        waiter_job: None,
    };
    match compact_report(ctx, &o) {
        Ok(report) => {
            panel::waiter_log(ctx, &format!("{name} compact ok: {report}"));
            Ok(0)
        }
        Err(e) => {
            panel::waiter_log(ctx, &format!("{name} compact failed: {e}"));
            Err(e)
        }
    }
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
fn after_note(result: &Judged, session_id: &str, name: &str) -> R<String> {
    match live_check(session_id) {
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
