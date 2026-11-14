//! The session verbs that use the VS Code panel driver: compacting a running chat, `--when-idle`,
//! `--then-file`, `rename`, `start` (with `--group`), `panel` and `group`. The driver is a stub compiled for the test and named
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
    assert_eq!(calls(&fx)[0].0, ["list", "-WaitMinutes", "0.5"], "a list waits half a minute at most");
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

// ------------------------------------------------------------------ group

/// The groups the stub's `groups` action prints: his Main and Q3 2026, and the panel's own Ungrouped.
const GROUPS: &str = r#"[{"name":"Main","count":2,"fixed":false},{"name":"Q3 2026","count":1,"fixed":false},{"name":"Ungrouped","count":3,"fixed":true},{"name":"Archived sessions","count":4,"fixed":true}]"#;

fn group_calls(fx: &Fx) -> Vec<Vec<String>> {
    calls(fx).into_iter().map(|(a, _)| a).filter(|a| a[0].starts_with("group")).collect()
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

#[test]
fn group_list_prints_the_groups_and_their_counts_and_asks_the_driver_to_read_only() {
    let fx = Fx::new();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_GROUPS", GROUPS)], &["group", "list"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("| Main | 2 | his group |") && out.contains("| Q3 2026 | 1 | his group |") && out.contains("| Ungrouped | 3 | the panel's own |"), "{out}");
    let (code, out, _) = session(&fx, &[("STUB_PANEL_GROUPS", GROUPS)], &["group", "list", "--json"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!((v[0]["name"].as_str(), v[0]["count"].as_i64(), v[0]["fixed"].as_bool()), (Some("Main"), Some(2), Some(false)));
    assert_eq!(v[2]["fixed"], true);
    assert_eq!(calls(&fx)[0].0, ["groups", "-WaitMinutes", "0.5"], "a read waits half a minute at most, like a list");
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "2"), ("STUB_PANEL_STDERR", "no VS Code window")], &["group", "list"]);
    assert_eq!(code, 1);
    assert!(err.contains("could not find") && err.contains("no VS Code window"), "{err}");
}

