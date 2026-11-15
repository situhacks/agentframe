//! `af session warm`: a chat keeps its own prompt cache warm through a long wait. Transcripts are fixture
//! JSONL lines as Claude Code writes them (`message.usage`, `isSidechain`, `compact_boundary`). The test
//! hook `AF_WARM_MINUTE_SECONDS` sets the length of a minute in seconds, so the 5-minute floor, the 30-second
//! poll and the 12-hour cap all run in seconds.

mod common;
use common::*;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const TRANSCRIPT: &str = "cfg/projects/p1/sid-1.jsonl";
const LOCK: &str = "system/logs/warm/sid-1.json";
const LOG: &str = "system/logs/warm/warm.log";

/// An RFC 3339 stamp `secs` seconds before now (negative: in the future).
fn ts(secs: f64) -> String {
    let t = chrono::Utc::now() - chrono::Duration::milliseconds((secs * 1000.0) as i64);
    t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// A main-chain reply of `total` context tokens whose cache write was `five` tokens of the 5-minute kind
/// and `hour` tokens of the 1-hour kind (the rest is read from cache).
fn reply_at(stamp: &str, total: u64, five: u64, hour: u64, sidechain: bool) -> String {
    let created = five + hour;
    let mut line = json!({
        "type": "assistant",
        "timestamp": stamp,
        "message": {"role": "assistant", "usage": {
            "input_tokens": 1,
            "cache_read_input_tokens": total - 1 - created,
            "cache_creation_input_tokens": created,
            "cache_creation": {"ephemeral_5m_input_tokens": five, "ephemeral_1h_input_tokens": hour},
            "output_tokens": 5
        }}
    });
    if sidechain {
        line["isSidechain"] = json!(true);
    }
    line.to_string()
}

/// A reply with a 1-hour cache write.
fn reply(stamp: &str, total: u64) -> String {
    reply_at(stamp, total, 0, 1861, false)
}

const BOUNDARY: &str = r#"{"type":"system","subtype":"compact_boundary","content":"Conversation compacted"}"#;

fn chat(lines: &[String]) -> Fx {
    let fx = Fx::new();
    fx.ok(&["session", "register", "system/x", "--session", "sid-1"]);
    fx.write(TRANSCRIPT, &format!("{}\n", lines.join("\n")));
    fx
}

/// Write the transcript again, so stamps taken now are not stale by the time the verb starts (registering is slow).
fn fresh(fx: &Fx, lines: &[String]) {
    fx.write(TRANSCRIPT, &format!("{}\n", lines.join("\n")));
}

fn append(fx: &Fx, line: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(fx.path(TRANSCRIPT)).unwrap();
    writeln!(f, "{line}").unwrap();
}

type Out = (i32, String, String);

fn command(fx: &Fx, scale: f64, args: &[&str]) -> assert_cmd::Command {
    let mut c = fx.af();
    c.env("CLAUDE_CONFIG_DIR", fx.path("cfg")).env("AF_WARM_MINUTE_SECONDS", scale.to_string());
    let mut full = vec!["session", "warm", "system/x"];
    full.extend_from_slice(args);
    c.args(full);
    c
}

fn done(o: std::process::Output) -> Out {
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Run `af session warm system/x` to its end.
fn warm(fx: &Fx, scale: f64, args: &[&str]) -> Out {
    done(command(fx, scale, args).output().unwrap())
}

/// Start it on a thread, so the test can change the transcript while it waits.
fn spawn(fx: &Fx, scale: f64, args: &[&str]) -> std::thread::JoinHandle<(Out, Duration)> {
    let mut c = command(fx, scale, args);
    std::thread::spawn(move || {
        let start = Instant::now();
        let out = done(c.output().unwrap());
        (out, start.elapsed())
    })
}

fn lines(out: &str) -> Vec<&str> {
    out.lines().collect()
}

/// A stop prints exactly its one line and exits 0; nothing is left locked.
fn assert_stop(fx: &Fx, out: &Out, line: &str) {
    assert_eq!(out.0, 0, "{out:?}");
    assert_eq!(lines(&out.1), vec![line], "{out:?}");
    assert!(!fx.exists(LOCK), "a stop leaves no lock");
    let log = fx.read(LOG);
    assert!(log.contains(&format!("outcome={}", line.trim_start_matches("stop: "))), "the log names the stop: {log}");
}

fn wait_for_lock(fx: &Fx) {
    let until = Instant::now() + Duration::from_secs(10);
    while !fx.exists(LOCK) {
        assert!(Instant::now() < until, "the timer never took its lock");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

// ------------------------------------------------------------------ firing

#[test]
fn fires_after_idle_with_three_exact_lines_and_the_options_carried_into_the_rearm() {
    // A minute is 0.4 s: 5 minutes of idleness is 2 s, counted from the chat's last reply.
    let fx = chat(&[]);
    fresh(&fx, &[reply(&ts(0.0), 312_000)]);
    let start = Instant::now();
    let out = warm(&fx, 0.4, &["--wake", "2", "--minutes", "5", "--max-wakes", "6", "--min-tokens", "100000"]);
    assert!(start.elapsed() >= Duration::from_millis(1500), "it must wait for the idle time: {:?}", start.elapsed());
    assert_eq!(out.0, 0, "{out:?}");
    let got = lines(&out.1);
    assert_eq!(got.len(), 3, "{out:?}");
    let idle: u64 = got[0]
        .strip_prefix("warm: wake 2 of 6 for system/x · idle ")
        .and_then(|r| r.strip_suffix(" min · context 312k · cache 1h"))
        .unwrap_or_else(|| panic!("first line is wrong: {}", got[0]))
        .parse()
        .unwrap();
    assert!((5..=8).contains(&idle), "idle minutes {idle}");
    assert_eq!(
        got[1],
        "Still waiting on work you will return to? Re-arm in the background: af session warm system/x --wake 3 --minutes 5 --max-wakes 6 --min-tokens 100000"
    );
    assert_eq!(got[2], "Task in hand done? Compact instead: af session compact system/x --when-idle");
    assert!(!fx.exists(LOCK), "the lock goes with the fire");
    let log = fx.read(LOG);
    assert!(log.contains("name=system/x wake=2 outcome=fired") && log.contains("context=312000"), "{log}");
}

#[test]
fn a_chat_already_idle_fires_at_once_and_default_options_are_not_repeated() {
    // At a 1 s minute the last reply is 60 minutes old: past the default 55.
    let fx = chat(&[reply(&ts(60.0), 400_000)]);
    let out = warm(&fx, 1.0, &[]);
    assert_eq!(out.0, 0, "{out:?}");
    let got = lines(&out.1);
    assert_eq!(got.len(), 3, "{out:?}");
    assert!(got[0].starts_with("warm: wake 1 of 12 for system/x · idle 6") && got[0].ends_with(" min · context 400k · cache 1h"), "{}", got[0]);
    assert_eq!(got[1], "Still waiting on work you will return to? Re-arm in the background: af session warm system/x --wake 2");
}

#[test]
fn a_chat_with_no_cache_creation_data_is_assumed_to_be_on_the_hour_cache_and_says_so() {
    let line = json!({"type": "assistant", "timestamp": ts(60.0), "message": {"usage": {"input_tokens": 1, "cache_read_input_tokens": 299_999}}}).to_string();
    let fx = chat(&[line]);
    let out = warm(&fx, 1.0, &[]);
    assert_eq!(out.0, 0, "{out:?}");
    assert!(lines(&out.1)[0].ends_with(" · context 300k · cache 1h (assumed: no cache_creation data)"), "{out:?}");
}

#[test]
fn new_activity_during_the_wait_pushes_the_fire_time_back() {
    // A minute is 0.5 s, so the 5 idle minutes are 2.5 s. The reply is 1 s old when it starts; a new reply
    // lands 1.5 s in, so the fire cannot come before 4 s. Without the reset it would fire at once.
    let fx = chat(&[]);
    fresh(&fx, &[reply(&ts(1.0), 312_000)]);
    let run = spawn(&fx, 0.5, &["--minutes", "5"]);
    wait_for_lock(&fx);
    let lock: serde_json::Value = serde_json::from_str(&fx.read(LOCK)).unwrap();
    assert!(lock["pid"].as_u64().unwrap() > 0 && lock["wake"] == 1 && lock["armed"].as_str().is_some(), "{lock}");
    wait_ms(1200);
    append(&fx, &reply(&ts(0.0), 313_000));
    let ((code, out, err), elapsed) = run.join().unwrap();
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.starts_with("warm: wake 1 of 12 for system/x"), "{out}");
    assert!(elapsed >= Duration::from_millis(3500), "the new reply must reset the countdown: fired after {elapsed:?}");
    assert!(out.contains("context 313k"), "it reports the context at the fire: {out}");
    assert!(!fx.exists(LOCK));
}

#[test]
fn a_sidechain_reply_counts_neither_as_activity_nor_as_the_cache_kind() {
    // The main chain has been idle for 60 s (a 1 s minute); a newer subagent reply wrote only the 5-minute cache.
    // Counted, it would reset the idle time and stop the run on the cache kind; ignored, it fires at once.
    let fx = chat(&[reply(&ts(60.0), 312_000), reply_at(&ts(0.0), 20_000, 1000, 0, true)]);
    let start = Instant::now();
    let out = warm(&fx, 1.0, &["--minutes", "5"]);
    assert!(start.elapsed() < Duration::from_secs(4), "{:?}", start.elapsed());
    assert_eq!(out.0, 0, "{out:?}");
    assert!(lines(&out.1)[0].starts_with("warm: wake 1 of 12 for system/x · idle 6"), "{out:?}");
    assert!(lines(&out.1)[0].ends_with("context 312k · cache 1h"), "{out:?}");
}

// ------------------------------------------------------------------ stops before waiting

#[test]
fn stops_when_the_wake_cap_is_reached() {
    let fx = chat(&[reply(&ts(0.0), 312_000)]);
    let out = warm(&fx, 1.0, &["--wake", "3", "--max-wakes", "2"]);
    assert_stop(&fx, &out, "stop: 2 wakes reached; let the cache lapse, or compact if the task is done");
}

#[test]
fn stops_when_the_context_is_below_the_floor() {
    let fx = chat(&[reply(&ts(0.0), 80_000)]);
    let out = warm(&fx, 1.0, &[]);
    assert_stop(&fx, &out, "stop: context is 80k, below 150k: a cold restart rewrites little; not warming");
    // The floor is an option.
    let out = warm(&fx, 1.0, &["--min-tokens", "90000"]);
    assert_stop(&fx, &out, "stop: context is 80k, below 90k: a cold restart rewrites little; not warming");
}

#[test]
fn stops_when_the_context_is_unknown_or_the_chat_was_compacted_after_its_last_reply() {
    let no_usage = chat(&[json!({"type": "user", "message": {"content": "hi"}}).to_string()]);
    let out = warm(&no_usage, 1.0, &[]);
    assert_eq!(out.0, 0, "{out:?}");
    assert!(out.1.starts_with("stop: context is unknown") && out.1.contains("below 150k: a cold restart rewrites little; not warming"), "{out:?}");
    assert!(!no_usage.exists(LOCK));

    let compacted = chat(&[reply(&ts(0.0), 312_000), BOUNDARY.to_string()]);
    let out = warm(&compacted, 1.0, &[]);
    assert_eq!(out.0, 0, "{out:?}");
    assert!(out.1.starts_with("stop: context is unknown") && out.1.contains("below 150k"), "{out:?}");
    assert!(!compacted.exists(LOCK));
}

#[test]
fn stops_on_the_5_minute_cache() {
    // The newest reply that wrote the cache wrote only the 5-minute kind; an older 1-hour write does not rescue it.
    let fx = chat(&[reply(&ts(100.0), 300_000), reply_at(&ts(60.0), 310_000, 4000, 0, false), reply_at(&ts(50.0), 311_000, 0, 0, false)]);
    let out = warm(&fx, 1.0, &[]);
    assert_stop(
        &fx,
        &out,
        "stop: this chat is on the 5-minute cache (usage overage); warming would need a wake every 4 minutes. If the task in hand is done, compact instead.",
    );
}

#[test]
fn stops_when_a_live_timer_already_holds_the_chat_and_leaves_that_timer_alone() {
    let fx = chat(&[reply(&ts(0.0), 312_000)]);
    let pid = std::process::id();
    let armed = "2026-11-15T01:02:03+00:00";
    let body = json!({"pid": pid, "armed": armed, "wake": 1}).to_string();
    fx.write(LOCK, &body);
    let out = warm(&fx, 1.0, &[]);
    let hhmm = chrono::DateTime::parse_from_rfc3339(armed).unwrap().with_timezone(&chrono::Local).format("%H:%M").to_string();
    assert_eq!(out.0, 0, "{out:?}");
    assert_eq!(lines(&out.1), vec![format!("stop: already armed since {hhmm} (pid {pid}); nothing started")], "{out:?}");
    assert_eq!(fx.read(LOCK), body, "the live timer's lock is not this run's to remove");
}

#[test]
fn a_dead_timers_lock_is_taken_over_and_removed_at_the_end() {
    let fx = chat(&[reply(&ts(60.0), 312_000)]);
    let mut dead = std::process::Command::new(std::env::current_exe().unwrap()).arg("--list").stdout(std::process::Stdio::null()).spawn().unwrap();
    let pid = dead.id();
    dead.wait().unwrap();
    fx.write(LOCK, &json!({"pid": pid, "armed": "2026-11-15T01:02:03+00:00", "wake": 4}).to_string());
    let out = warm(&fx, 1.0, &["--minutes", "5"]);
    assert_eq!(out.0, 0, "{out:?}");
    assert!(out.1.starts_with("warm: wake 1 of 12"), "a dead pid's lock is not a timer: {out:?}");
    assert!(!fx.exists(LOCK), "taken over, then removed");
}

// ------------------------------------------------------------------ stops while waiting

#[test]
fn stops_when_the_chat_compacts_after_the_timer_was_armed() {
    let fx = chat(&[reply(&ts(0.0), 312_000)]);
    let run = spawn(&fx, 1.0, &["--minutes", "5"]);
    wait_for_lock(&fx);
    wait_ms(300);
    append(&fx, BOUNDARY);
    let ((code, out, err), elapsed) = run.join().unwrap();
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(lines(&out), vec!["stop: this chat compacted; nothing to keep warm"]);
    assert!(elapsed < Duration::from_secs(4), "it stops at the next poll, not at the fire time: {elapsed:?}");
    assert!(!fx.exists(LOCK));
    assert!(fx.read(LOG).contains("outcome=this chat compacted; nothing to keep warm"));
}

#[test]
fn ends_after_twelve_hours_armed() {
    // A minute is 5 ms, so 12 hours are 3.6 s. The last reply is an hour in the future: it never counts as idle.
    let fx = chat(&[reply(&ts(-3600.0), 312_000)]);
    let start = Instant::now();
    let out = warm(&fx, 0.005, &[]);
    assert!(start.elapsed() >= Duration::from_millis(3000), "{:?}", start.elapsed());
    assert_stop(&fx, &out, "stop: armed 12 hours; ended");
}

#[test]
fn a_transcript_that_disappears_exits_1_and_still_frees_the_lock() {
    let fx = chat(&[reply(&ts(0.0), 312_000)]);
    let run = spawn(&fx, 1.0, &["--minutes", "5"]);
    wait_for_lock(&fx);
    let path: PathBuf = fx.path(TRANSCRIPT);
    let until = Instant::now() + Duration::from_secs(5);
    while std::fs::remove_file(&path).is_err() {
        assert!(Instant::now() < until, "could not remove the transcript");
        wait_ms(10);
    }
    let ((code, out, err), _) = run.join().unwrap();
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("is gone"), "{err}");
    assert!(!fx.exists(LOCK));
    assert!(fx.read(LOG).contains("its transcript is gone"));
}

// ------------------------------------------------------------------ arguments

#[test]
fn minutes_outside_5_to_58_and_an_unknown_name_exit_2() {
    let fx = chat(&[reply(&ts(0.0), 312_000)]);
    for bad in ["4", "59"] {
        let (code, out, err) = warm(&fx, 1.0, &["--minutes", bad]);
        assert_eq!(code, 2, "--minutes {bad}: {out}{err}");
        assert!(err.contains("--minutes is 5 to 58"), "{err}");
    }
    let (code, out, err) = done(
        fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).args(["session", "warm", "no/such-chat"]).output().unwrap(),
    );
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("no registry row for no/such-chat"), "{err}");
    assert!(!fx.exists(LOCK) && !fx.exists(LOG), "an argument error starts nothing and logs nothing");
}

#[test]
fn the_verb_is_listed_in_the_session_help() {
    let fx = Fx::new();
    let help = fx.ok(&["session", "--help"]);
    assert!(help.contains("warm"), "{help}");
    let verb = fx.ok(&["session", "warm", "--help"]);
    for flag in ["--wake", "--minutes", "--max-wakes", "--min-tokens"] {
        assert!(verb.contains(flag), "{flag} is documented: {verb}");
    }
}
