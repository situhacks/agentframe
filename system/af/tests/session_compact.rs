//! `af session compact`: compacting a closed chat from outside it. The program it runs is a stub
//! compiled for the test and named through `AF_CLAUDE_BIN`; the real `claude` is never called.
//! The stub answers `agents` with the roster in `STUB_ROSTER`, and otherwise records its working
//! directory and arguments as a JSON line in `STUB_CALLS`, appends a `compact_boundary` line to
//! `STUB_TRANSCRIPT`, and prints `STUB_OUTPUT` (a success result by default). Other `STUB_*`
//! variables make it write a session record, sleep, or exit non-zero.

mod common;
use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const STUB_SRC: &str = r##"
use std::io::Write;
fn append(path: &str, text: &str) {
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
}
fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("agents") {
        if std::env::var("STUB_ROSTER_FAIL").is_ok() {
            std::process::exit(1);
        }
        print!("{}", std::env::var("STUB_ROSTER").unwrap_or_else(|_| "[]".into()));
        return;
    }
    if let Ok(calls) = std::env::var("STUB_CALLS") {
        let cwd = std::env::current_dir().unwrap();
        let quoted: Vec<String> = args.iter().map(|a| format!("\"{}\"", esc(a))).collect();
        append(&calls, &format!("{{\"cwd\":\"{}\",\"args\":[{}]}}\n", esc(&cwd.display().to_string()), quoted.join(",")));
    }
    if let Ok(t) = std::env::var("STUB_TRANSCRIPT") {
        if std::env::var("STUB_NO_BOUNDARY").is_err() {
            append(&t, "{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"content\":\"Conversation compacted\"}\n");
        }
        if let Ok(line) = std::env::var("STUB_AFTER_LINE") {
            append(&t, &format!("{line}\n"));
        }
    }
    if let Ok(record) = std::env::var("STUB_WRITE_SESSION") {
        std::fs::create_dir_all(std::path::Path::new(&record).parent().unwrap()).unwrap();
        std::fs::write(&record, "{\"pid\":4242,\"sessionId\":\"sid-1\",\"status\":\"busy\"}").unwrap();
    }
    if let Ok(secs) = std::env::var("STUB_SLEEP_SECS") {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs.parse().unwrap()));
    }
    println!("{}", std::env::var("STUB_OUTPUT").unwrap_or_else(|_| {
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":0,\"session_id\":\"sid-1\",\"total_cost_usd\":0.4321}".into()
    }));
    if let Ok(code) = std::env::var("STUB_EXIT") {
        std::process::exit(code.parse().unwrap());
    }
}
"##;

/// The stub program, compiled once for the whole test run.
fn stub() -> &'static Path {
    static STUB: OnceLock<PathBuf> = OnceLock::new();
    STUB.get_or_init(|| {
        let target = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
        let dir = target.join("af-test-stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("claude_stub.rs");
        let exe = dir.join(if cfg!(windows) { "claude_stub.exe" } else { "claude_stub" });
        std::fs::write(&src, STUB_SRC).unwrap();
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
        let status = std::process::Command::new(rustc).args(["--edition", "2021", "-o"]).arg(&exe).arg(&src).status().unwrap();
        assert!(status.success(), "the claude stub did not compile");
        exe
    })
}

/// An assistant line as Claude Code writes it: `message.model` is the plain id and the optional
/// top-level `requestedModel` is what the chat asked for, with `[1m]` on the large window.
fn assistant_requested(model: Option<&str>, requested: Option<&str>, total: u64) -> String {
    let mut message = json!({"role": "assistant", "usage": {"input_tokens": 1, "cache_read_input_tokens": total - 1, "output_tokens": 5}});
    if let Some(m) = model {
        message["model"] = json!(m);
    }
    let mut line = json!({"type": "assistant", "message": message});
    if let Some(r) = requested {
        line["requestedModel"] = json!(r);
    }
    line.to_string()
}

fn assistant(model: Option<&str>, total: u64) -> String {
    assistant_requested(model, model, total)
}

const TRANSCRIPT: &str = "cfg/projects/p1/sid-1.jsonl";

fn registered() -> Fx {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx
}