#[test]
fn group_create_rename_and_delete_build_the_drivers_calls() {
    let fx = Fx::new();
    let (code, out, err) = session(&fx, &[("STUB_PANEL_OUT", "created the group 'Work'; the list shows it"), ("STUB_PANEL_NOTE", "pointer put back")], &["group", "create", "Work"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("created the group 'Work'") && out.contains("Driver notes: pointer put back"), "{out}");
    assert_eq!(session(&fx, &[], &["group", "create", "  Q3 2026  ", "--wait-minutes", "3"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "rename", "Old name", "New name"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "delete", "Work"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "delete", "Main", "--force", "--wait-minutes", "1"]).0, 0);
    assert_eq!(
        group_calls(&fx),
        vec![
            argv(&["group-create", "-Group", "Work"]),
            argv(&["group-create", "-Group", "Q3 2026", "-WaitMinutes", "3"]),
            argv(&["group-rename", "-Group", "Old name", "-NewName", "New name"]),
            argv(&["group-delete", "-Group", "Work"]),
            argv(&["group-delete", "-Group", "Main", "-Force", "-WaitMinutes", "1"]),
        ]
    );
    assert!(calls(&fx).iter().all(|(a, t)| t.is_none() && !a.contains(&"-TextFile".to_string())), "a group's name is an argument, never a file");
}

#[test]
fn group_add_and_remove_take_a_registered_name_or_a_session_id_and_may_target_the_calling_chat() {
    let fx = chat();
    let id = "0a1b2c3d-1111-2222-3333-444455556666";
    assert_eq!(session(&fx, &[], &["group", "add", "system/x", "Main"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "add", "--session-id", id, "Q3 2026"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "add", "system/x", "Main", "--session-id", "sid-1"]).0, 0, "the name and its own id may both be given");
    assert_eq!(session(&fx, &[], &["group", "remove", "system/x"]).0, 0);
    assert_eq!(session(&fx, &[], &["group", "remove", "--session-id", id, "--wait-minutes", "2"]).0, 0);
    // Nothing here needs the chat idle, so the calling chat can be grouped too.
    assert_eq!(session(&fx, &[("CLAUDE_CODE_SESSION_ID", "sid-1")], &["group", "add", "system/x", "Main"]).0, 0);
    assert_eq!(session(&fx, &[("CLAUDE_CODE_SESSION_ID", "sid-1")], &["group", "remove", "system/x"]).0, 0);
    assert_eq!(
        group_calls(&fx),
        vec![
            argv(&["group-add", "-Session", "sid-1", "-Group", "Main"]),
            argv(&["group-add", "-Session", id, "-Group", "Q3 2026"]),
            argv(&["group-add", "-Session", "sid-1", "-Group", "Main"]),
            argv(&["group-remove", "-Session", "sid-1"]),
            argv(&["group-remove", "-Session", id, "-WaitMinutes", "2"]),
            argv(&["group-add", "-Session", "sid-1", "-Group", "Main"]),
            argv(&["group-remove", "-Session", "sid-1"]),
        ]
    );
}

#[test]
fn group_verbs_refuse_bad_arguments_before_the_driver_runs() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[], &["group", "add", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("needs a chat name, or --session-id"), "{err}");
    let (code, _, err) = session(&fx, &[], &["group", "add", "system/nope", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("no registry row for system/nope"), "{err}");
    let (code, _, err) = session(&fx, &[], &["group", "add", "--session-id", "not an id", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("8 to 64 hex digits and dashes"), "{err}");
    let (code, _, err) = session(&fx, &[], &["group", "add", "system/x", "Main", "--session-id", "0a1b2c3d-1111-2222-3333-444455556666"]);
    assert_eq!(code, 1);
    assert!(err.contains("not the session id of that registry row"), "{err}");
    let (code, _, err) = session(&fx, &[], &["group", "remove"]);
    assert_eq!(code, 1);
    assert!(err.contains("needs a chat name, or --session-id"), "{err}");
    let (code, _, err) = session(&fx, &[], &["group", "create", "   "]);
    assert_eq!(code, 1);
    assert!(err.contains("the group name is empty"), "{err}");
    assert_eq!(session(&fx, &[], &["group", "add", "a", "b", "c"]).0, 2, "three words are a usage error");
    assert!(calls(&fx).is_empty(), "nothing reached the driver");
}

#[test]
fn group_dry_run_prints_the_call_and_what_will_show_on_his_screen_and_runs_nothing() {
    let fx = chat();
    let id = "0a1b2c3d-1111-2222-3333-444455556666";
    let cases: Vec<(Vec<&str>, &str, &str)> = vec![
        (vec!["group", "create", "Work", "--dry-run"], "group-create -Group Work", "the New group button is pressed"),
        (vec!["group", "rename", "Old", "New", "--dry-run", "--wait-minutes", "2"], "group-rename -Group Old -NewName New -WaitMinutes 2", "Rename group is pressed"),
        (vec!["group", "delete", "Main", "--force", "--dry-run"], "group-delete -Group Main -Force", "move to Ungrouped"),
        (vec!["group", "add", "--session-id", id, "Q3 2026", "--dry-run"], "group-add -Session 0a1b2c3d-1111-2222-3333-444455556666 -Group \"Q3 2026\"", "does not have to be idle"),
        (vec!["group", "remove", "system/x", "--dry-run"], "group-remove -Session sid-1", "Remove from group is pressed"),
    ];
    for (args, call, note) in cases {
        let (code, out, err) = session(&fx, &[], &args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        assert!(out.contains(call), "{args:?}: {out}");
        assert!(out.contains("VS Code comes to the front") && out.contains("idle for 120 seconds") && out.contains("Nothing was run."), "{args:?}: {out}");
        assert!(out.contains(note), "{args:?}: {out}");
        assert!(out.contains("put back") || out.contains("goes back"), "{args:?}: the pointer and the foreground are put back: {out}");
    }
    assert!(calls(&fx).is_empty(), "a dry run starts no driver");
}

#[test]
fn a_group_verb_exits_with_the_drivers_code_and_says_what_it_refused() {
    let cases = [
        ("5", 5, "refused (exit 5)", "nothing was changed"),
        ("3", 3, "stopped (exit 3)", "operator was using the computer"),
        ("2", 2, "could not find", "no row for this chat"),
        ("1", 1, "failed (exit 1)", "the stub driver refused"),
        ("7", 7, "cancelled (exit 7)", "cancelled"),
        ("6", 6, "MODEL KEY MISMATCH", "MODEL"),
    ];
    for (driver_code, af_code, label, extra) in cases {
        let fx = chat();
        let stderr = match driver_code {
            "5" => "a group named 'Main' already exists; nothing was changed",
            "2" => "no row for this chat is in the list as shown",
            _ => "the stub driver refused",
        };
        let (exit, out, err) = session(&fx, &[("STUB_PANEL_EXIT", driver_code), ("STUB_PANEL_STDERR", stderr), ("STUB_PANEL_NOTE", "a note")], &["group", "add", "system/x", "Main"]);
        assert_eq!(exit, af_code, "{driver_code}: {out}{err}");
        assert!(err.contains(label) && err.contains(extra), "{driver_code}: {err}");
        assert!(err.contains("a note"), "{driver_code}: the driver's notes are printed too: {err}");
        assert!(!err.contains("nothing was sent"), "{driver_code}: {err}");
    }
    // Nothing to do is a success.
    let fx = chat();
    let (code, out, _) = session(&fx, &[("STUB_PANEL_OUT", "the chat is already in the group 'Main'; nothing to do")], &["group", "add", "system/x", "Main"]);
    assert_eq!(code, 0);
    assert!(out.contains("nothing to do"), "{out}");
}

#[test]
fn a_group_action_has_a_short_limit_of_its_own_and_a_groups_read_the_list_limit() {
    use afcore::panel::{action_limit, DRIVER_GROUP_WORK_SECS, DRIVER_MARGIN_SECS, READ_LIMIT_SECS};
    assert_eq!(DRIVER_GROUP_WORK_SECS, 60, "claude_panel.ps1 $GroupWorkSeconds is the same number (a Python test pairs them)");
    assert_eq!(action_limit("group-add", Some(1.0)), Duration::from_secs(60 + DRIVER_GROUP_WORK_SECS + DRIVER_MARGIN_SECS));
    assert_eq!(action_limit("group-create", None), Duration::from_secs(20 * 60 + DRIVER_GROUP_WORK_SECS + DRIVER_MARGIN_SECS));
    assert_eq!(action_limit("groups", Some(0.5)), Duration::from_secs(READ_LIMIT_SECS));
    assert_eq!(action_limit("list", Some(0.5)), Duration::from_secs(READ_LIMIT_SECS));
}

// ------------------------------------------------------------------ start --group

#[test]
fn start_group_reads_the_groups_then_starts_the_chat_then_places_it_with_group_add_in_that_order() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let new_id = "abcdef12-3456-7890-abcd-ef1234567890";
    let (code, out, err) = session(
        &fx,
        &[("STUB_PANEL_GROUPS", GROUPS), ("STUB_PANEL_NEW_ID", new_id)],
        &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "q3 2026"],
    );
    assert_eq!(code, 0, "{out}{err}");
    let made = calls(&fx);
    let actions: Vec<&str> = made.iter().map(|(a, _)| a[0].as_str()).collect();
    assert_eq!(actions, ["groups", "new", "group-add"], "groups read first, then the new chat, then the group");
    let (new_args, _) = &made[1];
    assert_eq!(arg_after(new_args, "-Group"), None, "no group is passed with the new chat (that route leaves it in Ungrouped)");
    assert_eq!(arg_after(new_args, "-GroupId"), None);
    assert_eq!(arg_after(new_args, "-Model").as_deref(), Some("Haiku 5.5"));
    let (add_args, _) = &made[2];
    assert_eq!(arg_after(add_args, "-Session").as_deref(), Some(new_id));
    assert_eq!(arg_after(add_args, "-Group").as_deref(), Some("Q3 2026"), "the group's own spelling, found without regard to case");
    assert!(out.contains(new_id) && out.contains("af session register system/x --session abcdef12") && out.contains("placed in the group 'Q3 2026'"), "{out}");
    // No --group: one call, no group read.
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let (code, _, _) = session(&fx, &[("STUB_PANEL_GROUPS", GROUPS)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5"]);
    assert_eq!(code, 0);
    assert_eq!(calls(&fx).len(), 1);
}

#[test]
fn an_unknown_group_is_refused_with_the_names_there_are_before_any_chat_is_started() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let (code, _, err) = session(
        &fx,
        &[("STUB_PANEL_GROUPS", GROUPS)],
        &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Nope"],
    );
    assert_eq!(code, 1);
    assert!(err.contains("no session group named 'Nope'") && err.contains("'Main'") && err.contains("'Q3 2026'"), "{err}");
    assert!(!err.contains("Ungrouped") && !err.contains("Archived"), "only his groups are listed: {err}");
    let actions: Vec<String> = calls(&fx).into_iter().map(|(a, _)| a[0].clone()).collect();
    assert_eq!(actions, ["groups"], "the groups were read and no chat was started for nothing");
    // The panel's own Ungrouped is not a group to start into; a vault with no groups says none.
    let (code, _, err) = session(&fx, &[("STUB_PANEL_GROUPS", GROUPS)], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Ungrouped"]);
    assert_eq!(code, 1);
    assert!(err.contains("no session group named 'Ungrouped'"), "{err}");
    let (code, _, err) = session(&fx, &[], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("The groups are: none"), "{err}");
    assert!(calls(&fx).iter().all(|(a, _)| a[0] == "groups"), "no chat was started in any case");
    // A driver that cannot read the groups stops the start too.
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "2"), ("STUB_PANEL_STDERR", "no VS Code window")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("no VS Code window"), "{err}");
    assert!(calls(&fx).iter().all(|(a, _)| a[0] == "groups"));
}

#[test]
fn when_the_group_add_fails_the_chat_is_reported_as_started_with_its_id_and_the_retry_command_and_af_exits_with_the_adds_code() {
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let new_id = "abcdef12-3456-7890-abcd-ef1234567890";
    let (code, out, err) = session(
        &fx,
        &[("STUB_PANEL_GROUPS", GROUPS), ("STUB_PANEL_NEW_ID", new_id), ("STUB_PANEL_EXITS", "0,0,5"), ("STUB_PANEL_STDERR", "the Add to group submenu has no item named 'Q3 2026'; nothing was changed")],
        &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Q3 2026"],
    );
    assert_eq!(code, 5, "af exits with the add's code: {out}{err}");
    assert!(out.contains(new_id) && out.contains("started in the panel as session"), "the started line is still printed: {out}");
    assert!(err.contains("the chat WAS started") && err.contains(new_id) && err.contains("NOT placed in the group 'Q3 2026'"), "{err}");
    assert!(err.contains("no item named") && err.contains("refused (exit 5)"), "why: {err}");
    assert!(err.contains(&format!("af session group add --session-id {new_id} \"Q3 2026\"")), "the command to retry: {err}");
    let actions: Vec<String> = calls(&fx).into_iter().map(|(a, _)| a[0].clone()).collect();
    assert_eq!(actions, ["groups", "new", "group-add"]);
    // A failed start is as before: no group-add, and the add's code is not involved.
    let fx = Fx::new();
    fx.write("prompt.md", "hello");
    let (code, _, err) = session(&fx, &[("STUB_PANEL_GROUPS", GROUPS), ("STUB_PANEL_EXITS", "0,3")], &["start", "system/x", "--brief-prompt-file", &fx.p("prompt.md"), "--model", "Haiku 5.5", "--group", "Main"]);
    assert_eq!(code, 1);
    assert!(err.contains("exit 3") && !err.contains("WAS started"), "{err}");
    let actions: Vec<String> = calls(&fx).into_iter().map(|(a, _)| a[0].clone()).collect();
    assert_eq!(actions, ["groups", "new"]);
}

#[test]
fn panel_json_carries_the_group_of_every_chat() {
    let fx = chat();
    let list = r#"[{"session_id":"sid-1","name":"system/x","status":"idle","active":true,"group":"Main"},{"session_id":"sid-9","name":"stray","status":"idle","active":false}]"#;
    let (code, out, _) = session(&fx, &[("STUB_PANEL_LIST", list)], &["panel", "--json"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!((v[0]["group"].as_str(), v[1]["group"].as_str()), (Some("Main"), Some("")));
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
    // A live pid that started at another time than the one recorded is another process (a reused pid).
    let start = afcore::panel::pid_start(me).unwrap();
    fx.write("system/logs/panel-bridge/driver.lock", &json!({"pid": me, "pid_start": start + 1, "session_id": "sid-1", "action": "send"}).to_string());
    let (_, out, _) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert!(out.contains("would run from"), "a reused pid does not hold the lock: {out}");
    fx.write("system/logs/panel-bridge/driver.lock", &json!({"pid": me, "pid_start": start.to_string(), "session_id": "sid-1", "action": "send"}).to_string());
    let (_, out, _) = session(&fx, &[("STUB_ROSTER", "[]")], &["compact", "system/x", "--dry-run"]);
    assert!(out.contains("holds the panel lock"), "the recorded process itself holds it: {out}");
}

// ------------------------------------------------------------------ one waiter per chat, deciding when it fires

#[test]
fn a_second_when_idle_finds_the_pending_waiter_and_starts_none() {
    let fx = chat();
    let me = std::process::id();
    let job = "system/logs/claude-panel/waiter-sid-1.json";
    let start = afcore::panel::pid_start(me).unwrap();
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "pid_start": start, "queued": "2026-11-13T10:00:00+00:00", "queued_ms": now_ms(), "wait_minutes": 60}).to_string());
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
    let (code, out, err) = session(&fx, &[("CLAUDE_CODE_SESSION_ID", "sid-1"), ("STUB_PANEL_EXIT", "4"), ("STUB_PANEL_STDERR", "the tab was not closed: the chat has a draft typed in its input box. Nothing was sent. The panel was not changed")], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    wait_for(&fx, "system/logs/panel-waiters.log", "compact failed");
    let result = "system/logs/context-nudge/sid-1/waiter-result.txt";
    let until = Instant::now() + Duration::from_secs(10);
    while !fx.exists(result) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(fx.read(result).contains("The --when-idle compaction this chat queued for itself failed") && fx.read(result).contains("exit 4"));
    let payload = json!({"session_id": "sid-1", "tool_name": "Read", "hook_event_name": "PostToolUse"}).to_string();
    let o = fx.af().args(["hook", "post-tool-use"]).write_stdin(payload.clone()).output().unwrap();
    let shown = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(shown.contains("The --when-idle compaction this chat queued for itself failed"), "{shown}");
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

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

// ------------------------------------------------------------------ second review: the lock, the job, the limits

#[test]
fn a_list_has_a_short_limit_of_its_own_and_a_send_keeps_the_long_one() {
    use afcore::panel::{action_limit, driver_limit, DRIVER_LIST_WORK_SECS, LIST_WAIT_MINUTES, READ_LIMIT_SECS};
    assert_eq!(READ_LIMIT_SECS, 90);
    assert_eq!(action_limit("list", Some(LIST_WAIT_MINUTES)), Duration::from_secs(90));
    assert_eq!(action_limit("list", Some(60.0)), Duration::from_secs(90), "whatever wait a caller passes");
    assert_eq!(action_limit("send", Some(5.0)), driver_limit(Some(5.0)));
    // The driver's own list deadline (its wait and $ListWorkSeconds, which a Python test pairs) ends inside it.
    assert!((LIST_WAIT_MINUTES * 60.0) as u64 + DRIVER_LIST_WORK_SECS + 10 <= READ_LIMIT_SECS);
}

#[test]
fn a_waiter_job_is_left_over_once_its_pid_started_at_another_time_or_its_time_is_past() {
    let fx = chat();
    let me = std::process::id();
    let start = afcore::panel::pid_start(me).unwrap();
    let job = "system/logs/claude-panel/waiter-sid-1.json";
    // A live pid with another start time: after a reboot the pid belongs to another process.
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "pid_start": start + 7, "queued_ms": now_ms(), "wait_minutes": 60}).to_string());
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("detached waiter"), "a reused pid is not a pending waiter: {out}");
    wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    // The right process, but queued longer ago than its wait and all a waiter may run after it.
    let fx = chat();
    let old = now_ms() - 60 * 60_000 - afcore::compact::WAITER_RUN_SECS * 1000 - 1;
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "pid_start": start, "queued_ms": old, "wait_minutes": 60}).to_string());
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("detached waiter"), "an expired job is not pending: {out}");
    wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
}

