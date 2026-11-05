//! `af session reach`: the state a message would meet, and whether one arrived. The transcript
//! records here have the shapes real chats wrote during the cross-session test.

mod common;

use afcore::reach::{delivered_at, last_mode, verdict, Facts};
use common::*;

const BYPASS: &str = "bypassPermissions";

fn facts<'a>(roster: Option<&'a str>, target: Option<&'a str>, sender: Option<&'a str>) -> Facts<'a> {
    Facts { name: "life/x", session_id: "sid-1", roster, target_mode: target, sender_mode: sender, inbound: None, since: "2026-11-05T04:25:00Z" }
}

#[test]
fn same_class_idle_starts_a_turn_and_asks_for_the_confirm() {
    let r = verdict(&facts(Some("idle"), Some(BYPASS), Some(BYPASS)));
    assert_eq!(r.state, "live-idle");
    assert!(r.line.contains("starts a new turn"), "{}", r.line);
    assert!(r.line.contains("af session reach life/x --confirm"), "{}", r.line);
    assert!(r.line.contains("--since 2026-11-05T04:25:00Z"), "the line carries the time to bound by: {}", r.line);
    assert_eq!(verdict(&facts(Some("idle"), Some("default"), Some("acceptEdits"))).state, "live-idle");
}

#[test]
fn same_class_busy_lands_between_tool_calls() {
    let r = verdict(&facts(Some("busy"), Some(BYPASS), Some(BYPASS)));
    assert_eq!(r.state, "live-busy");
    assert!(r.line.contains("between its tool calls"), "{}", r.line);
}

#[test]
fn different_classes_hold_in_either_direction_and_the_line_says_do_not_send() {
    for (target, sender) in [(BYPASS, "default"), ("default", BYPASS), (BYPASS, "acceptEdits")] {
        let r = verdict(&facts(Some("idle"), Some(target), Some(sender)));
        assert_eq!(r.state, "holding", "{target} {sender}");
        assert!(r.line.contains("Do not send"), "{}", r.line);
        assert!(r.line.contains("SendMessage still answers success"), "{}", r.line);
    }
}

#[test]
fn the_user_setting_decides_over_the_classes() {
    let mut f = facts(Some("idle"), Some(BYPASS), Some("default"));
    f.inbound = Some("accept");
    assert_eq!(verdict(&f).state, "live-idle");
    f.inbound = Some("hold");
    assert_eq!(verdict(&f).state, "holding");
    f.inbound = Some("refuse");
    assert_eq!(verdict(&f).state, "refusing");
}

#[test]
fn a_chat_off_the_roster_is_closed_and_the_line_names_the_resume() {
    let r = verdict(&facts(Some("absent"), Some(BYPASS), Some(BYPASS)));
    assert_eq!(r.state, "closed");
    assert!(r.line.contains("claude -p \"<message>\" --resume sid-1"), "{}", r.line);
    assert_eq!(verdict(&facts(Some("completed"), None, None)).state, "closed");
    for tool in afcore::reach::ORDER_TOOLS {
        assert!(r.line.contains(tool), "the resume line denies {tool}: {}", r.line);
    }
    assert!(r.line.contains("--disallowedTools"), "{}", r.line);
    for tool in ["create_order_instruction", "create_option_instruction", "create_flow", "update_flow_status"] {
        assert!(afcore::reach::ORDER_TOOLS.iter().any(|t| t.ends_with(tool)), "{tool}");
    }
}

#[test]
fn no_roster_is_unknown_and_never_claims_a_turn() {
    let r = verdict(&facts(None, Some(BYPASS), Some(BYPASS)));
    assert_eq!(r.state, "unknown");
    assert!(r.line.contains("do not say a message started a turn"), "{}", r.line);
}

#[test]
fn an_unknown_mode_says_a_hold_is_possible() {
    let r = verdict(&facts(Some("idle"), None, Some(BYPASS)));
    assert_eq!(r.state, "live-idle");
    assert!(r.line.contains("a hold is possible"), "{}", r.line);
}

#[test]
fn a_prompt_in_the_way_is_reported_as_waiting_on_him() {
    assert_eq!(verdict(&facts(Some("waiting"), Some(BYPASS), Some(BYPASS))).state, "live-waiting");
}

// ------------------------------------------------------------------ transcripts

