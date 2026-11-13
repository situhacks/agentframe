//! `af session compact`: compacting a closed chat from outside it. The program it runs is a stub
//! compiled for the test and named through `AF_CLAUDE_BIN`; the real `claude` is never called.
//! The stub answers `agents` with the roster in `STUB_ROSTER`, and otherwise records its
//! arguments in `STUB_CALLS`, appends a `compact_boundary` line to `STUB_TRANSCRIPT`, and prints
//! `STUB_OUTPUT` (a success result by default).

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
        append(&calls, &format!("{}\n{}\n", cwd.display(), args.join("\u{1f}")));
    }
    if let Ok(t) = std::env::var("STUB_TRANSCRIPT") {
        if std::env::var("STUB_NO_BOUNDARY").is_err() {
            append(&t, "{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"content\":\"Conversation compacted\"}\n");
        }
        if let Ok(line) = std::env::var("STUB_AFTER_LINE") {
            append(&t, &format!("{line}\n"));
        }
    }
    println!("{}", std::env::var("STUB_OUTPUT").unwrap_or_else(|_| {
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":0,\"session_id\":\"sid-1\",\"total_cost_usd\":0.4321}".into()
    }));
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

fn assistant(model: Option<&str>, total: u64) -> String {
    let mut message = json!({"role": "assistant", "usage": {"input_tokens": 1, "cache_read_input_tokens": total - 1, "output_tokens": 5}});
    if let Some(m) = model {
        message["model"] = json!(m);
    }
    json!({"type": "assistant", "message": message}).to_string()
}

const TRANSCRIPT: &str = "cfg/projects/p1/sid-1.jsonl";

/// A registered chat `system/x` (session `sid-1`) whose transcript holds one assistant line.
fn chat(model: Option<&str>, tokens: u64) -> Fx {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx.write(TRANSCRIPT, &format!("{}\n", assistant(model, tokens)));
    fx
}