#[test]
fn a_waiter_job_still_inside_its_run_time_is_pending_however_long_ago_it_was_queued() {
    let fx = chat();
    let me = std::process::id();
    let start = afcore::panel::pid_start(me).unwrap();
    let job = "system/logs/claude-panel/waiter-sid-1.json";
    // Queued 60 minutes ago with a 60-minute wait, fired at its last minute: it may run on for
    // WAITER_RUN_SECS, so a minute before that end it is still the chat's waiter.
    let queued = now_ms() - 60 * 60_000 - (afcore::compact::WAITER_RUN_SECS - 60) * 1000;
    fx.write(job, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "pid_start": start, "queued_ms": queued, "wait_minutes": 60}).to_string());
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("detached waiter") && out.contains("already pending"), "still pending: {out}");
}

/// The worst case of a waiter's run, stage by stage, as fire_waiter and through_panel cut it: each driver
/// run is given `stage_wait_minutes` and may use all of it plus the driver's work time and af's margin;
/// the boundary wait is cut to the end. Whenever it fires inside its wait, it ends by `waiter_end_ms`,
/// the same end the job's expiry uses.
#[test]
fn a_waiter_cut_to_its_window_always_ends_by_the_end_its_job_expires_at() {
    use afcore::compact::{stage_wait_minutes, waiter_end_ms, BOUNDARY_WAIT_SECS, STAGE_MIN_WAIT_SECS, WAITER_RUN_SECS};
    use afcore::panel::{DRIVER_MARGIN_SECS, DRIVER_WORK_SECS};
    let run = (DRIVER_WORK_SECS + DRIVER_MARGIN_SECS) * 1000;
    for wait_minutes in [0.0_f64, 1.0, 5.0, 60.0, 240.0] {
        let queued = 1_000_000_000u64;
        let until = queued + (wait_minutes * 60_000.0) as u64;
        let end = waiter_end_ms(queued, wait_minutes);
        assert_eq!(end, until + WAITER_RUN_SECS * 1000);
        let mut fire = queued;
        while fire <= until {
            // The compaction: the whole stage wait used, the boundary wait cut to the end.
            let w1 = stage_wait_minutes(fire, until, end).expect("a waiter firing inside its wait always gets its compaction run");
            let mut t = fire + (w1 * 60_000.0) as u64 + run;
            assert!(t <= end, "fire {fire}: the compaction run ends by the waiter's end");
            t = (t + BOUNDARY_WAIT_SECS * 1000).min(end);
            // The --then-file message: run only when it fits.
            if let Some(w2) = stage_wait_minutes(t, until, end) {
                assert!(w2 * 60_000.0 >= (STAGE_MIN_WAIT_SECS * 1000) as f64);
                t += (w2 * 60_000.0) as u64 + run;
            }
            assert!(t <= end, "fire {fire}, wait {wait_minutes}: ends {t} after {end}");
            fire += 60_000;
        }
        // Fired at the last moment, both runs still fit: the margin is not cut short.
        let w1 = stage_wait_minutes(until, until, end).unwrap();
        let t = until + (w1 * 60_000.0) as u64 + run + BOUNDARY_WAIT_SECS * 1000;
        assert!(stage_wait_minutes(t, until, end).is_some(), "a waiter fired at the end of its wait still sends its next message");
    }
    // The reviewer's case: queued for 60 minutes, fired at minute 1, its first run given the 59 left.
    let queued = 0u64;
    let until = 60 * 60_000;
    let end = waiter_end_ms(queued, 60.0);
    let w1 = stage_wait_minutes(60_000, until, end).unwrap();
    assert!((w1 - 59.0).abs() < 0.02, "{w1}");
    assert!(stage_wait_minutes(end - run - 59_000, until, end).is_none(), "no run starts that could not end in time");
}

