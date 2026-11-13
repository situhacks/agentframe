//! `af session compact <name>`: compact a chat that has no live process, from outside it, with
//! `claude -p "/compact ..." --resume <id>`. The project's `pre-compact` hook adds the standing
//! summary instructions and `post-compact` re-arms the nudges. A chat with a live process is
//! never compacted this way: two processes would write one transcript, so the guard fails
//! closed and is checked again after the run.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::outln;
use crate::pyos;
use crate::proc::{claude_bin, run_command, Out};
use crate::reach;
use crate::sessions::{self, Registry};
use serde_json::Value;
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

pub fn cmd_compact(ctx: &Ctx, name: &str, model: Option<&str>, instructions: Option<&str>, force: bool, dry_run: bool) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let row = reach::row_or_refuse(&reg, name)?;

    match live_check(&row.session_id) {
        Live::Closed => {}
        Live::Unknown => die!("{name}: cannot tell whether it is running, the claude agents roster is unavailable; nothing was compacted"),
        Live::Running(why) => die!("{name} is running ({why}), so compacting it from outside would put two processes on one transcript. Close its tab and rerun, or type /compact in that chat"),
    }

    let Some(path) = reach::transcript_path(&row.session_id) else {
        die!("{name}: no transcript found for session {}; nothing to compact", row.session_id);
    };
    let usage = reach::context_usage(&path);
    let tokens = usage.as_ref().map(|u| u.tokens);
    match tokens {
        Some(t) if t < MIN_TOKENS && !force => {
            outln!("{name}: context is {}, under {}; nothing worth compacting (--force overrides).", k(t), k(MIN_TOKENS));
            return Ok(0);
        }
        None if !force => die!("{name}: its transcript has no assistant usage line, so its size is unknown; --force compacts anyway"),
        _ => {}
    }

    let chosen = match model.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => Some((m.to_string(), "--model".to_string())),
        None => usage.as_ref().and_then(|u| default_model(u, &reach::tail(&path).unwrap_or_default())),
    };
    let Some((model, source)) = chosen else {
        die!("{name}: its transcript records no model; pass --model <id>");
    };
    if model.to_lowercase().contains("haiku") && !force {
        die!("{name}: {model} would write the summary, and a summary is the chat's whole memory; pass a larger --model, or --force");
    }

    let args = command_args(instructions, &row.session_id, &model);
    let before = tokens.map(k).unwrap_or_else(|| "unknown".to_string());
    let bin = claude_bin();
    if dry_run {
        let line: Vec<String> = args.iter().map(|a| shown(a)).collect();
        outln!("{name}: would run from {}: {} {}", ctx.root, shown(&bin), line.join(" "));
        outln!("model {model} (from {source}), context before {before} tokens.");
        return Ok(0);
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
            outln!("{name}: compacted. Before {before} tokens, model {model} (from {source}), cost {cost}; context after: {after}.{note}");
            Ok(0)
        }
        Err(why) => die!("{name}: compaction failed, {why}"),
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
