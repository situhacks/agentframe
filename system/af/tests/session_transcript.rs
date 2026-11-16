//! `af session transcript <name> --around <time> [--minutes N]`: the conversation around one moment.

mod common;
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Utc};
use common::*;
use serde_json::{json, Value};

const SESSION: &str = "s-trans";
const CENTER: &str = "2026-11-15T23:12:00Z";

fn center() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(CENTER).unwrap().with_timezone(&Utc)
}

/// The time `minutes` from the centre, as the transcript writes it.
fn at(minutes: i64) -> String {
    (center() + Duration::minutes(minutes)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn user(minutes: i64, text: &str) -> Value {
    json!({"type": "user", "timestamp": at(minutes), "message": {"role": "user", "content": text}})
}

fn assistant(minutes: i64, blocks: Value) -> Value {
    json!({"type": "assistant", "timestamp": at(minutes), "message": {"role": "assistant", "content": blocks}})
}

fn text(minutes: i64, t: &str) -> Value {
    assistant(minutes, json!([{"type": "text", "text": t}]))
}

fn fixture(records: &[Value]) -> Fx {
    let fx = Fx::new();
    let lines: Vec<String> = records.iter().map(|r| r.to_string()).collect();
    fx.write(&format!("cfg/projects/p/{SESSION}.jsonl"), &format!("{}\n", lines.join("\n")));
    fx.ok(&["session", "register", "project/demo", "--session", SESSION]);
    fx
}

fn transcript(fx: &Fx, args: &[&str]) -> (i32, String, String) {
    let mut cmd = fx.af();
    cmd.env("CLAUDE_CONFIG_DIR", fx.p("cfg")).args(["session", "transcript"]).args(args);
    let o = cmd.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&o.stderr).replace("\r\n", "\n"),
    )
}

#[test]
fn only_the_window_prints_and_the_newest_line_is_last() {
    let fx = fixture(&[
        user(-30, "far before"),
        user(-19, "early inside"),
        text(0, "the middle"),
        user(19, "late inside"),
        text(21, "far after"),
    ]);
    let (code, out, err) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert_eq!(code, 0, "{err}");
    assert!(!out.contains("far before") && !out.contains("far after"), "{out}");
    let (a, b, c) = (out.find("early inside").unwrap(), out.find("the middle").unwrap(), out.find("late inside").unwrap());
    assert!(a < b && b < c, "oldest first, newest last: {out}");
    assert!(out.contains("user: early inside") && out.contains("assistant: the middle"), "{out}");

    let (_, narrow, _) = transcript(&fx, &["project/demo", "--around", CENTER, "--minutes", "5"]);
    assert!(narrow.contains("the middle") && !narrow.contains("early inside") && !narrow.contains("late inside"), "{narrow}");
    let (_, wide, _) = transcript(&fx, &["project/demo", "--around", CENTER, "--minutes", "25"]);
    assert!(wide.contains("far after") && !wide.contains("far before"), "{wide}");
}

#[test]
fn a_window_with_nothing_in_it_says_so_and_exits_0() {
    let fx = fixture(&[user(-300, "long ago"), user(300, "much later")]);
    let (code, out, _) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert_eq!(code, 0);
    assert!(out.contains("nothing said or done in that window"), "{out}");
}