#[test]
fn two_when_idle_calls_at_the_same_moment_start_one_waiter() {
    let fx = chat();
    let me = std::process::id();
    // The chat is busy, so the first waiter holds its job while the second call looks.
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode", "status": "busy"}).to_string());
    let spawn = || {
        let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin("af"));
        c.env("AF_ROOT", fx.root()).env_remove("CLAUDE_CODE_SESSION_ID").current_dir(fx.dir.path());
        c.env("CLAUDE_CONFIG_DIR", fx.path("cfg"))
            .env("AF_CLAUDE_BIN", panel_stub())
            .env("AF_PANEL_DRIVER", panel_stub())
            .env("STUB_PANEL_CALLS", fx.path("calls.txt"))
            .env("STUB_PANEL_TRANSCRIPT", fx.path(TRANSCRIPT))
            .env("STUB_ROSTER", RUNNING)
            .env("AF_COMPACT_BOUNDARY_SECS", "2")
            .env("AF_PANEL_READBACK_MS", "10");
        c.args(["session", "compact", "system/x", "--when-idle"]).env("AF_WAITER_POLL_MS", "100").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        c.spawn().unwrap()
    };
    let (a, b) = (spawn(), spawn());
    let outs: Vec<String> = [a, b].into_iter().map(|c| String::from_utf8_lossy(&c.wait_with_output().unwrap().stdout).into_owned()).collect();
    let started = outs.iter().filter(|o| o.contains("detached waiter")).count();
    let pending = outs.iter().filter(|o| o.contains("already pending")).count();
    assert_eq!((started, pending), (1, 1), "{outs:?}");
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode", "status": "idle"}).to_string());
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(log.matches("compact ok").count(), 1, "{log}");
    assert_eq!(sends(&fx).len(), 1);
}