const ENQUEUED_ONLY: &str = r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-11-05T04:25:23.000Z","content":"<cross-session-message from=\"uds:x\" from-name=\"af-main\">\nPING-abc do the thing\n</cross-session-message>"}"#;
const DELIVERED: &str = r#"{"type":"user","permissionMode":"default","isMeta":true,"origin":{"kind":"peer","from":"uds:x"},"timestamp":"2026-11-05T04:25:24.000Z","message":{"role":"user","content":"Another Claude session sent a message:\n<cross-session-message from=\"uds:x\" from-name=\"af-main\">\nPING-abc do the thing\n</cross-session-message>"}}"#;
const QUEUED_MID_TURN: &str = r#"{"type":"attachment","timestamp":"2026-11-05T04:25:30.000Z","attachment":{"type":"queued_command","prompt":"<cross-session-message from=\"uds:x\">\nPING-def come back later\n</cross-session-message>","commandMode":"prompt"}}"#;
const READ_MID_TURN: &str = r#"{"type":"queue-operation","operation":"remove","reason":"absorbed_mid_turn","timestamp":"2026-11-05T04:26:08.000Z","content":"<cross-session-message from=\"uds:x\">\nPING-def come back later\n</cross-session-message>"}"#;
const REMOVED_UNREAD: &str = r#"{"type":"queue-operation","operation":"remove","reason":"expired","timestamp":"2026-11-05T04:26:08.000Z","content":"<cross-session-message from=\"uds:x\">\nPING-def come back later\n</cross-session-message>"}"#;
const OWN_PROMPT: &str = r#"{"type":"user","permissionMode":"bypassPermissions","timestamp":"2026-11-05T04:25:10.000Z","message":{"role":"user","content":"Send the session the exact text PING-own to the worker"}}"#;
const PASTED_TAG: &str = r#"{"type":"user","timestamp":"2026-11-05T04:25:10.000Z","message":{"role":"user","content":"What does <cross-session-message from=\"uds:x\">\nPING-abc do the thing\n</cross-session-message> mean?"}}"#;
const TOOL_RESULT: &str = r#"{"type":"user","timestamp":"2026-11-05T04:25:10.000Z","toolUseResult":{"stdout":"x"},"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"<cross-session-message from=\"uds:x\">\nPING-abc do the thing\n</cross-session-message>"}]}}"#;
const SENDER_NAMED: &str = r#"{"type":"user","origin":{"kind":"peer","from":"uds:x"},"timestamp":"2026-11-05T04:25:24.000Z","message":{"role":"user","content":"Another Claude session sent a message:\n<cross-session-message from=\"uds:x\" from-name=\"a-long-sender-name\">\nHello.\n</cross-session-message>"}}"#;

const BEFORE: &str = "2026-11-05T04:25:00Z";
const AFTER: &str = "2026-11-05T05:00:00Z";

fn rig(lines: &[&str]) -> (Fx, String) {
    let fx = Fx::new();
    fx.write("cfg/projects/c--some-project/sid-1.jsonl", &format!("{}\n", lines.join("\n")));
    let path = fx.path("cfg/projects/c--some-project/sid-1.jsonl").to_string_lossy().into_owned();
    (fx, path)
}

fn at(text: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(text).unwrap().with_timezone(&chrono::Utc)
}

#[test]
fn a_message_record_is_delivered_and_the_enqueue_alone_is_not() {
    let (_fx, held) = rig(&[ENQUEUED_ONLY]);
    assert_eq!(delivered_at(&held, "PING-abc do the", at(BEFORE)), None);
    let (_fx2, read) = rig(&[ENQUEUED_ONLY, DELIVERED]);
    assert_eq!(delivered_at(&read, "PING-abc do the", at(BEFORE)).as_deref(), Some("2026-11-05T04:25:24.000Z"));
}

#[test]
fn a_message_queued_during_a_running_tool_is_delivered_only_when_read() {
    let (_fx, queued) = rig(&[QUEUED_MID_TURN]);
    assert_eq!(delivered_at(&queued, "PING-def come back", at(BEFORE)), None);
    let (_fx2, read) = rig(&[QUEUED_MID_TURN, READ_MID_TURN]);
    assert_eq!(delivered_at(&read, "PING-def come back", at(BEFORE)).as_deref(), Some("2026-11-05T04:26:08.000Z"));
    let (_fx3, gone) = rig(&[QUEUED_MID_TURN, REMOVED_UNREAD]);
    assert_eq!(delivered_at(&gone, "PING-def come back", at(BEFORE)), None, "a removal for another reason was not read");
}

