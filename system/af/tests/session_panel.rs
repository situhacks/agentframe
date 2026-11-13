//! The session verbs that use the VS Code panel driver: compacting a running chat, `--when-idle`,
//! `--then-file`, `rename`, `start` (with `--group`) and `panel`. The driver is a stub compiled for the test and named
//! through `AF_PANEL_DRIVER` (see `common::panel_stub`); the real driver is never run, no window is
//! touched and no key is sent. The same stub answers `agents` for `AF_CLAUDE_BIN`, so the roster says
//! whether the chat is running. It records each call (arguments and the text file's content) in
//! `STUB_PANEL_CALLS`.

mod common;
use common::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const TRANSCRIPT: &str = "cfg/projects/p1/sid-1.jsonl";
const RUNNING: &str = r#"[{"sessionId":"sid-1","status":"idle"}]"#;

fn assistant(total: u64) -> String {
    json!({"type": "assistant", "message": {"role": "assistant", "model": "claude-opus-5-5", "usage": {"input_tokens": 1, "cache_read_input_tokens": total - 1, "output_tokens": 5}}}).to_string()
}

/// A registered chat `system/x` (session `sid-1`) with a 312k-token transcript.
fn chat() -> Fx {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx.write(TRANSCRIPT, &format!("{}\n", assistant(312_000)));
    fx
}

fn stub_env(fx: &Fx, c: &mut assert_cmd::Command) {
    c.env("CLAUDE_CONFIG_DIR", fx.path("cfg"))
        .env("AF_CLAUDE_BIN", panel_stub())
        .env("AF_PANEL_DRIVER", panel_stub())
        .env("STUB_PANEL_CALLS", fx.path("calls.txt"))
        .env("STUB_PANEL_TRANSCRIPT", fx.path(TRANSCRIPT))
        .env("STUB_ROSTER", RUNNING)
        .env("AF_COMPACT_BOUNDARY_SECS", "2")
        .env("AF_PANEL_READBACK_MS", "10");
}

/// Run `af session <args>` with the stubs; (exit code, stdout, stderr).
fn session(fx: &Fx, env: &[(&str, &str)], args: &[&str]) -> (i32, String, String) {
    let mut c = fx.af();
    stub_env(fx, &mut c);
    for (k, v) in env {
        c.env(k, v);
    }
    let mut full = vec!["session"];
    full.extend_from_slice(args);
    let o = c.args(full).output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every driver call: its arguments and the text it was handed.
fn calls(fx: &Fx) -> Vec<(Vec<String>, Option<String>)> {
    let Ok(text) = std::fs::read_to_string(fx.path("calls.txt")) else { return Vec::new() };
    text.lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            let args = v["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
            (args, v["text"].as_str().map(str::to_string))
        })
        .collect()
}

fn sends(fx: &Fx) -> Vec<(Vec<String>, Option<String>)> {
    calls(fx).into_iter().filter(|(a, _)| a[0] == "send").collect()
}