#[test]
fn the_failure_line_goes_to_the_chat_that_queued_it_and_says_who_queued_what() {
    let failed = [("STUB_PANEL_EXIT", "4"), ("STUB_PANEL_STDERR", "a draft")];
    // Queued by another chat: the line goes to that chat.
    let fx = chat();
    let mut env = failed.to_vec();
    env.push(("CLAUDE_CODE_SESSION_ID", "sid-main"));
    assert_eq!(session(&fx, &env, &["compact", "system/x", "--when-idle"]).0, 0);
    wait_for(&fx, "system/logs/panel-waiters.log", "compact failed");
    let other = "system/logs/context-nudge/sid-main/waiter-result.txt";
    let until = Instant::now() + Duration::from_secs(10);
    while !fx.exists(other) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(fx.read(other).contains("The --when-idle compaction you queued for system/x failed"), "{}", fx.read(other));
    assert!(!fx.exists("system/logs/context-nudge/sid-1/waiter-result.txt"), "not to the compacted chat");
    // No caller known: the compacted chat is told, in words that do not claim it queued it.
    let fx = chat();
    assert_eq!(session(&fx, &failed, &["compact", "system/x", "--when-idle"]).0, 0);
    wait_for(&fx, "system/logs/panel-waiters.log", "compact failed");
    let own = "system/logs/context-nudge/sid-1/waiter-result.txt";
    let until = Instant::now() + Duration::from_secs(10);
    while !fx.exists(own) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    let line = fx.read(own);
    assert!(line.contains("A --when-idle compaction of this chat failed") && line.contains("could not identify") && !line.contains("this chat queued"), "{line}");
}

/// A closed chat with a transcript `claude -p` can compact.
fn closed_env(fx: &Fx) -> Vec<(String, String)> {
    vec![
        ("STUB_ROSTER".into(), "[]".into()),
        ("STUB_LOCK_SNAPSHOT".into(), fx.p("lock-seen.txt")),
        ("STUB_LOCK_FILE".into(), fx.p("system/logs/panel-bridge/driver.lock")),
    ]
}

#[test]
fn the_closed_route_holds_the_panel_lock_for_its_whole_run_and_lets_go_after() {
    let fx = chat();
    let env = closed_env(&fx);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    // The stub is not a real claude, so the run itself fails; what matters is the lock while it ran.
    let _ = session(&fx, &env, &["compact", "system/x"]);
    let seen = fx.read("lock-seen.txt");
    let held: Vec<&str> = seen.lines().filter(|l| l.contains("headless-compact")).collect();
    assert!(!held.is_empty(), "claude -p ran while the lock was held: {seen}");
    let v: Value = serde_json::from_str(held[0]).unwrap();
    assert_eq!(v["session_id"], "sid-1");
    assert!(v["pid"].as_u64().is_some() && v["pid_start"].as_u64().is_some(), "{v}");
    assert!(!fx.exists("system/logs/panel-bridge/driver.lock"), "let go after the run");
}

#[test]
fn the_closed_route_refuses_while_a_driver_run_holds_the_lock_for_another_chat() {
    let fx = chat();
    let lock = fx.path("system/logs/panel-bridge/driver.lock");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let me = std::process::id();
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(1);
    }
    let mut held = opts.open(&lock).unwrap();
    use std::io::Write;
    held.write_all(json!({"pid": me, "pid_start": afcore::panel::pid_start(me), "session_id": "sid-other", "action": "send"}).to_string().as_bytes()).unwrap();
    held.flush().unwrap();
    let mut env = closed_env(&fx);
    env.push(("AF_LOCK_WAIT_SECS".into(), "1".into()));
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let began = std::time::Instant::now();
    let (code, out, err) = session(&fx, &env, &["compact", "system/x"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(began.elapsed() >= std::time::Duration::from_secs(1), "it waited for the lock first");
    assert!(err.contains("nothing was compacted") && err.contains("held by a 'send' run for session sid-other") && err.contains("whole wait of 1 seconds"), "{err}");
    assert!(!fx.exists("lock-seen.txt") || !fx.read("lock-seen.txt").lines().any(|l| l.contains("headless")), "claude -p never ran");
    drop(held);
}

/// Hold the panel lock as another process would, with this holder's action and session.
fn hold_lock(fx: &Fx, action: &str, sid: &str) -> std::fs::File {
    let lock = fx.path("system/logs/panel-bridge/driver.lock");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let me = std::process::id();
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(1);
    }
    let mut held = opts.open(&lock).unwrap();
    use std::io::Write;
    held.write_all(json!({"pid": me, "pid_start": afcore::panel::pid_start(me), "session_id": sid, "action": action}).to_string().as_bytes()).unwrap();
    held.flush().unwrap();
    held
}