#[test]
fn sidechains_tool_output_and_thinking_stay_out() {
    let fx = fixture(&[
        user(-1, "please fix the guard"),
        json!({"type": "assistant", "isSidechain": true, "timestamp": at(0), "message": {"content": [{"type": "text", "text": "SIDECHAIN TEXT"}]}}),
        json!({"type": "user", "isSidechain": true, "timestamp": at(0), "message": {"content": "SIDECHAIN PROMPT"}}),
        assistant(0, json!([
            {"type": "thinking", "thinking": "HIDDEN THOUGHT"},
            {"type": "text", "text": "looking now"},
            {"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "cargo test"}},
        ])),
        json!({"type": "user", "timestamp": at(1), "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "SECRET TOOL OUTPUT"}]}}),
        json!({"type": "user", "isMeta": true, "timestamp": at(1), "message": {"content": "META NOTE"}}),
        json!({"type": "system", "timestamp": at(1), "content": "SYSTEM RECORD"}),
        json!({"type": "user", "timestamp": at(2), "message": {"content": [{"type": "text", "text": "user text in a block"}]}}),
    ]);
    let (code, out, err) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert_eq!(code, 0, "{err}");
    for hidden in ["SIDECHAIN TEXT", "SIDECHAIN PROMPT", "HIDDEN THOUGHT", "SECRET TOOL OUTPUT", "META NOTE", "SYSTEM RECORD"] {
        assert!(!out.contains(hidden), "{hidden} printed: {out}");
    }
    assert!(out.contains("user: please fix the guard") && out.contains("assistant: looking now") && out.contains("user: user text in a block"), "{out}");
    assert!(out.lines().any(|l| l.ends_with("tool: Bash cargo test")), "{out}");
}

#[test]
fn a_tool_call_is_one_line_of_its_name_and_the_first_120_characters() {
    let long = format!("echo {}", "x".repeat(300));
    let fx = fixture(&[
        assistant(0, json!([{"type": "tool_use", "id": "a", "name": "Bash", "input": {"command": long}}])),
        assistant(1, json!([{"type": "tool_use", "id": "b", "name": "Write",
            "input": {"content": "BODY OF THE FILE\nwith lines", "file_path": "workspace/life/sources/orders/x.md"}}])),
        assistant(2, json!([{"type": "tool_use", "id": "c", "name": "Mystery", "input": {"alpha": "one\ntwo", "beta": 2}}])),
    ]);
    let (_, out, _) = transcript(&fx, &["project/demo", "--around", CENTER]);
    let bash = out.lines().find(|l| l.contains("tool: Bash")).unwrap();
    let detail = bash.split("tool: Bash ").nth(1).unwrap();
    assert_eq!(detail.chars().count(), 120, "{bash}");
    assert!(detail.starts_with("echo xxx"));
    let write = out.lines().find(|l| l.contains("tool: Write")).unwrap();
    assert!(write.ends_with("tool: Write workspace/life/sources/orders/x.md"), "the path, not the body: {write}");
    assert!(!out.contains("BODY OF THE FILE"));
    let other = out.lines().find(|l| l.contains("tool: Mystery")).unwrap();
    assert!(other.contains(r#"{"alpha":"one\ntwo","beta":2}"#), "no salient field: its JSON on one line: {other}");
}

#[test]
fn the_output_is_capped_by_words_and_the_oldest_lines_go_first() {
    let paragraph = vec!["word"; 500].join(" ");
    let mut recs = Vec::new();
    for i in 0..30 {
        recs.push(user(-15 + i / 2, &format!("marker{i:02} {paragraph}")));
    }
    let fx = fixture(&recs);
    let (code, out, err) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert_eq!(code, 0, "{err}");
    let body: usize = out.lines().filter(|l| l.starts_with('[')).map(|l| l.split_whitespace().count()).sum();
    assert!(body <= 6000, "{body} words");
    assert!(body > 5000, "about the cap, not far under it: {body}");
    assert!(out.contains("marker29") && !out.contains("marker00"), "the newest stay: {out}");
    assert!(out.contains("earlier lines left out"), "{out}");
}

#[test]
fn a_local_time_names_the_same_moment_as_its_rfc_3339_form() {
    let fx = fixture(&[text(0, "at the centre"), text(15, "fifteen later")]);
    let local: NaiveDateTime = center().with_timezone(&Local).naive_local();
    let spelled = local.format("%Y-%m-%d %H:%M").to_string();
    let (code, out, err) = transcript(&fx, &["project/demo", "--around", &spelled, "--minutes", "10"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("at the centre") && !out.contains("fifteen later"), "{out}");
    assert_eq!(afcore::transcript::parse_around(&spelled), Local.from_local_datetime(&local).earliest().map(|d| d.with_timezone(&Utc)));
    assert_eq!(afcore::transcript::parse_around(CENTER), Some(center()));
    assert_eq!(afcore::transcript::parse_around("yesterday"), None);
}

#[test]
fn an_unknown_name_a_bad_time_and_a_missing_transcript_exit_2() {
    let fx = fixture(&[text(0, "x")]);
    let (code, out, err) = transcript(&fx, &["project/nope", "--around", CENTER]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("no registry row for project/nope"), "{err}");
    let (code, _, err) = transcript(&fx, &["project/demo", "--around", "yesterday"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--around is a time"), "{err}");
    let (code, _, err) = transcript(&fx, &["project/demo", "--around", CENTER, "--minutes", "0"]);
    assert_eq!(code, 2, "{err}");
    fx.ok(&["session", "register", "project/ghost", "--session", "s-none"]);
    let (code, _, err) = transcript(&fx, &["project/ghost", "--around", CENTER]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no transcript on this machine for project/ghost"), "{err}");
}

#[test]
fn an_oldest_line_that_mostly_fits_is_trimmed_not_dropped() {
    let big = format!("FIRSTWORD {} LASTWORD", vec!["filler"; 2200].join(" "));
    let para = vec!["word"; 500].join(" ");
    let mut recs = vec![user(-15, &big)];
    for i in 0..8 {
        recs.push(user(-10 + i, &format!("later{i} {para}")));
    }
    let fx = fixture(&recs);
    let (code, out, err) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert_eq!(code, 0, "{err}");
    let body: usize = out.lines().filter(|l| l.starts_with('[') || !l.starts_with("af session") && !l.starts_with("(over")).map(|l| l.split_whitespace().count()).sum();
    assert!(body <= 6000, "{body} words");
    assert!(out.contains("LASTWORD"), "the end of the old message stays: {out}");
    assert!(!out.contains("FIRSTWORD"), "its start is the part cut");
    assert!(out.contains("(trimmed: its first "), "the cut is marked: {out}");
    assert!(out.contains("0 earlier lines left out"), "no whole line was dropped: {out}");
    assert!(out.contains("later7") && out.contains("later0"), "{out}");
}

#[test]
fn a_compaction_summary_is_not_printed_as_the_users_words() {
    let fx = fixture(&[
        user(-1, "a real message"),
        json!({"type": "user", "isCompactSummary": true, "timestamp": at(0), "message": {"role": "user", "content": "This session is being continued... SUMMARY TEXT"}}),
        text(1, "reply"),
    ]);
    let (_, out, _) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert!(!out.contains("SUMMARY TEXT"), "{out}");
    assert!(out.contains("user: a real message") && out.contains("assistant: reply"));
}

#[test]
fn a_window_before_the_transcript_begins_says_so() {
    let fx = fixture(&[user(0, "the first record")]);
    let before = (center() - Duration::days(3)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let (code, out, _) = transcript(&fx, &["project/demo", "--around", &before]);
    assert_eq!(code, 0);
    assert!(out.contains("before this chat's transcript begins") && out.contains("latest session"), "{out}");
    assert!(!out.contains("nothing said or done"), "{out}");
    let after = (center() + Duration::days(3)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let (_, out, _) = transcript(&fx, &["project/demo", "--around", &after]);
    assert!(out.contains("after this chat's last record"), "{out}");
    let (_, out, _) = transcript(&fx, &["project/demo", "--around", CENTER, "--minutes", "1"]);
    assert!(out.contains("the first record"));
}

#[test]
fn a_huge_minutes_value_exits_2_instead_of_crashing() {
    let fx = fixture(&[text(0, "x")]);
    for m in ["9999999999999", "9223372036854775807"] {
        let (code, _, err) = transcript(&fx, &["project/demo", "--around", CENTER, "--minutes", m]);
        assert_eq!(code, 2, "{m}: {err}");
        assert!(err.contains("--minutes"), "{err}");
        assert!(!err.contains("panicked"), "{err}");
    }
}

#[test]
fn the_top_level_timestamp_decides_not_one_inside_a_tool_call() {
    // The nested timestamp comes first in the line and lies far outside the window.
    let rec = format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"t","name":"Bash","input":{{"timestamp":"2001-01-01T00:00:00Z","command":"echo hi"}}}}]}},"timestamp":"{}"}}"#,
        at(0)
    );
    let fx = Fx::new();
    fx.write(&format!("cfg/projects/p/{SESSION}.jsonl"), &format!("{rec}
"));
    fx.ok(&["session", "register", "project/demo", "--session", SESSION]);
    let (_, out, _) = transcript(&fx, &["project/demo", "--around", CENTER]);
    assert!(out.contains("tool: Bash echo hi"), "{out}");
}

#[test]
fn a_time_without_seconds_and_with_z_or_an_offset_is_accepted() {
    assert_eq!(afcore::transcript::parse_around("2026-11-15T23:12Z"), Some(center()));
    assert_eq!(afcore::transcript::parse_around("2026-11-15T23:12:00Z"), Some(center()));
    assert_eq!(afcore::transcript::parse_around("2026-11-15T22:12-01:00"), Some(center()));
    assert_eq!(afcore::transcript::parse_around("2026-11-15T23:12"), afcore::transcript::parse_around("2026-11-15 23:12"));
}