fn with_lines(lines: &[String]) -> Fx {
    let fx = registered();
    fx.write(TRANSCRIPT, &format!("{}\n", lines.join("\n")));
    fx
}

/// A registered chat `system/x` (session `sid-1`) whose transcript holds one assistant line.
fn chat(model: Option<&str>, tokens: u64) -> Fx {
    with_lines(&[assistant(model, tokens)])
}

/// A chat with a 312k-token context on the 1M window, as a real transcript records it.
fn big() -> Fx {
    with_lines(&[assistant_requested(Some("claude-opus-5-5"), Some("claude-opus-5-5[1m]"), 312_000)])
}

const IDLE: &str = r#"[{"sessionId":"sid-1","status":"idle"}]"#;

/// Run `af session compact` with the stub; (exit code, stdout, stderr).
fn compact(fx: &Fx, env: &[(&str, &str)], args: &[&str]) -> (i32, String, String) {
    let mut c = fx.af();
    c.env("CLAUDE_CONFIG_DIR", fx.path("cfg"))
        .env("AF_CLAUDE_BIN", stub())
        .env("STUB_TRANSCRIPT", fx.path(TRANSCRIPT))
        .env("STUB_CALLS", fx.path("calls.txt"));
    for (k, v) in env {
        c.env(k, v);
    }
    let mut full = vec!["session", "compact", "system/x"];
    full.extend_from_slice(args);
    let o = c.args(full).output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Where each stub call ran and its arguments.
fn calls(fx: &Fx) -> Vec<(String, Vec<String>)> {
    let Ok(text) = std::fs::read_to_string(fx.path("calls.txt")) else { return Vec::new() };
    text.lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            let args = v["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
            (v["cwd"].as_str().unwrap().to_string(), args)
        })
        .collect()
}

fn model_arg(fx: &Fx, call: usize) -> String {
    let args = &calls(fx)[call].1;
    let at = args.iter().position(|a| a == "--model").unwrap();
    args[at + 1].clone()
}

fn boundaries(fx: &Fx) -> usize {
    fx.read(TRANSCRIPT).lines().filter(|l| l.contains("compact_boundary")).count()
}

fn run_log_has(fx: &Fx, needle: &str) -> bool {
    fx.run_log_rows().iter().any(|r| r.get("refusal").and_then(Value::as_str).is_some_and(|m| m.contains(needle)))
}

// ------------------------------------------------------------------ the happy path

#[test]
fn a_closed_chat_is_compacted_with_the_resume_command_from_the_vault_root() {
    let fx = big();
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(boundaries(&fx), 1);
    for part in ["system/x: compacted", "312k", "claude-opus-5-5[1m] (from requestedModel of its last reply)", "$0.43", "next turn"] {
        assert!(out.contains(part), "{part}: {out}");
    }
    let made = calls(&fx);
    assert_eq!(made.len(), 1, "{made:?}");
    let (cwd, args) = &made[0];
    assert_eq!(std::fs::canonicalize(cwd).unwrap(), std::fs::canonicalize(fx.dir.path()).unwrap());
    assert_eq!(args, &["-p", "/compact", "--resume", "sid-1", "--model", "claude-opus-5-5[1m]", "--output-format", "json"]);
}