/// A chat with a 312k-token context on a 1M model.
fn big() -> Fx {
    chat(Some("claude-opus-5-5[1m]"), 312_000)
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

/// The arguments of each stub call, and where it ran.
fn calls(fx: &Fx) -> Vec<(String, Vec<String>)> {
    let Ok(text) = std::fs::read_to_string(fx.path("calls.txt")) else { return Vec::new() };
    let lines: Vec<&str> = text.lines().collect();
    lines.chunks(2).map(|c| (c[0].to_string(), c[1].split('\u{1f}').map(str::to_string).collect())).collect()
}

fn boundaries(fx: &Fx) -> usize {
    fx.read(TRANSCRIPT).lines().filter(|l| l.contains("compact_boundary")).count()
}

#[test]
fn a_closed_chat_is_compacted_with_the_resume_command_from_the_vault_root() {
    let fx = chat(Some("claude-opus-5-5[1m]"), 312_000);
    let (code, out, err) = compact(&fx, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(boundaries(&fx), 1);
    for part in ["system/x: compacted", "312k", "claude-opus-5-5[1m]", "$0.43", "next turn"] {
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
    let after = assistant(Some("claude-opus-5-5[1m]"), 41_000);
    let (code, out, err) = compact(&fx, &[("STUB_AFTER_LINE", &after)], &["--instructions", "keep the plan path"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(calls(&fx)[0].1[1], "/compact keep the plan path");
    assert!(out.contains("context after: 41k tokens"), "{out}");
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
fn a_running_chat_is_refused_and_the_program_is_never_called() {
    let fx = big();
    for status in ["idle", "busy", "waiting"] {
        let roster = format!(r#"[{{"sessionId":"sid-1","status":"{status}"}}]"#);
        let (code, _, err) = compact(&fx, &[("STUB_ROSTER", &roster)], &["--force"]);
        assert_eq!(code, 1, "{status}: {err}");
        assert!(err.contains("system/x is running"), "{err}");
        assert!(err.contains("Close its tab and rerun, or type /compact in that chat"), "{err}");
    }
    assert!(calls(&fx).is_empty());
    assert_eq!(boundaries(&fx), 0);
    assert!(
        fx.run_log_rows().iter().any(|r| r.get("refusal").and_then(Value::as_str).is_some_and(|m| m.contains("is running"))),
        "the refusal is in the run log"
    );
}

#[test]
fn another_chat_on_the_roster_does_not_make_this_one_running() {
    let fx = big();
    let roster = r#"[{"sessionId":"sid-other","status":"busy"},{"sessionId":"sid-1","status":"completed"}]"#;
    let (code, out, err) = compact(&fx, &[("STUB_ROSTER", roster)], &[]);
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
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
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
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx.write(TRANSCRIPT, "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\n");
    let (code, _, err) = compact(&fx, &[], &["--model", "claude-opus-5-5"]);
    assert_eq!(code, 1);
    assert!(err.contains("size is unknown"), "{err}");
    let (code, out, err) = compact(&fx, &[], &["--model", "claude-opus-5-5", "--force"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Before unknown tokens"), "{out}");
}

#[test]
fn the_model_defaults_to_the_last_reply_with_its_bracket_suffix_and_an_explicit_one_wins() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    let side = json!({"type": "assistant", "isSidechain": true, "message": {"model": "claude-sonnet-5-5", "usage": {"input_tokens": 9000}}});
    fx.write(
        TRANSCRIPT,
        &format!("{}\n{}\n{side}\n", assistant(Some("claude-sonnet-5-5"), 80_000), assistant(Some("claude-fable-5-2[1m]"), 200_000)),
    );
    assert_eq!(compact(&fx, &[], &[]).0, 0);
    assert_eq!(calls(&fx)[0].1[5], "claude-fable-5-2[1m]");
    assert_eq!(compact(&fx, &[], &["--model", "claude-opus-5-5"]).0, 0);
    assert_eq!(calls(&fx)[1].1[5], "claude-opus-5-5");
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

#[test]
fn dry_run_prints_the_command_the_model_and_the_tokens_and_runs_nothing() {
    let fx = chat(Some("claude-opus-5-5[1m]"), 312_000);
    let before = fx.read(TRANSCRIPT);
    let (code, out, err) = compact(&fx, &[], &["--dry-run", "--instructions", "keep the plan"]);
    assert_eq!(code, 0, "{out}{err}");
    for part in ["-p \"/compact keep the plan\"", "--resume sid-1", "--model claude-opus-5-5[1m]", "--output-format json", "312k tokens"] {
        assert!(out.contains(part), "{part}: {out}");
    }
    assert!(calls(&fx).is_empty());
    assert_eq!(fx.read(TRANSCRIPT), before);
}

#[test]
fn a_dry_run_still_refuses_a_running_chat() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    assert_eq!(compact(&fx, &[("STUB_ROSTER", IDLE)], &["--dry-run"]).0, 1);
}

#[test]
fn a_reported_error_exits_one_and_shows_the_output() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    let result = r#"{"type":"result","is_error":true,"result":"Prompt is too long","total_cost_usd":0}"#;
    let (code, _, err) = compact(&fx, &[("STUB_OUTPUT", result)], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("compaction failed") && err.contains("is_error: true") && err.contains("Prompt is too long"), "{err}");
}

#[test]
fn no_boundary_line_is_a_failure_even_when_the_output_says_success() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    let (code, out, err) = compact(&fx, &[("STUB_NO_BOUNDARY", "1")], &[]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("no compact_boundary line") && err.contains("\"is_error\":false"), "{err}");
    assert!(!out.contains("compacted"), "{out}");
}

#[test]
fn output_that_is_not_json_fails_and_only_the_first_300_characters_show() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    let noise = format!("{}{}", "x".repeat(300), "TAIL-MARKER");
    let (code, _, err) = compact(&fx, &[("STUB_OUTPUT", &noise)], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("no JSON result"), "{err}");
    assert!(err.contains(&"x".repeat(300)) && !err.contains("TAIL-MARKER"), "{err}");
}

#[test]
fn a_missing_program_leaves_the_roster_unavailable_and_refuses() {
    let fx = chat(Some("claude-opus-5-5"), 312_000);
    let missing = fx.path("no-such-program");
    let (code, _, err) = compact(&fx, &[("AF_CLAUDE_BIN", &missing.to_string_lossy())], &[]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot tell whether it is running"), "the roster call fails first: {err}");
}