#[test]
fn the_chats_own_prompt_naming_the_phrase_is_not_delivery() {
    let (_fx, p) = rig(&[OWN_PROMPT]);
    assert_eq!(delivered_at(&p, "the exact text PING-own", at(BEFORE)), None);
}

#[test]
fn a_tool_result_holding_the_tag_is_not_delivery() {
    let (_fx, p) = rig(&[TOOL_RESULT]);
    assert_eq!(delivered_at(&p, "PING-abc do the", at(BEFORE)), None);
}

#[test]
fn a_prompt_quoting_the_tag_is_not_delivery() {
    let (_fx, p) = rig(&[PASTED_TAG]);
    assert_eq!(delivered_at(&p, "PING-abc do the", at(BEFORE)), None);
}

#[test]
fn a_phrase_in_the_sender_attributes_is_not_the_message() {
    let (_fx, p) = rig(&[SENDER_NAMED]);
    assert_eq!(delivered_at(&p, "a-long-sender-name", at(BEFORE)), None);
}

#[test]
fn an_empty_or_short_phrase_never_matches() {
    let (_fx, p) = rig(&[DELIVERED]);
    assert_eq!(delivered_at(&p, "", at(BEFORE)), None);
    assert_eq!(delivered_at(&p, "   ", at(BEFORE)), None);
    assert_eq!(delivered_at(&p, "the", at(BEFORE)), None);
    assert_eq!(delivered_at(&p, "PING-abc", at(BEFORE)), None);
}

#[test]
fn a_message_older_than_the_send_time_does_not_count() {
    let (_fx, p) = rig(&[DELIVERED]);
    assert_eq!(delivered_at(&p, "PING-abc do the", at(AFTER)), None);
    assert!(delivered_at(&p, "PING-abc do the", at("2026-11-05T04:25:24Z")).is_some(), "the bound is inclusive");
}

#[test]
fn the_last_permission_mode_wins() {
    let (_fx, p) = rig(&[OWN_PROMPT, DELIVERED]);
    assert_eq!(last_mode(&p).as_deref(), Some("default"));
}

#[test]
fn confirm_runs_end_to_end_with_its_exit_codes() {
    let (fx, _) = rig(&[ENQUEUED_ONLY, DELIVERED]);
    fx.ok(&["session", "register", "life/x", "--session", "sid-1"]);
    let cfg = fx.path("cfg");
    let run = |args: &[&str]| fx.af().env("CLAUDE_CONFIG_DIR", &cfg).args(args).output().unwrap();

    let out = run(&["session", "reach", "life/x", "--confirm", "PING-abc do the", "--since", BEFORE]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("life/x: delivered"), "{out:?}");

    let out = run(&["session", "reach", "life/x", "--confirm", "PING-never arrived", "--since", BEFORE]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stdout).contains("not delivered"), "{out:?}");

    let out = run(&["session", "reach", "life/x", "--confirm", "PING-abc do the", "--since", AFTER]);
    assert_eq!(out.status.code(), Some(3), "an older message: {out:?}");
}

#[test]
fn confirm_refuses_a_short_phrase_a_missing_time_and_a_bad_time_and_names_the_fix() {
    let (fx, _) = rig(&[DELIVERED]);
    fx.ok(&["session", "register", "life/x", "--session", "sid-1"]);
    for phrase in ["", "  ", "the"] {
        let msg = fx.refused(&["session", "reach", "life/x", "--confirm", phrase, "--since", BEFORE]);
        assert!(msg.contains("at least 12 characters"), "{msg}");
    }
    let msg = fx.refused(&["session", "reach", "life/x", "--confirm", "PING-abc do the"]);
    assert!(msg.contains("--since"), "{msg}");
    let msg = fx.refused(&["session", "reach", "life/x", "--confirm", "PING-abc do the", "--since", "yesterday"]);
    assert!(msg.contains("RFC 3339"), "{msg}");
}

#[test]
fn reach_with_no_roster_prints_unknown_and_refuses_an_unregistered_name() {
    let (fx, _) = rig(&[DELIVERED]);
    fx.ok(&["session", "register", "life/x", "--session", "sid-1"]);
    let out = fx
        .af()
        .env("PATH", "")
        .env("CLAUDE_CONFIG_DIR", fx.path("cfg"))
        .args(["session", "reach", "life/x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("life/x: unknown"), "{out:?}");
    let msg = fx.refused(&["session", "reach", "life/nobody"]);
    assert!(msg.contains("no registry row for life/nobody"), "{msg}");
}