#[test]
fn instructions_follow_compact_and_a_new_usage_line_gives_the_context_after() {
    let fx = big();
    let after = assistant(Some("claude-opus-5-5"), 41_000);
    let (code, out, err) = compact(&fx, &[("STUB_AFTER_LINE", &after)], &["--instructions", "keep the plan path"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(calls(&fx)[0].1[1], "/compact keep the plan path");
    assert!(out.contains("context after: 41k tokens"), "{out}");
}

#[test]
fn instructions_with_quotes_and_a_newline_arrive_as_one_argument() {
    let fx = big();
    let text = "say \"hi\" to \"him\"\nthen stop";
    let (code, out, err) = compact(&fx, &[], &["--instructions", text]);
    assert_eq!(code, 0, "{out}{err}");
    let args = &calls(&fx)[0].1;
    assert_eq!(args.len(), 8, "{args:?}");
    assert_eq!(args[1], format!("/compact {text}"));
    assert_eq!(args[2], "--resume");
}

#[test]
fn an_ended_row_is_compacted_like_any_other() {
    let fx = big();
    fx.ok(&["session", "end", "system/x", "--reason", "exit"]);
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(boundaries(&fx), 1);
}

#[test]
fn a_second_compact_on_a_just_compacted_chat_says_already_compact_and_runs_nothing() {
    let fx = big();
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!((boundaries(&fx), calls(&fx).len()), (1, 1));
    for extra in [&[][..], &["--dry-run"][..], &["--model", "claude-opus-5-5"][..]] {
        let (code, out, err) = compact(&fx, &[], extra);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("system/x: already compact") && out.contains("--force runs it anyway"), "{out}");
        assert!(!err.contains("no compact_boundary"), "{err}");
    }
    assert_eq!((boundaries(&fx), calls(&fx).len()), (1, 1), "no further run, no further boundary");
}

#[test]
fn force_runs_the_compaction_on_a_chat_that_reads_as_already_compact() {
    // After a compaction he pastes a file and the request fails "prompt too long": that error reply
    // has all-zero usage, so the transcript still reads as compacted, but there is something to compact.
    let fx = big();
    compact(&fx, &[], &[]);
    let (code, out, err) = compact(&fx, &[], &["--force", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("would run") && out.contains("model claude-opus-5-5[1m] (from requestedModel of its last reply)"), "the model still comes from the reply before the boundary: {out}");
    assert_eq!(calls(&fx).len(), 1, "a dry run runs nothing");
    let (code, out, err) = compact(&fx, &[], &["--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: compacted") && out.contains("Before unknown tokens"), "{out}");
    assert_eq!((boundaries(&fx), calls(&fx).len()), (2, 2));
    assert_eq!(model_arg(&fx, 1), "claude-opus-5-5[1m]");
}

#[test]
fn a_reply_after_the_boundary_makes_the_chat_compactable_again() {
    let fx = big();
    compact(&fx, &[], &[]);
    let mut text = fx.read(TRANSCRIPT);
    text.push_str(&format!("{}\n", assistant_requested(Some("claude-opus-5-5"), Some("claude-opus-5-5[1m]"), 120_000)));
    fx.write(TRANSCRIPT, &text);
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: compacted"), "{out}");
    assert_eq!(boundaries(&fx), 2);
}

#[test]
fn a_roster_entry_with_no_status_or_status_shell_is_reported_as_running_by_list_and_reach() {
    let fx = big();
    for roster in [r#"[{"sessionId":"sid-1"}]"#, r#"[{"sessionId":"sid-1","status":"shell"}]"#] {
        let run = |args: &[&str]| {
            let o = fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).env("AF_CLAUDE_BIN", stub()).env("STUB_ROSTER", roster).args(args).output().unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let list: Value = serde_json::from_str(&run(&["session", "list", "--json"])).unwrap();
        let state = list[0]["live_state"].as_str().unwrap().to_string();
        assert!(state == "unknown" || state == "shell", "{roster}: {state}");
        let reach: Value = serde_json::from_str(&run(&["session", "reach", "system/x", "--json"])).unwrap();
        let line = reach["line"].as_str().unwrap();
        assert_ne!(reach["state"], "closed", "{roster}: {line}");
        assert!(!line.contains("not running"), "{roster}: {line}");
    }
}

// ------------------------------------------------------------------ the live-process guard

#[test]
fn any_roster_entry_for_the_chat_means_running_so_it_is_compacted_through_the_panel_not_from_outside() {
    let fx = big();
    for status in ["idle", "busy", "waiting", "shell", "completed", "a-status-nobody-has-seen"] {
        let roster = format!(r#"[{{"sessionId":"sid-1","status":"{status}"}}]"#);
        let (code, out, err) = compact(&fx, &[("STUB_ROSTER", &roster)], &["--force", "--dry-run"]);
        assert_eq!(code, 0, "{status}: {err}");
        assert!(out.contains("system/x: running") && out.contains(&format!("with status {status}")) && out.contains("through the panel"), "{out}");
        // Run for real, the driver (which does not exist in a test fixture) is the only thing tried.
        let (code, _, err) = compact(&fx, &[("STUB_ROSTER", &roster)], &["--force"]);
        assert_eq!(code, 1, "{status}: {err}");
        assert!(err.contains("panel driver") && err.contains("would not start"), "{err}");
    }
    assert!(calls(&fx).is_empty(), "claude -p is never run for a running chat");
    assert_eq!(boundaries(&fx), 0);
    assert!(run_log_has(&fx, "panel driver"), "the refusal is in the run log");
}

#[test]
fn another_chat_on_the_roster_does_not_make_this_one_running() {
    let fx = big();
    let roster = r#"[{"sessionId":"sid-other","status":"busy"}]"#;
    let (code, out, err) = compact(&fx, &[("STUB_ROSTER", roster)], &[]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn a_session_record_naming_the_chat_means_running_even_when_the_roster_is_empty() {
    let fx = big();
    fx.write("cfg/sessions/4242.json", r#"{"pid":4242,"sessionId":"sid-1","status":"shell"}"#);
    let (code, out, err) = compact(&fx, &[], &["--force", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("system/x: running") && out.contains("sessions/4242.json") && out.contains("through the panel"), "{out}");
    let (code, _, err) = compact(&fx, &[], &["--force"]);
    assert_eq!(code, 1, "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn session_records_for_other_chats_or_that_do_not_parse_are_ignored() {
    let fx = big();
    fx.write("cfg/sessions/1.json", r#"{"pid":1,"sessionId":"sid-other"}"#);
    fx.write("cfg/sessions/2.json", "{ this is not json");
    fx.write("cfg/sessions/3.json", r#"["sid-1"]"#);
    fx.write("cfg/sessions/4.key", "sid-1");
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn an_unavailable_roster_refuses_because_a_process_cannot_be_ruled_out() {
    let fx = big();
    let (code, _, err) = compact(&fx, &[("STUB_ROSTER_FAIL", "1")], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot tell whether it is running"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_missing_program_leaves_the_roster_unavailable_and_refuses() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    let missing = fx.path("no-such-program");
    let (code, _, err) = compact(&fx, &[("AF_CLAUDE_BIN", &missing.to_string_lossy())], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot tell whether it is running"), "{err}");
}

#[test]
fn a_chat_opened_during_the_run_is_an_error_even_when_the_compaction_succeeded() {
    let fx = big();
    let record = fx.p("cfg/sessions/777.json");
    let (code, out, err) = compact(&fx, &[("STUB_WRITE_SESSION", &record)], &[]);
    assert_eq!(code, 1, "{out}{err}");
    for part in ["system/x is live now", "opened during the compaction", "two processes may have written its transcript", "Close that chat and reopen it", "succeeded"] {
        assert!(err.contains(part), "{part}: {err}");
    }
    assert!(!out.contains("compacted."), "no success report: {out}");
    assert_eq!(boundaries(&fx), 1, "the compaction did run");
    assert!(run_log_has(&fx, "live now"));
}

#[test]
fn a_chat_opened_during_a_failed_run_gets_the_warning_and_the_failure() {
    let fx = big();
    let record = fx.p("cfg/sessions/777.json");
    let (code, _, err) = compact(&fx, &[("STUB_WRITE_SESSION", &record), ("STUB_NO_BOUNDARY", "1")], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("is live now") && err.contains("no compact_boundary line"), "{err}");
}

// ------------------------------------------------------------------ the size

#[test]
fn under_fifty_thousand_tokens_is_skipped_and_force_runs_it() {
    let fx = chat(Some("claude-opus-5-5"), 49_999);
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("nothing worth compacting"), "{out}");
    assert!(calls(&fx).is_empty());
    assert_eq!(boundaries(&fx), 0);
    let (code, out, err) = compact(&fx, &[], &["--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(boundaries(&fx), 1);
    assert_eq!(calls(&fx).len(), 1);
    let exactly = chat(Some("claude-opus-5-5"), 50_000);
    assert!(compact(&exactly, &[], &[]).1.contains("compacted"));
}

#[test]
fn no_transcript_and_an_unknown_name_are_refused() {
    let fx = registered();
    let (code, _, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("no transcript found"), "{err}");
    let o = fx.af().env("AF_CLAUDE_BIN", stub()).args(["session", "compact", "system/nobody"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no registry row for system/nobody"));
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_transcript_with_no_usage_needs_force() {
    let fx = registered();
    fx.write(TRANSCRIPT, "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\n");
    let (code, _, err) = compact(&fx, &[], &["--model", "claude-opus-5-5"]);
    assert_eq!(code, 1);
    assert!(err.contains("size is unknown"), "{err}");
    let (code, out, err) = compact(&fx, &[], &["--model", "claude-opus-5-5", "--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Before unknown tokens"), "{out}");
}

// ------------------------------------------------------------------ the summary model

fn attachment(kind: &str, model: &str) -> String {
    json!({"type": "attachment", "attachment": {"type": kind, "model": model}}).to_string()
}

#[test]
fn the_model_is_the_last_replys_requested_id_with_its_bracket_and_says_where_it_came_from() {
    let side = json!({"type": "assistant", "isSidechain": true, "requestedModel": "claude-sonnet-5-5", "message": {"model": "claude-sonnet-5-5", "usage": {"input_tokens": 9000}}});
    let fx = with_lines(&[
        assistant_requested(Some("claude-sonnet-5-5"), Some("claude-sonnet-5-5"), 80_000),
        assistant_requested(Some("claude-fable-5-2"), Some("claude-fable-5-2[1m]"), 200_000),
        side.to_string(),
    ]);
    assert_eq!(compact(&fx, &[], &[]).0, 0);
    assert_eq!(model_arg(&fx, 0), "claude-fable-5-2[1m]");
    // No reply since the first compaction, so the next ones need --force.
    let (code, out, _) = compact(&fx, &[], &["--model", "claude-opus-5-5", "--dry-run", "--force"]);
    assert_eq!(code, 0);
    assert!(out.contains("model claude-opus-5-5 (from --model)"), "{out}");
    assert_eq!(compact(&fx, &[], &["--model", "claude-opus-5-5", "--force"]).0, 0);
    assert_eq!(model_arg(&fx, 1), "claude-opus-5-5");
}

#[test]
fn a_reply_that_requested_the_plain_id_stays_plain_over_an_older_bracketed_record() {
    let fx = with_lines(&[
        attachment("thinking_drop", "claude-opus-5-5[1m]"),
        assistant_requested(Some("claude-opus-5-5"), Some("claude-opus-5-5"), 120_000),
    ]);
    assert_eq!(compact(&fx, &[], &[]).0, 0);
    assert_eq!(model_arg(&fx, 0), "claude-opus-5-5");
}

#[test]
fn a_reply_with_no_requested_id_takes_a_recent_bracketed_record_of_the_same_base() {
    let plain = json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "usage": {"input_tokens": 1, "cache_read_input_tokens": 119_999}}}).to_string();
    let fx = with_lines(&[plain.clone(), attachment("thinking_drop", "claude-opus-5-5[1m]")]);
    let (code, out, err) = compact(&fx, &[], &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("model claude-opus-5-5[1m] (from a recent attachment record)"), "{out}");
    // A bracketed id of another model is not this chat's model.
    let other = with_lines(&[plain, attachment("thinking_drop", "claude-fable-5-2[1m]")]);
    let (_, out, _) = compact(&other, &[], &["--dry-run"]);
    assert!(out.contains("model claude-opus-5-5 (from the model of its last reply)"), "{out}");
}

#[test]
fn no_recorded_model_and_no_flag_asks_for_one() {
    let fx = chat(None, 100_000);
    let (code, _, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("--model"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_haiku_summary_is_refused_unless_forced() {
    let fx = chat(Some("claude-haiku-4-5"), 100_000);
    let (code, _, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("claude-haiku-4-5") && err.contains("--force"), "{err}");
    let (code, _, err) = compact(&fx, &[], &["--model", "Claude-Haiku-4-5"]);
    assert_eq!(code, 1, "{err}");
    assert!(calls(&fx).is_empty());
    let (code, out, err) = compact(&fx, &[], &["--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(calls(&fx).len(), 1);
}

// ------------------------------------------------------------------ dry run

#[test]
fn dry_run_prints_the_command_the_model_and_the_tokens_and_runs_nothing() {
    let fx = big();
    let before = fx.read(TRANSCRIPT);
    let (code, out, err) = compact(&fx, &[], &["--dry-run", "--instructions", "keep the plan"]);
    assert_eq!(code, 0, "{out}{err}");
    for part in ["-p \"/compact keep the plan\"", "--resume sid-1", "--model claude-opus-5-5[1m]", "--output-format json", "312k tokens", "from requestedModel of its last reply"] {
        assert!(out.contains(part), "{part}: {out}");
    }
    assert!(calls(&fx).is_empty());
    assert_eq!(fx.read(TRANSCRIPT), before);
}

#[test]
fn a_dry_run_on_a_running_chat_prints_the_panel_plan_and_runs_nothing() {
    let fx = big();
    let before = fx.read(TRANSCRIPT);
    let (code, out, err) = compact(&fx, &[("STUB_ROSTER", IDLE)], &["--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("through the panel") && out.contains("send -Session sid-1"), "{out}");
    assert!(calls(&fx).is_empty());
    assert_eq!(fx.read(TRANSCRIPT), before);
}

// ------------------------------------------------------------------ the panel driver after a closed-chat compaction

fn panel_calls(fx: &Fx) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(fx.path("panel-calls.txt")) else { return Vec::new() };
    text.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn with_panel<'a>(fx: &Fx, env: &'a [(&'a str, &'a str)]) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    all.push(("AF_PANEL_DRIVER".into(), panel_stub().to_string_lossy().into_owned()));
    all.push(("STUB_PANEL_CALLS".into(), fx.p("panel-calls.txt")));
    all
}

#[test]
fn then_file_follows_a_closed_chat_compaction_as_the_chats_next_message_through_the_driver() {
    let fx = big();
    fx.write("then.md", "Continue the build.
Read handoff.md first.");
    let owned = with_panel(&fx, &[]);
    let env: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = compact(&fx, &env, &["--then-file", &fx.p("then.md")]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: compacted.") && out.contains("as its next message"), "{out}");
    assert_eq!(boundaries(&fx), 1);
    let made = panel_calls(&fx);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!(made[0]["args"][0], "send");
    assert_eq!(made[0]["args"][2], "sid-1");
    assert_eq!(made[0]["text"], "Continue the build.
Read handoff.md first.");
}

#[test]
fn then_file_is_not_sent_when_the_closed_chat_compaction_failed() {
    let fx = big();
    fx.write("then.md", "go on");
    let owned = with_panel(&fx, &[("STUB_NO_BOUNDARY", "1")]);
    let env: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, _, err) = compact(&fx, &env, &["--then-file", &fx.p("then.md")]);
    assert_eq!(code, 1, "{err}");
    assert!(panel_calls(&fx).is_empty());
}

#[test]
fn when_idle_on_a_closed_chat_compacts_it_at_once_there_is_nothing_to_wait_for() {
    let fx = big();
    let owned = with_panel(&fx, &[]);
    let env: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = compact(&fx, &env, &["--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: compacted."), "{out}");
    assert_eq!(boundaries(&fx), 1);
    assert!(!fx.exists("system/logs/panel-waiters.log"));
}

#[test]
fn a_waiter_job_runs_the_compaction_again_by_the_route_the_chat_has_then_and_logs_it() {
    // The waiter chooses its route when it runs: this chat closed meanwhile, so it goes from outside.
    let fx = big();
    let job = fx.write("system/logs/claude-panel/waiter-1.json", r#"{"name":"system/x","instructions":"keep the plan","force":false,"then_file":null,"wait_minutes":60}"#);
    let (code, out, err) = compact(&fx, &[], &["--waiter-job", &job]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(boundaries(&fx), 1);
    assert_eq!(calls(&fx)[0].1[1], "/compact keep the plan");
    let log = fx.read("system/logs/panel-waiters.log");
    assert!(log.contains("system/x compact ok: system/x: compacted."), "{log}");
    assert!(!fx.exists("system/logs/claude-panel/waiter-1.json"), "the job file is consumed");
}

#[test]
fn a_waiter_job_that_fails_is_logged_as_failed() {
    let fx = big();
    let job = fx.write("system/logs/claude-panel/waiter-2.json", r#"{"name":"system/x","instructions":null,"force":false,"then_file":null,"wait_minutes":1}"#);
    let (code, _, _) = compact(&fx, &[("STUB_NO_BOUNDARY", "1")], &["--waiter-job", &job]);
    assert_eq!(code, 1);
    assert!(fx.read("system/logs/panel-waiters.log").contains("system/x compact failed: system/x: compaction failed"));
}

// ------------------------------------------------------------------ failures

#[test]
fn a_reported_error_exits_one_and_shows_the_output() {
    let fx = big();
    let result = r#"{"type":"result","is_error":true,"result":"Prompt is too long","total_cost_usd":0}"#;
    let (code, _, err) = compact(&fx, &[("STUB_OUTPUT", result)], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("compaction failed") && err.contains("is_error: true") && err.contains("Prompt is too long"), "{err}");
}

#[test]
fn a_non_zero_exit_is_a_failure_even_with_is_error_false_and_a_boundary_line() {
    let fx = big();
    let (code, out, err) = compact(&fx, &[("STUB_EXIT", "3")], &[]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("the command exited 3") && err.contains("\"is_error\":false"), "{err}");
    assert!(!out.contains("compacted."), "{out}");
}

#[test]
fn no_boundary_line_is_a_failure_even_when_the_output_says_success() {
    let fx = big();
    let (code, out, err) = compact(&fx, &[("STUB_NO_BOUNDARY", "1")], &[]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("no compact_boundary line") && err.contains("\"is_error\":false"), "{err}");
    assert!(!out.contains("compacted."), "{out}");
}

#[test]
fn output_that_is_not_json_fails_and_only_the_first_300_characters_show() {
    let fx = big();
    let noise = format!("{}{}", "x".repeat(300), "TAIL-MARKER");
    let (code, _, err) = compact(&fx, &[("STUB_OUTPUT", &noise)], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("no JSON result"), "{err}");
    assert!(err.contains(&"x".repeat(300)) && !err.contains("TAIL-MARKER"), "{err}");
}

#[test]
fn json_after_other_output_lines_is_parsed_and_the_last_json_line_wins() {
    let ok = r#"{"type":"result","is_error":false,"num_turns":0,"total_cost_usd":0.5}"#;
    let bad = r#"{"type":"result","is_error":true,"result":"nope"}"#;
    let fx = big();
    let (code, out, err) = compact(&fx, &[("STUB_OUTPUT", &format!("starting up\nwarning: slow disk\n{ok}"))], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("$0.50"), "{out}");
    let fx = big();
    let (code, out, err) = compact(&fx, &[("STUB_OUTPUT", &format!("{bad}\n{ok}"))], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let fx = big();
    let (code, _, err) = compact(&fx, &[("STUB_OUTPUT", &format!("{ok}\n{bad}"))], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("is_error: true"), "{err}");
}

// ------------------------------------------------------------------ the deadline

#[test]
fn a_command_past_the_deadline_is_stopped_and_the_transcript_is_checked() {
    let started = std::time::Instant::now();
    let fx = big();
    let (code, out, err) = compact(&fx, &[("AF_COMPACT_DEADLINE_SECS", "1"), ("STUB_SLEEP_SECS", "60")], &[]);
    assert_eq!(code, 1, "{out}");
    assert!(started.elapsed() < std::time::Duration::from_secs(40), "the stub was killed, not waited for");
    assert!(err.contains("ran past 1 second") && err.contains("stopped"), "{err}");
    assert!(err.contains("the transcript changed and has a compact_boundary line"), "{err}");
    assert!(!out.contains("compacted."), "{out}");
    assert!(run_log_has(&fx, "ran past"));

    let quiet = big();
    let (code, _, err) = compact(&quiet, &[("AF_COMPACT_DEADLINE_SECS", "1"), ("STUB_SLEEP_SECS", "60"), ("STUB_NO_BOUNDARY", "1")], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("the transcript did not change and has no compact_boundary line"), "{err}");
}