fn arg_after(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn boundaries(fx: &Fx) -> usize {
    fx.read(TRANSCRIPT).lines().filter(|l| l.contains("compact_boundary")).count()
}

/// Wait for a file under the root to hold `needle`; its content.
fn wait_for(fx: &Fx, rel: &str, needle: &str) -> String {
    let until = Instant::now() + Duration::from_secs(40);
    loop {
        let text = std::fs::read_to_string(fx.path(rel)).unwrap_or_default();
        if text.contains(needle) || Instant::now() > until {
            return text;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

// ------------------------------------------------------------------ compact, a running chat

#[test]
fn a_running_chat_is_compacted_by_typing_compact_through_the_driver() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[], &["compact", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: compacted through the panel") && out.contains("312k"), "{out}");
    assert_eq!(boundaries(&fx), 1);
    let made = calls(&fx);
    assert_eq!(made.len(), 1, "one driver call and no claude -p: {made:?}");
    let (args, text) = &made[0];
    assert_eq!(args[0], "send");
    assert_eq!(arg_after(args, "-Session").as_deref(), Some("sid-1"));
    assert!(arg_after(args, "-TextFile").is_some());
    assert_eq!(arg_after(args, "-WaitMinutes"), None, "the driver's own default wait applies");
    assert_eq!(text.as_deref(), Some("/compact"), "the slash text reaches the driver unchanged");
}

#[test]
fn the_instructions_arrive_in_the_file_with_quotes_and_newlines_intact() {
    let fx = chat();
    let instr = "say \"hi\" to \"him\"\nthen 'stop'\n\nand done";
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--instructions", instr]);
    assert_eq!(code, 0, "{out}{err}");
    let (args, text) = &sends(&fx)[0];
    assert_eq!(text.as_deref(), Some(format!("/compact {instr}").as_str()));
    assert!(!args.iter().any(|a| a.contains("say")), "the text is not an argument: {args:?}");
}

#[test]
fn the_text_file_is_removed_once_the_driver_returns() {
    let fx = chat();
    assert_eq!(session(&fx, &[], &["compact", "system/x"]).0, 0);
    let dir = fx.path("system/logs/claude-panel");
    let left: Vec<_> = std::fs::read_dir(dir).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn a_closed_chat_still_takes_the_closed_route_and_never_the_driver() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_dry_run_on_a_running_chat_shows_the_driver_call_and_runs_nothing() {
    let fx = chat();
    let before = fx.read(TRANSCRIPT);
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--dry-run", "--instructions", "keep the plan", "--then-file", &fx.p("then.md")]);
    assert_eq!(code, 1, "the then-file does not exist yet: {out}{err}");
    fx.write("then.md", "go on");
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--dry-run", "--instructions", "keep the plan", "--then-file", &fx.p("then.md"), "--model", "claude-opus-5-5"]);
    assert_eq!(code, 0, "{out}{err}");
    for part in ["through the panel", "send -Session sid-1 -TextFile", "\"/compact keep the plan\"", "compact_boundary", "312k tokens", "next message", "--model has no effect"] {
        assert!(out.contains(part), "{part}: {out}");
    }
    assert!(calls(&fx).is_empty());
    assert_eq!(fx.read(TRANSCRIPT), before);
}

#[test]
fn a_session_record_that_names_the_chat_counts_as_running_and_is_named_in_the_plan() {
    let fx = chat();
    fx.write("cfg/sessions/4242.json", r#"{"pid":4242,"sessionId":"sid-1","status":"idle"}"#);
    let (code, out, err) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("sessions/4242.json") && out.contains("through the panel"), "{out}");
}

#[test]
fn an_unavailable_roster_still_refuses_because_a_process_cannot_be_ruled_out() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_ROSTER_FAIL", "1")], &["compact", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot tell whether it is running"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_chat_under_fifty_thousand_tokens_is_not_compacted_through_the_panel_unless_forced() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx.write(TRANSCRIPT, &format!("{}\n", assistant(40_000)));
    let (code, out, _) = session(&fx, &[], &["compact", "system/x"]);
    assert_eq!(code, 0);
    assert!(out.contains("nothing worth compacting"), "{out}");
    assert!(calls(&fx).is_empty());
    assert_eq!(session(&fx, &[], &["compact", "system/x", "--force"]).0, 0);
    assert_eq!(sends(&fx).len(), 1);
}

#[test]
fn the_driver_wait_is_passed_when_asked_for() {
    let fx = chat();
    assert_eq!(session(&fx, &[], &["compact", "system/x", "--wait-minutes", "7.5"]).0, 0);
    assert_eq!(arg_after(&sends(&fx)[0].0, "-WaitMinutes").as_deref(), Some("7.5"));
}

// ------------------------------------------------------------------ the driver's refusals

#[test]
fn driver_exit_codes_become_clear_refusals_and_nothing_else_runs() {
    let cases = [
        ("3", "exit 3", "operator was using the computer"),
        ("4", "exit 4", "draft typed in its input box"),
        ("5", "exit 5", "safety check failed, so nothing was sent"),
        ("6", "exit 6", "MODEL KEY MISMATCH"),
        ("2", "could not find", "no chat row for session sid-1"),
        ("1", "failed (exit 1)", "the stub driver refused"),
    ];
    for (code, label, extra) in cases {
        let fx = chat();
        let stderr = if code == "2" { "no chat row for session sid-1 in the panel" } else { "the stub driver refused" };
        let (exit, out, err) = session(&fx, &[("STUB_PANEL_EXIT", code), ("STUB_PANEL_STDERR", stderr)], &["compact", "system/x"]);
        assert_eq!(exit, 1, "{code}: {out}{err}");
        assert!(err.contains("system/x: ") && err.contains(label) && err.contains(extra), "{code}: {err}");
        assert!(err.contains("nothing was sent") || code == "2" || code == "1" || code == "6", "{code}: {err}");
        assert_eq!(boundaries(&fx), 0);
        assert_eq!(calls(&fx).len(), 1);
        assert!(fx.run_log_rows().iter().any(|r| r.get("refusal").and_then(Value::as_str).is_some_and(|m| m.contains(label) || m.contains("could not find"))), "{code}: in the run log");
    }
}

#[test]
fn a_missing_driver_program_is_a_refusal_not_a_hang() {
    let fx = chat();
    let missing = fx.p("no-driver-here");
    let (code, _, err) = session(&fx, &[("AF_PANEL_DRIVER", &missing)], &["compact", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("panel driver") && err.contains("would not start"), "{err}");
}

#[test]
fn without_a_boundary_line_the_compaction_fails_honestly() {
    let fx = chat();
    let started = Instant::now();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_NO_BOUNDARY", "1")], &["compact", "system/x"]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("sent to the chat") && err.contains("no compact_boundary line within 2 seconds"), "{err}");
    assert!(!out.contains("compacted"), "{out}");
    assert!(started.elapsed() < Duration::from_secs(30));
    assert_eq!(boundaries(&fx), 0);
}

#[test]
fn a_boundary_is_new_only_when_it_was_appended_after_the_send() {
    let fx = chat();
    let old = "{\"type\":\"system\",\"subtype\":\"compact_boundary\"}\n";
    fx.write(TRANSCRIPT, &format!("{old}{}\n", assistant(312_000)));
    let (code, _, err) = session(&fx, &[("STUB_PANEL_NO_BOUNDARY", "1")], &["compact", "system/x"]);
    assert_eq!(code, 1, "an older boundary does not count: {err}");
}

#[test]
fn the_context_after_comes_from_a_usage_line_written_after_the_boundary() {
    let fx = chat();
    let after = assistant(41_000);
    let (code, out, err) = session(&fx, &[("STUB_PANEL_AFTER_LINE", &after)], &["compact", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("context after: 41k tokens"), "{out}");
}

// ------------------------------------------------------------------ --when-idle

#[test]
fn when_idle_returns_at_once_and_the_waiter_sends_the_compact_and_logs_its_result() {
    let fx = chat();
    let started = Instant::now();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_SLEEP_SECS", "4")], &["compact", "system/x", "--when-idle", "--instructions", "keep the plan"]);
    let took = started.elapsed();
    assert_eq!(code, 0, "{out}{err}");
    assert!(took < Duration::from_secs(3), "returned in {took:?}, not after the 4 second driver call");
    for part in ["detached waiter", "within 60 minutes", "panel-waiters.log"] {
        assert!(out.contains(part), "{part}: {out}");
    }
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("system/x compact ok: system/x: compacted through the panel"), "{log}");
    let made = sends(&fx);
    assert_eq!(made.len(), 1, "{made:?}");
    let w: f64 = arg_after(&made[0].0, "-WaitMinutes").unwrap().parse().unwrap();
    assert!((59.0..=60.0).contains(&w), "what is left of the 60 minutes: {w}");
    assert_eq!(made[0].1.as_deref(), Some("/compact keep the plan"));
    assert_eq!(boundaries(&fx), 1);
    assert!(fx.read("system/logs/panel-waiters.log").lines().count() == 1);
    let jobs: Vec<_> = std::fs::read_dir(fx.path("system/logs/claude-panel")).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(jobs.is_empty(), "the job file is gone: {jobs:?}");
}

#[test]
fn the_waiters_failure_lands_in_the_log_too_and_wait_minutes_sets_its_budget() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_EXIT", "3")], &["compact", "system/x", "--when-idle", "--wait-minutes", "5"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("within 5 minutes"), "{out}");
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact failed");
    assert!(log.contains("system/x compact failed: ") && log.contains("exit 3"), "{log}");
    let w: f64 = arg_after(&sends(&fx)[0].0, "-WaitMinutes").unwrap().parse().unwrap();
    assert!((4.0..=5.0).contains(&w), "what is left of the 5 minutes: {w}");
    assert_eq!(boundaries(&fx), 0);
}

#[test]
fn when_idle_dry_run_describes_the_waiter_and_starts_none() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("would start a detached waiter") && out.contains("within 60 minutes"), "{out}");
    assert!(!fx.exists("system/logs/panel-waiters.log") && calls(&fx).is_empty());
}

// ------------------------------------------------------------------ --then-file

#[test]
fn then_file_is_sent_after_a_confirmed_compaction_as_the_next_message() {
    let fx = chat();
    let then = "Continue the build.\nRead \"handoff.md\" first.";
    fx.write("then.md", then);
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--then-file", &fx.p("then.md"), "--wait-minutes", "3"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("compacted through the panel") && out.contains("as its next message"), "{out}");
    let made = sends(&fx);
    assert_eq!(made.len(), 2, "{made:?}");
    assert_eq!(made[0].1.as_deref(), Some("/compact"));
    assert_eq!(made[1].1.as_deref(), Some(then));
    assert_eq!(arg_after(&made[1].0, "-Session").as_deref(), Some("sid-1"));
    assert_eq!(arg_after(&made[1].0, "-WaitMinutes").as_deref(), Some("3"), "the same guards and wait");
}

#[test]
fn then_file_is_not_sent_when_the_compaction_was_not_confirmed() {
    let fx = chat();
    fx.write("then.md", "go on");
    let (code, _, err) = session(&fx, &[("STUB_PANEL_NO_BOUNDARY", "1")], &["compact", "system/x", "--then-file", &fx.p("then.md")]);
    assert_eq!(code, 1, "{err}");
    assert_eq!(sends(&fx).len(), 1, "only the /compact went out");
}

#[test]
fn a_refused_then_file_send_is_reported_after_the_compaction_which_did_happen() {
    let fx = chat();
    fx.write("then.md", "go on");
    let (code, out, err) = session(&fx, &[("STUB_PANEL_EXITS", "0,4")], &["compact", "system/x", "--then-file", &fx.p("then.md")]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("compacted, but the next message") && err.contains("exit 4") && err.contains("draft"), "{err}");
    assert_eq!(boundaries(&fx), 1);
    assert_eq!(sends(&fx).len(), 2);
}

#[test]
fn a_then_file_that_is_missing_or_empty_is_refused_before_anything_is_sent() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[], &["compact", "system/x", "--then-file", &fx.p("nope.md")]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot read --then-file"), "{err}");
    fx.write("empty.md", "  \n");
    let (code, _, err) = session(&fx, &[], &["compact", "system/x", "--then-file", &fx.p("empty.md")]);
    assert_eq!(code, 1);
    assert!(err.contains("is empty"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn a_then_file_text_reaches_the_driver_exactly_as_written_whether_or_not_it_starts_with_a_slash() {
    for (content, typed) in [("/status", "/status"), (" /status", " /status"), ("status", "status")] {
        let fx = chat();
        fx.write("then.md", content);
        assert_eq!(session(&fx, &[], &["compact", "system/x", "--then-file", &fx.p("then.md")]).0, 0);
        assert_eq!(sends(&fx)[1].1.as_deref(), Some(typed), "{content:?}");
    }
}

#[test]
fn then_file_is_not_sent_after_a_closed_chat_compaction_that_failed() {
    // A closed chat is compacted from outside by `claude -p`; here that program is the panel stub, which
    // prints no JSON result, so the compaction fails and the driver is never reached.
    let fx = chat();
    fx.write("then.md", "go on");
    let (code, _, err) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--then-file", &fx.p("then.md")]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("compaction failed"), "{err}");
    assert!(sends(&fx).is_empty(), "no message after an unconfirmed compaction");
}

// ------------------------------------------------------------------ rename

#[test]
fn rename_types_the_registry_name_by_default_and_reads_the_panel_row_back() {
    let fx = chat();
    let list = r#"[{"session_id":"sid-1","name":"system/x","status":"idle","active":false}]"#;
    let (code, out, err) = session(&fx, &[("STUB_PANEL_LIST", list)], &["rename", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: renamed to system/x"), "{out}");
    let (args, text) = &sends(&fx)[0];
    assert_eq!(arg_after(args, "-Session").as_deref(), Some("sid-1"));
    assert_eq!(text.as_deref(), Some("/rename system/x"));
}

#[test]
fn rename_to_sends_that_text_and_fails_when_the_row_does_not_show_it() {
    let fx = chat();
    let stale = r#"[{"session_id":"sid-1","name":"old title","status":"idle","active":false}]"#;
    let (code, _, err) = session(&fx, &[("STUB_PANEL_LIST", stale)], &["rename", "system/x", "--to", "a \"new\" title"]);
    assert_eq!(code, 1);
    assert!(err.contains("the panel row shows 'old title'"), "{err}");
    assert_eq!(sends(&fx)[0].1.as_deref(), Some("/rename a \"new\" title"));
    let fresh = r#"[{"session_id":"sid-1","name":"a \"new\" title","status":"idle","active":false}]"#;
    let (code, out, err) = session(&fx, &[("STUB_PANEL_LIST", fresh)], &["rename", "system/x", "--to", "a \"new\" title"]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn rename_refuses_a_multi_line_or_empty_text_and_an_unknown_name() {
    let fx = chat();
    assert_eq!(session(&fx, &[], &["rename", "system/x", "--to", "two\nlines"]).0, 1);
    assert_eq!(session(&fx, &[], &["rename", "system/x", "--to", "  "]).0, 1);
    let (code, _, err) = session(&fx, &[], &["rename", "system/nobody"]);
    assert_eq!(code, 1);
    assert!(err.contains("no registry row for system/nobody"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn rename_turns_a_driver_refusal_into_a_refusal() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "4")], &["rename", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("system/x: ") && err.contains("draft"), "{err}");
}

// ------------------------------------------------------------------ start

#[test]
fn start_opens_a_new_chat_with_the_file_as_its_first_message_and_prints_the_session_id() {
    let fx = Fx::new();
    let prompt = "You are system/x.\nRead \"brief.md\", then register.\n";
    fx.write("prompt.md", prompt);
    let (code, out, err) = session(&fx, &[("STUB_PANEL_NEW_ID", "abcdef12-3456-7890-abcd-ef1234567890")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Opus 5.5"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("abcdef12-3456-7890-abcd-ef1234567890") && out.contains("af session register system/x --session abcdef12"), "{out}");
    let made = calls(&fx);
    assert_eq!(made.len(), 1);
    let (args, text) = &made[0];
    assert_eq!(args[0], "new");
    assert_eq!(arg_after(args, "-Model").as_deref(), Some("Opus 5.5"));
    assert_eq!(text.as_deref(), Some(prompt));
    assert_eq!(arg_after(args, "-Session"), None);
}

#[test]
fn start_without_a_model_refuses_and_lists_the_options_and_a_leading_slash_is_not_changed() {
    let fx = Fx::new();
    fx.write("prompt.md", "/status please");
    let (code, _, err) = session(&fx, &[], &["start", "worker/y", "--brief-prompt-file", &fx.p("prompt.md")]);
    assert_eq!(code, 1);
    assert!(err.contains("--model is needed") && err.contains("Haiku 4.5") && err.contains("Opus 5.5"), "{err}");
    assert!(calls(&fx).is_empty(), "nothing may reach the driver without a model");
    let (code, _, err) = session(&fx, &[], &["start", "worker/y", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "  "]);
    assert_eq!(code, 1, "{err}");
    assert!(calls(&fx).is_empty());
    let (code, out, err) = session(&fx, &[], &["start", "worker/y", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 4.5"]);
    assert_eq!(code, 0, "{out}{err}");
    let (args, text) = &calls(&fx)[0];
    assert_eq!(arg_after(args, "-Model").as_deref(), Some("Haiku 4.5"));
    assert_eq!(text.as_deref(), Some("/status please"));
}

#[test]
fn start_refuses_a_bad_name_a_missing_or_empty_file_and_a_driver_refusal() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    fx.write("empty.md", "\n");
    assert_eq!(session(&fx, &[], &["start", "bad name", "--brief-prompt-file", &fx.p("prompt.md")]).0, 1);
    assert_eq!(session(&fx, &[], &["start", "system/x", "--brief-prompt-file", &fx.p("missing.md")]).0, 1);
    assert_eq!(session(&fx, &[], &["start", "system/x", "--brief-prompt-file", &fx.p("empty.md")]).0, 1);
    assert!(calls(&fx).is_empty());
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "3")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 4.5"]);
    assert_eq!(code, 1);
    assert!(err.contains("system/x: ") && err.contains("exit 3"), "{err}");
}

#[test]
fn start_refuses_an_answer_that_is_not_a_session_id() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let (code, _, err) = session(&fx, &[("STUB_PANEL_NEW_ID", "not an id!")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 4.5"]);
    assert_eq!(code, 1);
    assert!(err.contains("printed no session id"), "{err}");
}

// ------------------------------------------------------------------ panel

#[test]
fn panel_lists_the_chats_joined_with_registry_names() {
    let fx = chat();
    let list = r#"[{"session_id":"sid-1","name":"system/x","status":"idle","active":true},{"session_id":"sid-9","name":"stray chat","status":"running","active":false}]"#;
    let (code, out, err) = session(&fx, &[("STUB_PANEL_LIST", list)], &["panel"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("| sid-1 | system/x | system/x | idle | yes |"), "{out}");
    assert!(out.contains("| sid-9 | stray chat | - | running | no |"), "{out}");
    let (code, out, _) = session(&fx, &[("STUB_PANEL_LIST", list)], &["panel", "--json"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v[0]["registry_name"], "system/x");
    assert_eq!(v[1]["registry_name"], "");
    assert_eq!(calls(&fx)[0].0, ["list", "-WaitMinutes", "1"], "a list waits a minute at most");
}

#[test]
fn panel_refuses_when_the_driver_does() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "2"), ("STUB_PANEL_STDERR", "no VS Code window with 'x' in its title")], &["panel"]);
    assert_eq!(code, 1);
    assert!(err.contains("could not find") && err.contains("no VS Code window"), "{err}");
}

// ------------------------------------------------------------------ the text reaches the driver unchanged

#[test]
fn a_slash_text_reaches_the_driver_unchanged_through_every_verb() {
    let fx = chat();
    fx.write("prompt.md", "/status please\nsecond line");
    let (code, out, err) = session(&fx, &[], &["start", "worker/y", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(calls(&fx)[0].1.as_deref(), Some("/status please\nsecond line"), "no space is added in front of the slash");
    let list = r#"[{"session_id":"sid-1","name":"/odd","status":"idle","active":false}]"#;
    let (code, _, _) = session(&fx, &[("STUB_PANEL_LIST", list)], &["rename", "system/x", "--to", "/odd"]);
    assert_eq!(code, 0);
    assert_eq!(sends(&fx)[0].1.as_deref(), Some("/rename /odd"));
}

#[test]
fn the_exit_6_message_tells_him_to_fix_the_settings_file_before_anything_else() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "6"), ("STUB_PANEL_STDERR", "MODEL KEY NOT RESTORED: was fable, is haiku")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5"]);
    assert_eq!(code, 1);
    assert!(err.contains("MODEL KEY MISMATCH") && err.contains("settings.json by hand") && err.contains("MODEL KEY NOT RESTORED"), "{err}");
}

// ------------------------------------------------------------------ start --group

/// A `state.vscdb` like VS Code's: table ItemTable, key `Anthropic.claude-code`, one JSON object whose
/// `sessionGroups:<root>` entries hold the groups.
fn state_db(fx: &Fx, root_key: &str, groups: &[(&str, &str)]) -> String {
    let path = fx.path("state.vscdb");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)", []).unwrap();
    let list: Vec<Value> = groups.iter().map(|(id, name)| json!({"id": id, "name": name, "collapsed": false, "sessionIds": ["x"]})).collect();
    let state = json!({
        format!("sessionGroups:{root_key}"): list,
        "sessionGroups:c:\\elsewhere": [{"id": "other-id", "name": "Elsewhere", "collapsed": false, "sessionIds": []}],
        "hiddenSessionIds": ["a", "b"],
    });
    conn.execute("INSERT INTO ItemTable (key, value) VALUES ('Anthropic.claude-code', ?1)", [state.to_string()]).unwrap();
    drop(conn);
    path.to_string_lossy().into_owned()
}

#[test]
fn start_group_is_resolved_to_its_id_from_the_stored_groups_and_sent_to_the_driver() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    // The extension keys groups by the vault path in its own spelling: lower-case drive, back slashes.
    let key = fx.root().replace('/', "\\").to_lowercase();
    let db = state_db(&fx, &key, &[("d00fc9eb-0001", "Main"), ("7880c530-0002", "AgentFrame Build")]);
    let (code, out, err) = session(&fx, &[("AF_VSCODE_STATE_DB", &db)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "AgentFrame Build"]);
    assert_eq!(code, 0, "{out}{err}");
    let (args, _) = &calls(&fx)[0];
    assert_eq!(arg_after(args, "-GroupId").as_deref(), Some("7880c530-0002"));
    assert_eq!(arg_after(args, "-Model").as_deref(), Some("Haiku 5.5"));
    // A different case is accepted when it is not ambiguous.
    let (code, _, err) = session(&fx, &[("AF_VSCODE_STATE_DB", &db)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "main"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(arg_after(&calls(&fx)[1].0, "-GroupId").as_deref(), Some("d00fc9eb-0001"));
    // No --group, no -GroupId.
    let (code, _, _) = session(&fx, &[("AF_VSCODE_STATE_DB", &db)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5"]);
    assert_eq!(code, 0);
    assert_eq!(arg_after(&calls(&fx)[2].0, "-GroupId"), None);
}

#[test]
fn an_unknown_group_is_refused_with_the_names_there_are_and_nothing_reaches_the_driver() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let key = fx.root();
    let db = state_db(&fx, &key, &[("id-1", "Main"), ("id-2", "AgentFrame Build")]);
    let (code, _, err) = session(&fx, &[("AF_VSCODE_STATE_DB", &db)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Nope"]);
    assert_eq!(code, 1);
    assert!(err.contains("no session group named 'Nope'") && err.contains("'Main'") && err.contains("'AgentFrame Build'"), "{err}");
    assert!(!err.contains("Elsewhere"), "only this vault's groups are listed: {err}");
    assert!(calls(&fx).is_empty());
    // A vault with no groups says none; a missing state file is a refusal too.
    let other = Fx::new();
    let empty = state_db(&other, "c:\\nowhere", &[]);
    let (code, _, err) = session(&fx, &[("AF_VSCODE_STATE_DB", &empty)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("The groups are: none"), "{err}");
    let (code, _, err) = session(&fx, &[("AF_VSCODE_STATE_DB", &fx.p("missing.vscdb"))], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot find VS Code's state file"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn the_state_file_is_only_read_and_a_worktree_root_finds_the_vaults_groups() {
    let fx = Fx::new();
    let db = state_db(&fx, &fx.root(), &[("id-1", "Main")]);
    let before = std::fs::read(&db).unwrap();
    let groups = afcore::panel::session_groups_at(std::path::Path::new(&db), &fx.root()).unwrap();
    assert_eq!(groups, vec![("id-1".to_string(), "Main".to_string())]);
    assert_eq!(std::fs::read(&db).unwrap(), before, "the original file is untouched");
    let wt = format!("{}/.claude/worktrees/one-machine", fx.root());
    assert_eq!(afcore::panel::session_groups_at(std::path::Path::new(&db), &wt).unwrap().len(), 1, "a worktree path resolves to the vault root's groups");
}

// ------------------------------------------------------------------ the real driver is addressed, never run

#[test]
fn without_a_named_driver_the_call_is_windows_powershell_on_the_script_and_a_missing_script_refuses() {
    let fx = chat();
    let mut c = fx.af();
    stub_env(&fx, &mut c);
    c.env_remove("AF_PANEL_DRIVER");
    let o = c.args(["session", "compact", "system/x", "--dry-run"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("the panel driver is missing"), "{}", String::from_utf8_lossy(&o.stderr));
    fx.write("system/tools/claude_panel.ps1", "");
    let mut c = fx.af();
    stub_env(&fx, &mut c);
    c.env_remove("AF_PANEL_DRIVER");
    let o = c.args(["session", "compact", "system/x", "--dry-run"]).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(0), "{out}{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains("powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File"), "{out}");
    assert!(out.contains("claude_panel.ps1 send -Session sid-1"), "{out}");
}

#[test]
fn every_test_fixture_points_the_driver_at_nothing_by_default() {
    let fx = chat();
    let o = fx.af().args(["session", "rename", "system/x"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(err.contains("would not start"), "{err}");
}

// ------------------------------------------------------------------ notes, the deadline, the lock

/// A process id that is certainly not running: a child that has already ended.
fn dead_pid() -> u32 {
    let mut c = std::process::Command::new(panel_stub()).arg("agents").stdout(std::process::Stdio::null()).spawn().unwrap();
    let id = c.id();
    c.wait().unwrap();
    id
}

#[test]
fn the_drivers_notes_are_printed_on_success_and_on_a_refusal() {
    let fx = chat();
    let note = "his view was not restored: the operator is at the machine";
    let (code, out, err) = session(&fx, &[("STUB_PANEL_NOTE", note)], &["compact", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("Driver notes: {note}")), "{out}");
    let list = r#"[{"session_id":"sid-1","name":"system/x","status":"idle","active":false}]"#;
    let (code, out, err) = session(&fx, &[("STUB_PANEL_NOTE", note), ("STUB_PANEL_LIST", list)], &["rename", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("note: {note}")), "{out}");
    let fx = chat();
    let reason = "read-back failed (the input does not hold the text). Nothing was sent. The driver closed the chat's tab, then opened it with the text in the input, then closed that tab again to take the unsent text away, then opened the chat's tab again with an empty input. The chat's tab is now open.";
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "5"), ("STUB_PANEL_STDERR", reason), ("STUB_PANEL_NOTE", note)], &["compact", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("exit 5") && err.contains("The chat's tab is now open.") && err.contains(note), "the whole driver message and its notes: {err}");
}

#[test]
fn afs_limit_is_the_drivers_own_deadline_plus_a_margin() {
    use afcore::panel::{driver_limit, DRIVER_MARGIN_SECS, DRIVER_WAIT_MINUTES, DRIVER_WORK_SECS};
    assert_eq!(DRIVER_WORK_SECS, 1200, "claude_panel.ps1 $WorkSeconds is the same number (a Python test pairs them)");
    assert_eq!(DRIVER_WAIT_MINUTES, 20.0);
    assert_eq!(driver_limit(Some(5.0)), Duration::from_secs(5 * 60 + DRIVER_WORK_SECS + DRIVER_MARGIN_SECS));
    assert_eq!(driver_limit(None), Duration::from_secs(20 * 60 + DRIVER_WORK_SECS + DRIVER_MARGIN_SECS));
    assert_eq!(driver_limit(Some(60.0)), Duration::from_secs(3600 + 1260));
    // The longest step after the wait (a /compact confirmation, 10 minutes) fits in the work time.
    assert!(DRIVER_WORK_SECS > 600 + 90 + 120);
}

#[test]
fn a_driver_that_runs_past_afs_limit_is_killed_logged_and_reported() {
    let fx = chat();
    let started = Instant::now();
    let (code, out, err) = session(&fx, &[("AF_PANEL_LIMIT_SECS", "1"), ("STUB_PANEL_SLEEP_SECS", "8")], &["compact", "system/x"]);
    assert_eq!(code, 1, "{out}");
    assert!(started.elapsed() < Duration::from_secs(7), "killed at the limit, not waited out");
    assert!(err.contains("af killed the panel driver") && err.contains("look at the chat now"), "{err}");
    assert!(!out.contains("compacted"), "{out}");
    let log = fx.read("system/logs/claude-panel.log");
    assert!(log.contains("send sid-1 exit=killed-by-af"), "{log}");
}

#[test]
fn the_vault_root_is_found_from_a_worktree() {
    use afcore::panel::vault_root;
    assert_eq!(vault_root("C:\\v\\Vault\\.claude\\worktrees\\one"), std::path::PathBuf::from("C:\\v\\Vault"));
    assert_eq!(vault_root("/v/Vault/.Claude/Worktrees/one"), std::path::PathBuf::from("/v/Vault"));
    assert_eq!(vault_root("C:\\v\\Vault"), std::path::PathBuf::from("C:\\v\\Vault"));
}

#[test]
fn a_live_driver_lock_on_the_chat_keeps_it_off_the_closed_route_and_a_dead_one_does_not() {
    let fx = chat();
    let me = std::process::id();
    fx.write("system/logs/panel-bridge/driver.lock", &json!({"pid": me, "session_id": "sid-1", "action": "send", "started": "now"}).to_string());
    let (code, out, err) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("the panel driver (pid {me}) holds the panel lock")) && out.contains("through the panel"), "{out}");
    fx.write("system/logs/panel-bridge/driver.lock", &json!({"pid": me, "session_id": "sid-other", "action": "send"}).to_string());
    let (_, out, _) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert!(out.contains("would run from"), "another chat's lock does not count: {out}");
    fx.write("system/logs/panel-bridge/driver.lock", &json!({"pid": dead_pid(), "session_id": "sid-1", "action": "send"}).to_string());
    let (_, out, _) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert!(out.contains("would run from"), "a lock left by a dead run is stale: {out}");
}

// ------------------------------------------------------------------ one waiter per chat, deciding when it fires

#[test]
fn a_second_when_idle_finds_the_pending_waiter_and_starts_none() {
    let fx = chat();
    let me = std::process::id();
    let job = "system/logs/claude-panel/waiter-sid-1.json";
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "queued": "2026-11-13T10:00:00+00:00", "queued_ms": 1}).to_string());
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("already pending") && out.contains(&format!("(pid {me})")) && out.contains("nothing new was started"), "{out}");
    std::thread::sleep(Duration::from_millis(500));
    assert!(calls(&fx).is_empty() && !fx.exists("system/logs/panel-waiters.log"));
    // A job whose waiter is gone is stale: a new waiter starts.
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": dead_pid(), "queued_ms": 1}).to_string());
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("detached waiter"), "{out}");
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("compact ok"), "{log}");
}

#[test]
fn the_waiter_does_nothing_when_the_chat_was_compacted_after_it_was_queued() {
    let fx = chat();
    let len = fx.read(TRANSCRIPT).len();
    fx.write(TRANSCRIPT, &format!("{}{{\"type\":\"system\",\"subtype\":\"compact_boundary\"}}\n", fx.read(TRANSCRIPT)));
    let job = fx.write(
        "system/logs/claude-panel/waiter-sid-1.json",
        &json!({"name": "system/x", "session_id": "sid-1", "instructions": null, "force": false, "then_file": null, "wait_minutes": 60, "queued_ms": 1, "transcript_len": len}).to_string(),
    );
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--waiter-job", &job]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(calls(&fx).is_empty(), "nothing was sent");
    assert!(fx.read("system/logs/panel-waiters.log").contains("nothing to do; its transcript gained a compact_boundary line after this waiter was queued"));
    assert!(!fx.exists("system/logs/claude-panel/waiter-sid-1.json"));
}

#[test]
fn the_waiter_waits_while_a_record_is_not_idle_and_fires_when_it_is() {
    let fx = chat();
    let me = std::process::id();
    // No status is not idle.
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode"}).to_string());
    let (code, out, err) = session(&fx, &[("AF_WAITER_POLL_MS", "100")], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(sends(&fx).is_empty(), "nothing is sent while the chat is not idle");
    assert!(fx.exists("system/logs/claude-panel/waiter-sid-1.json"), "the waiter holds its job while it waits");
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode", "status": "busy"}).to_string());
    std::thread::sleep(Duration::from_millis(600));
    assert!(sends(&fx).is_empty());
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode", "status": "idle"}).to_string());
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("system/x compact ok: system/x: compacted through the panel"), "{log}");
    assert_eq!(sends(&fx).len(), 1);
}

#[test]
fn a_failed_waiter_leaves_one_line_that_the_chat_sees_once() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_EXIT", "4"), ("STUB_PANEL_STDERR", "the tab was not closed: the chat has a draft typed in its input box. Nothing was sent. The panel was not changed")], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    wait_for(&fx, "system/logs/panel-waiters.log", "compact failed");
    let result = "system/logs/context-nudge/sid-1/waiter-result.txt";
    let until = Instant::now() + Duration::from_secs(10);
    while !fx.exists(result) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(fx.read(result).contains("The --when-idle compaction this chat queued failed") && fx.read(result).contains("exit 4"));
    let payload = json!({"session_id": "sid-1", "tool_name": "Read", "hook_event_name": "PostToolUse"}).to_string();
    let o = fx.af().args(["hook", "post-tool-use"]).write_stdin(payload.clone()).output().unwrap();
    let shown = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(shown.contains("The --when-idle compaction this chat queued failed"), "{shown}");
    assert!(!fx.exists(result), "shown once");
    let o = fx.af().args(["hook", "post-tool-use"]).write_stdin(payload).output().unwrap();
    assert!(!String::from_utf8_lossy(&o.stdout).contains("--when-idle compaction"));
    // At a stop it keeps the chat going once to read it; a stop that is already a continuation is left alone.
    fx.write(result, "2026-11-13T10:00:00+00:00 The --when-idle compaction this chat queued failed: x\n");
    let stop = |active: bool| {
        let p = json!({"session_id": "sid-1", "hook_event_name": "Stop", "stop_hook_active": active}).to_string();
        String::from_utf8_lossy(&fx.af().args(["hook", "stop"]).write_stdin(p).output().unwrap().stdout).into_owned()
    };
    assert!(!stop(true).contains("--when-idle compaction"));
    let out = stop(false);
    assert!(out.contains("\"decision\":\"block\"") && out.contains("The --when-idle compaction this chat queued failed: x"), "{out}");
    assert!(!fx.exists(result));
}

#[test]
fn a_successful_waiter_leaves_nothing_for_the_chat() {
    let fx = chat();
    assert_eq!(session(&fx, &[], &["compact", "system/x", "--when-idle"]).0, 0);
    wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(!fx.exists("system/logs/context-nudge/sid-1/waiter-result.txt"));
}