#[test]
fn the_closed_route_refusal_names_another_headless_compaction_as_the_holder() {
    let fx = chat();
    let held = hold_lock(&fx, "headless-compact", "sid-other");
    let mut env = closed_env(&fx);
    env.push(("AF_LOCK_WAIT_SECS".into(), "1".into()));
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (code, out, err) = session(&fx, &env, &["compact", "system/x"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("held by a 'headless-compact' run for session sid-other") && !err.contains("driver run"), "{err}");
    drop(held);
}

#[test]
fn the_closed_route_waits_for_the_lock_and_runs_once_it_is_let_go() {
    let fx = chat();
    let held = hold_lock(&fx, "send", "sid-other");
    let lock = fx.path("system/logs/panel-bridge/driver.lock");
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        drop(held);
        let _ = std::fs::remove_file(lock);
    });
    let mut env = closed_env(&fx);
    env.push(("AF_LOCK_WAIT_SECS".into(), "30".into()));
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (_code, out, err) = session(&fx, &env, &["compact", "system/x"]);
    release.join().unwrap();
    assert!(!err.contains("held by"), "{out}{err}");
    let seen = fx.read("lock-seen.txt");
    assert!(seen.lines().any(|l| l.contains("headless-compact")), "claude -p ran under the lock once it was free: {seen}");
}

// ------------------------------------------------------------------ archive

fn archives(fx: &Fx) -> Vec<Vec<String>> {
    calls(fx).into_iter().map(|(a, _)| a).filter(|a| a[0] == "archive").collect()
}

