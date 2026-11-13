//! `af session compact <name>`: compact a chat that has no live process, from outside it, with
//! `claude -p "/compact ..." --resume <id>`. The project's `pre-compact` hook adds the standing
//! summary instructions and `post-compact` re-arms the nudges. A chat with a live process is
//! never compacted this way: two processes would write one transcript.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::outln;
use crate::proc::{claude_bin, run_command};
use crate::reach::{self, Facts};
use crate::sessions::{self, Registry};
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;
use std::time::{Duration, Instant};

/// A chat under this many context tokens has nothing worth summarising.
pub const MIN_TOKENS: u64 = 50_000;
/// The longest a compaction may run.
const DEADLINE: Duration = Duration::from_secs(15 * 60);
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

pub fn cmd_compact(ctx: &Ctx, name: &str, model: Option<&str>, instructions: Option<&str>, force: bool, dry_run: bool) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let row = reach::row_or_refuse(&reg, name)?;

    // A chat with a process of its own is never compacted from here. Same roster, same verdict
    // as `af session reach`: only "closed" proves no process.
    let roster = sessions::claude_roster();
    let status = roster.as_ref().map(|r| sessions::join_roster(std::slice::from_ref(&row), r).remove(0));
    let verdict = reach::verdict(&Facts { name, session_id: &row.session_id, roster: status.as_deref(), ..Default::default() });
    match verdict.state {
        "closed" => {}
        "unknown" => die!("{name}: cannot tell whether it is running, the claude agents roster is unavailable; nothing was compacted"),
        _ => die!("{name} is running, so compacting it from outside would put two processes on one transcript. Close its tab and rerun, or type /compact in that chat"),
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

    let model = match model.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => m.to_string(),
        None => match usage.as_ref().and_then(|u| u.model.clone()) {
            Some(m) => m,
            None => die!("{name}: its transcript records no model; pass --model <id>"),
        },
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
        outln!("model {model}, context before {before} tokens.");
        return Ok(0);
    }

    let len_before = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut cmd = Command::new(&bin);
    cmd.args(&args).current_dir(&ctx.root);
    let out = run_command(cmd, "claude", Some(Instant::now() + DEADLINE));
    let combined = if out.stdout.trim().is_empty() { out.stderr.clone() } else { out.stdout.clone() };
    let fail = |why: &str| -> R<i32> { die!("{name}: compaction failed, {why}. Output: {}", head(&combined)) };

    if out.timed_out() {
        return fail("the command ran past 15 minutes and was stopped");
    }
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
    let (boundary, after) = appended_since(&path, len_before);
    if !boundary {
        return fail("the transcript gained no compact_boundary line");
    }
    let cost = result
        .as_ref()
        .and_then(|r| r.get("total_cost_usd"))
        .and_then(Value::as_f64)
        .map(|c| format!("${c:.2}"))
        .unwrap_or_else(|| "unknown".to_string());
    let after = match after {
        Some(u) => format!("{} tokens", k(u.tokens)),
        None => "shown at its next turn".to_string(),
    };
    outln!("{name}: compacted. Before {before} tokens, model {model}, cost {cost}; context after: {after}.");
    Ok(0)
}