#[test]
fn archive_asks_the_driver_for_the_chats_session_and_leaves_the_registry_row_alone() {
    let fx = chat();
    let before = fx.read("system/sessions/registry.md");
    let (code, out, err) = session(&fx, &[("STUB_PANEL_NOTE", "pointer put back")], &["archive", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    let made = archives(&fx);
    assert_eq!(made.len(), 1, "{made:?}");
    assert_eq!(arg_after(&made[0], "-Session").as_deref(), Some("sid-1"));
    assert_eq!(arg_after(&made[0], "-WaitMinutes"), None, "the driver's own default wait applies");
    assert_eq!(arg_after(&made[0], "-CancelFile"), None, "only a waiter passes a cancel file");
    assert!(out.contains("system/x: ok") && out.contains("Driver notes: pointer put back"), "{out}");
    assert!(out.contains("registry row for system/x is unchanged") && out.contains("af session end system/x --reason other"), "{out}");
    assert_eq!(fx.read("system/sessions/registry.md"), before, "the registry row is not changed");
    assert_eq!(calls(&fx).len(), 1, "no send, no claude -p");
}

#[test]
fn archive_passes_the_wait_and_dry_run_prints_the_call_and_runs_nothing() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[], &["archive", "system/x", "--wait-minutes", "3"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(arg_after(&archives(&fx)[0], "-WaitMinutes").as_deref(), Some("3"));
    let fx = chat();
    let (code, out, err) = session(&fx, &[], &["archive", "system/x", "--dry-run", "--wait-minutes", "3"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("would archive session sid-1") && out.contains("archive -Session sid-1 -WaitMinutes 3"), "{out}");
    assert!(out.contains("registry row is not changed") && out.contains("take the keyboard focus for the few seconds") && out.contains("hands the foreground back"), "{out}");
    assert!(calls(&fx).is_empty(), "a dry run starts no driver");
}

#[test]
fn archive_by_session_id_works_for_a_chat_with_no_registry_row() {
    let fx = chat();
    let id = "0a1b2c3d-1111-2222-3333-444455556666";
    let (code, out, err) = session(&fx, &[], &["archive", "--session-id", id]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(arg_after(&archives(&fx)[0], "-Session").as_deref(), Some(id));
    assert!(out.starts_with(&format!("{id}: ok")), "{out}");
    assert!(!out.contains("registry row"), "an unregistered chat has no row to name: {out}");
    // An id the driver would not take is refused here.
    let (code, _, _) = session(&fx, &[], &["archive", "--session-id", "sid-1"]);
    assert_eq!(code, 1);
    assert_eq!(archives(&fx).len(), 1);
}

#[test]
fn archive_refuses_bad_arguments_before_the_driver_runs() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[], &["archive"]);
    assert_eq!(code, 1);
    assert!(err.contains("needs a chat name, or --session-id"), "{err}");
    let (code, _, err) = session(&fx, &[], &["archive", "system/nope"]);
    assert_eq!(code, 1);
    assert!(err.contains("no registry row for system/nope"), "{err}");
    let (code, _, err) = session(&fx, &[], &["archive", "--session-id", "not an id"]);
    assert_eq!(code, 1);
    assert!(err.contains("8 to 64 hex digits and dashes") && err.contains("nothing was archived"), "{err}");
    let (code, _, err) = session(&fx, &[], &["archive", "system/x", "--session-id", "0a1b2c3d-1111-2222-3333-444455556666"]);
    assert_eq!(code, 1);
    assert!(err.contains("not the session id of that registry row"), "{err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn archive_never_archives_the_chat_that_runs_it() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("CLAUDE_CODE_SESSION_ID", "sid-1")], &["archive", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("does not archive the calling chat") && err.contains("nothing was archived"), "{err}");
    let own = "0a1b2c3d-1111-2222-3333-444455556666";
    let (code, _, err) = session(&fx, &[("CLAUDE_CODE_SESSION_ID", own)], &["archive", "--session-id", own]);
    assert_eq!(code, 1);
    assert!(err.contains("does not archive the calling chat"), "{err}");
    let (code, _, err) = session(&fx, &[("CLAUDE_CODE_SESSION_ID", own)], &["archive", "system/x", "--dry-run"]);
    assert_eq!(code, 0, "another chat may archive it: {err}");
    assert!(calls(&fx).is_empty());
}

#[test]
fn archive_driver_exit_codes_become_refusals_that_say_nothing_was_archived() {
    let cases = [
        ("3", "exit 3", "nothing was archived", "chat was busy or running"),
        ("4", "exit 4", "nothing was archived", "draft typed in its input box"),
        ("5", "exit 5", "nothing was archived", "safety check failed"),
        ("7", "cancelled (exit 7)", "nothing was changed", "cancelled"),
        ("2", "could not find", "no chat row", "no chat row for session sid-1"),
        ("1", "failed (exit 1)", "the stub driver refused", "the stub driver refused"),
        ("6", "MODEL KEY MISMATCH", "", "MODEL"),
    ];
    for (code, label, extra, _) in cases {
        let fx = chat();
        let stderr = if code == "2" { "no chat row for session sid-1 in the panel" } else { "the stub driver refused" };
        let (exit, out, err) = session(&fx, &[("STUB_PANEL_EXIT", code), ("STUB_PANEL_STDERR", stderr)], &["archive", "system/x"]);
        assert_eq!(exit, 1, "{code}: {out}{err}");
        assert!(err.contains("system/x: ") && err.contains(label) && err.contains(extra), "{code}: {err}");
        assert!(!err.contains("nothing was sent"), "{code}: archive never says sent: {err}");
        assert_eq!(archives(&fx).len(), 1);
        assert!(!out.contains("registry row"), "{code}: nothing succeeded: {out}");
    }
}

// ------------------------------------------------------------------ cancel a queued compaction

const JOB: &str = "system/logs/claude-panel/waiter-sid-1.json";
const MARKER: &str = "system/logs/claude-panel/waiter-sid-1.cancel";
const SENDING: &str = "system/logs/claude-panel/waiter-sid-1.sending";

fn leftovers(fx: &Fx) -> Vec<String> {
    std::fs::read_dir(fx.path("system/logs/claude-panel")).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(40);
    while !ok() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_queued_compaction_is_cancelled_while_its_driver_only_waits_and_nothing_runs_after() {
    let fx = chat();
    let then = fx.write("then.txt", "send this next");
    let env = [("STUB_PANEL_WAIT_FOR_CANCEL", "60"), ("AF_WAITER_POLL_MS", "100")];
    let (code, out, err) = session(&fx, &env, &["compact", "system/x", "--when-idle", "--then-file", &then]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("af session compact --cancel system/x"), "the verb says how to cancel: {out}");
    wait_until("the driver to start waiting", || !sends(&fx).is_empty());
    assert!(fx.exists(JOB), "the waiter holds its job while the driver waits");
    let sent_args = sends(&fx)[0].0.clone();
    let cancel_path = arg_after(&sent_args, "-CancelFile").expect("the waiter hands the driver its cancel file");
    assert!(cancel_path.replace('\\', "/").ends_with(MARKER), "{cancel_path}");

    let (code, out, err) = session(&fx, &env, &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("system/x: cancelled") && out.contains("no --then-file step ran"), "{out}");
    assert!(!fx.exists(JOB) && !fx.exists(MARKER) && !fx.exists(SENDING), "job file and marker are gone: {:?}", leftovers(&fx));
    assert!(leftovers(&fx).is_empty(), "and so is the text copy: {:?}", leftovers(&fx));
    let log = fx.read("system/logs/panel-waiters.log");
    assert!(log.contains("system/x compact cancelled:") && !log.contains("compact failed"), "{log}");
    assert_eq!(calls(&fx).len(), 1, "the then-step never started a driver run");
    assert_eq!(boundaries(&fx), 0, "nothing was sent");
    assert!(!fx.exists("system/logs/context-nudge/sid-1/waiter-result.txt"), "a cancelled job is not a failure for the chat");

    // A later --when-idle queues again and runs.
    let (code, out, err) = session(&fx, &[], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("detached waiter") && !out.contains("already pending"), "{out}");
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("system/x compact ok:"), "{log}");
    assert_eq!(boundaries(&fx), 1);
}

#[test]
fn cancel_with_no_job_says_so() {
    let fx = chat();
    let (code, out, err) = session(&fx, &[], &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("no --when-idle compaction is queued for this chat") && out.contains("nothing to cancel"), "{out}");
    assert!(!fx.exists(MARKER), "no marker is left for a job that does not exist");
    // A job whose waiter is gone is no job.
    fx.write(JOB, &json!({"name": "system/x", "session_id": "sid-1", "pid": dead_pid(), "queued_ms": 1}).to_string());
    let (code, out, _) = session(&fx, &[], &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 0);
    assert!(out.contains("no --when-idle compaction is queued") && out.contains("waiter it names is gone"), "{out}");
    assert!(!fx.exists(MARKER));
    let (code, _, err) = session(&fx, &[], &["compact", "--cancel", "system/nope"]);
    assert_eq!(code, 1);
    assert!(err.contains("no registry row for system/nope"), "{err}");
}

#[test]
fn a_waiter_that_waits_for_the_chat_is_cancelled_before_it_starts_any_driver() {
    let fx = chat();
    let me = std::process::id();
    fx.write("cfg/sessions/1.json", &json!({"pid": me, "sessionId": "sid-1", "entrypoint": "claude-vscode", "status": "busy"}).to_string());
    let env = [("AF_WAITER_POLL_MS", "100")];
    let (code, out, err) = session(&fx, &env, &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    wait_until("the waiter to write its pid", || fx.exists(JOB) && fx.read(JOB).contains("\"pid\":") && !fx.read(JOB).contains("\"pid\":null"));
    let (code, out, err) = session(&fx, &env, &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("cancelled"), "{out}");
    assert!(calls(&fx).is_empty(), "no driver run ever started");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    assert!(fx.read("system/logs/panel-waiters.log").contains("compact cancelled:"));
}

#[test]
fn a_cancel_that_comes_after_the_driver_began_changing_the_panel_is_refused_honestly() {
    let fx = chat();
    let env = [("STUB_PANEL_WAIT_FOR_CANCEL", "4"), ("STUB_PANEL_SENDING", "1"), ("AF_CANCEL_WAIT_SECS", "1.5")];
    let (code, out, err) = session(&fx, &env, &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    wait_until("the stub to mark that it began", || fx.exists(SENDING));
    let (code, out, err) = session(&fx, &env, &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("already being sent") && err.contains("not cancelled") && err.contains("(the waiter (pid"), "{err}");
    assert!(!fx.exists(MARKER), "the refused cancel takes its marker back");
    // The compaction itself finishes and is logged as ok.
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("compact ok") && !log.contains("cancelled"), "{log}");
    assert_eq!(boundaries(&fx), 1);
    wait_until("the job file to go", || !fx.exists(JOB));
    assert!(!fx.exists(SENDING));
}

#[test]
fn a_waiter_and_driver_that_do_not_stop_in_time_are_named_and_not_killed() {
    let fx = chat();
    let env = [("STUB_PANEL_WAIT_FOR_CANCEL", "4"), ("STUB_PANEL_IGNORE_CANCEL", "1"), ("AF_CANCEL_WAIT_SECS", "1")];
    let (code, out, err) = session(&fx, &env, &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    wait_until("the driver to start", || !sends(&fx).is_empty());
    let (code, out, err) = session(&fx, &env, &["compact", "--cancel", "system/x"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("cancel marker") && err.contains("still running after 1 seconds") && err.contains("the waiter (pid ") && err.contains("Nothing was killed") && err.contains("finish the compaction; otherwise it stops at its next check") && err.contains("Run af session compact --cancel system/x again"), "{err}");
    assert!(fx.exists(MARKER), "the marker stays, so they stop when they look");
    // Nothing was killed: the waiter finishes the send by itself.
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact ok");
    assert!(log.contains("compact ok"), "{log}");
}

#[test]
fn a_waiter_that_ended_before_it_saw_the_cancel_is_reported_as_not_cancelled() {
    let fx = chat();
    let me = std::process::id();
    let start = afcore::panel::pid_start(me).unwrap();
    fx.write(JOB, &json!({"name": "system/x", "session_id": "sid-1", "pid": me, "pid_start": start, "queued": "2026-11-13T10:00:00+00:00", "queued_ms": now_ms(), "wait_minutes": 60}).to_string());
    let job = fx.path(JOB);
    let remover = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        std::fs::remove_file(job).unwrap();
    });
    let (code, _, err) = session(&fx, &[], &["compact", "--cancel", "system/x"]);
    remover.join().unwrap();
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("had already ended before it looked at the cancel") && err.contains("nothing was cancelled"), "{err}");
    assert!(!fx.exists(MARKER), "the stale marker is removed");
}

#[test]
fn a_stale_marker_is_removed_when_a_new_job_is_queued_and_does_not_stop_it() {
    let fx = chat();
    fx.write(MARKER, "{\"requested\":\"2026-11-13T09:00:00+00:00\"}");
    fx.write(SENDING, "begun");
    let (code, out, err) = session(&fx, &[("STUB_PANEL_WAIT_FOR_CANCEL", "1")], &["compact", "system/x", "--when-idle"]);
    assert_eq!(code, 0, "{out}{err}");
    let log = wait_for(&fx, "system/logs/panel-waiters.log", "compact");
    assert!(log.contains("compact ok") && !log.contains("cancelled"), "{log}");
    assert_eq!(boundaries(&fx), 1);
    assert!(!fx.exists(MARKER) && !fx.exists(SENDING), "{:?}", leftovers(&fx));
}

#[test]
fn cancel_cannot_be_combined_with_the_options_of_a_compaction() {
    let fx = chat();
    for extra in [vec!["--when-idle"], vec!["--force"], vec!["--dry-run"], vec!["--instructions", "x"]] {
        let mut args = vec!["compact", "--cancel", "system/x"];
        args.extend(extra);
        let (code, _, err) = session(&fx, &[], &args);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("cannot be used with"), "{err}");
    }
    assert!(calls(&fx).is_empty());
}

#[test]
fn the_cancel_command_is_printed_by_a_when_idle_dry_run_too() {
    let fx = chat();
    let (code, out, _) = session(&fx, &[], &["compact", "system/x", "--when-idle", "--dry-run"]);
    assert_eq!(code, 0);
    assert!(out.contains("af session compact --cancel system/x"), "{out}");
}

#[test]
fn a_driver_that_exits_7_is_a_cancelled_run_not_a_failure_in_the_message() {
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "7"), ("STUB_PANEL_STDERR", "cancelled; nothing was changed")], &["compact", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("cancelled (exit 7)") && err.contains("nothing was changed"), "{err}");
    assert!(!err.contains("failed (exit 7)"), "{err}");
}

#[test]
fn a_waiter_on_the_closed_route_stops_for_the_cancel_marker_while_it_waits_for_the_lock() {
    let fx = chat();
    let held = hold_lock(&fx, "send", "sid-other");
    let job = fx.write(JOB, &json!({"name": "system/x", "session_id": "sid-1", "instructions": null, "force": false, "then_file": null, "wait_minutes": 60, "queued_ms": now_ms(), "transcript_len": 0}).to_string());
    let mut c = fx.af();
    stub_env(&fx, &mut c);
    c.env("STUB_ROSTER", "[]").env("AF_WAITER_POLL_MS", "100");
    let mut child = std::process::Command::new(c.get_program()).args(["session", "compact", "system/x", "--waiter-job", &job]).envs(c.get_envs().filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))).current_dir(fx.path(".")).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(child.try_wait().unwrap().is_none(), "it waits for the lock");
    fx.write(MARKER, "{}");
    wait_until("the waiter to stop", || child.try_wait().unwrap().is_some());
    drop(held);
    let log = fx.read("system/logs/panel-waiters.log");
    assert!(log.contains("compact cancelled:") && log.contains("panel lock"), "{log}");
    assert!(!fx.exists(JOB) && !fx.exists(MARKER));
    assert!(!fx.exists("lock-seen.txt") || !fx.read("lock-seen.txt").contains("headless-compact"), "claude -p never ran");
}

#[test]
fn archive_prints_what_the_driver_says_when_the_list_has_no_row_unchanged_and_names_the_chat_in_front_refusal() {
    let fx = chat();
    let said = "no row for this chat is in the list as shown (already archived, never listed, or inside a collapsed group); nothing was changed";
    let (code, out, err) = session(&fx, &[("STUB_PANEL_OUT", said)], &["archive", "system/x"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(&format!("system/x: {said}")), "the driver's words are printed unchanged: {out}");
    let fx = chat();
    let (code, _, err) = session(&fx, &[("STUB_PANEL_EXIT", "3"), ("STUB_PANEL_STDERR", "it is the chat he has in front; nothing was changed. Nothing was archived.")], &["archive", "system/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("refused (exit 3): nothing was archived") && err.contains("the chat is the one he has in front") && err.contains("it is the chat he has in front; nothing was changed"), "{err}");
}
