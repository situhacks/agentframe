//! `af session bundle` and the four session hook handlers (library calls and
//! `af hook <event>`).

mod common;
mod life_fx;
use afcore::sessions::{self, HookOutput};
use common::*;
use life_fx::*;

fn main_fixture() -> Fx {
    let fx = Fx::new();
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-300), "live"),
            row("worker/old", "worker", "s-old", &ts(-900), "ended 2026-10-20T10:00 exit"),
        ],
    );
    fx.write("workspace/life/life.md", "---\ncap_lines: 200\n---\n# Life\n\n- lives in the city (2026-10-01)\n");
    fx.write(
        "workspace/life/open.md",
        &format!(
            "# Open loops\n\n## Waiting on the operator\n\n- [ ] {} · renew lease · nudge {} · from: main\n- [ ] {} · call mum · from: main\n",
            days_ago(4),
            days_ago(1),
            days_ago(2)
        ),
    );
    fx.write("system/sessions/main/brief.md", "---\ngoal: run the day\n---\nbrief body\n");
    fx.write("system/sessions/main/handoff.md", &handoff(&ts(-10), &[]));
    fx.write(
        "system/backlog.md",
        "# Backlog\n\n## BB-2026-10-20-01 — one\nwhat\nStatus: open\n\n## BB-2026-10-21-01 — two\nStatus: done\n\n## BB-2026-10-22-01 — three\nStatus: open\n",
    );
    fx
}

fn pos(hay: &str, needle: &str) -> usize {
    hay.find(needle).unwrap_or_else(|| panic!("missing {needle:?} in:\n{hay}"))
}

#[test]
fn bundle_for_main_carries_every_part_in_order() {
    let fx = main_fixture();
    let out = fx.ok(&["session", "bundle", "--session", "s-main"]);
    let order = [
        "af: you are main (main)",
        "- lives in the city (2026-10-01)",
        "open loops: 2 open, 1 due",
        "renew lease",
        "brief (system/sessions/main/brief.md):",
        "brief body",
        "last handoff (system/sessions/main/handoff.md):",
        "## Files touched",
        "sessions: main live, project/acme idle",
        "backlog: 2 open rows",
    ];
    let mut last = 0;
    for n in order {
        let p = pos(&out, n);
        assert!(p >= last, "{n:?} out of order in:\n{out}");
        last = p;
    }
    assert!(!out.contains("worker/old"), "ended rows stay out of the roster");
    assert!(!out.contains("call mum"), "only due loops print");
    assert!(!out.contains("cap_lines"), "life.md frontmatter stays out");
    assert!(!out.contains("index:"));
}

#[test]
fn bundle_prints_the_life_overage_first() {
    let fx = main_fixture();
    let body: String = (0..12).map(|i| format!("- fact {i}\n")).collect();
    fx.write("workspace/life/life.md", &format!("---\ncap_lines: 10\n---\n{body}"));
    let out = fx.ok(&["session", "bundle", "--session", "s-main"]);
    assert!(pos(&out, "life.md is 2 lines over its cap of 10") < pos(&out, "- fact 0"));
}

#[test]
fn bundle_for_an_operator_chat_skips_the_main_parts() {
    let fx = main_fixture();
    fx.write("system/sessions/project/acme/brief.md", "---\ngoal: posts\n---\n");
    let out = fx.ok(&["session", "bundle", "--session", "s-acme"]);
    assert!(out.starts_with("af: you are project/acme (operator)"));
    assert!(out.contains("brief (system/sessions/project/acme/brief.md):"));
    for absent in ["lives in the city", "open loops", "backlog:", "sessions:", "last handoff"] {
        assert!(!out.contains(absent), "{absent} in {out}");
    }
}

fn directive(out: &str) -> &str {
    out.split("\n\n").next().unwrap().trim_end()
}

fn assert_side_and_one_off(d: &str, sid: &str) {
    assert!(d.contains("Before anything else, ask the operator in one line, lettered:"), "{d}");
    assert!(d.contains("(B) make it a side chat (name it life/<topic>, project/<slug>, or system/<topic>;"), "{d}");
    assert!(d.contains(&format!("\"af session register <name> --session {sid}\"")), "{d}");
    assert!(d.contains("write system/sessions/<name>/brief.md, and tell him to send main the name)"), "{d}");
    assert!(d.contains("(C) keep it a one-off (nothing is registered; it must not write loops, life pages, or handoffs)"), "{d}");
    assert!(d.ends_with("If he answers with a task instead of a letter, treat it as (C) and say so in your first sentence."), "{d}");
    assert!(d.split_whitespace().count() < 120, "{} words", d.split_whitespace().count());
}

#[test]
fn bundle_for_an_unregistered_chat_without_a_live_main_offers_main_side_or_one_off() {
    let fx = main_fixture();
    write_registry(&fx, &[row("main", "main", "s-main", &ts(-300), "live")]);
    let out = fx.ok(&["session", "bundle", "--session", "s-new"]);
    let d = directive(&out);
    assert!(d.starts_with("af: this chat is not registered and main is not running."), "{d}");
    assert!(d.contains("lettered: (A) make this chat main, (B) "), "{d}");
    assert_side_and_one_off(d, "s-new");
    assert!(!out.contains("lives in the city") && !out.contains("sessions:"), "{out}");
    let empty = Fx::new();
    assert!(empty.ok(&["session", "bundle", "--session", "s-x"]).starts_with("af: this chat is not registered and main is not running."));
}

#[test]
fn bundle_for_an_unregistered_chat_with_a_live_main_offers_side_or_one_off() {
    let fx = main_fixture();
    let out = fx.ok(&["session", "bundle", "--session", "s-new"]);
    let d = directive(&out);
    let seen = &ts(-5)[11..];
    assert!(
        d.starts_with(&format!("af: this chat is not registered. main is live (main, last seen {seen}); this chat cannot be main.")),
        "{d}"
    );
    assert!(!d.contains("(A)"), "{d}");
    assert!(d.contains("lettered: (B) "), "{d}");
    assert_side_and_one_off(d, "s-new");
}

#[test]
fn bundle_for_a_registered_chat_names_it_with_no_directive() {
    let fx = main_fixture();
    for (sid, who) in [("s-acme", "af: you are project/acme (operator)"), ("s-main", "af: you are main (main)")] {
        let out = fx.ok(&["session", "bundle", "--session", sid]);
        assert!(out.starts_with(who), "{out}");
        assert!(!out.contains("not registered") && !out.contains("Before anything else"), "{out}");
    }
}

#[test]
fn side_names_are_the_grammar_minus_main_and_worker() {
    for name in sessions::SIDE_NAMES {
        assert!(sessions::NAME_GRAMMAR.contains(name), "{name}");
        let concrete = name.replace("<topic>", "x").replace("<slug>", "x");
        assert!(sessions::valid_name(&concrete), "{concrete}");
        assert!(["main", "operator", "builder"].contains(&sessions::role_of(&concrete)), "{concrete}");
    }
}

#[test]
fn bundle_adds_the_index_line_when_the_index_is_old() {
    let fx = main_fixture();
    fx.write("system/index/vault.db", "db");
    set_mtime(&fx, "system/index/vault.db", -3 * 24 * 3600 - 60);
    let out = fx.ok(&["session", "bundle", "--session", "s-acme"]);
    assert!(out.trim_end().ends_with("index: last refresh 3d old; run af index update"), "{out}");
    set_mtime(&fx, "system/index/vault.db", -60);
    assert!(!fx.ok(&["session", "bundle", "--session", "s-acme"]).contains("index:"));
}

// ------------------------------------------------------------------ SessionStart

fn context(o: &HookOutput) -> serde_json::Value {
    assert_eq!(o.exit, 0);
    serde_json::from_str(&o.stdout).unwrap()
}

#[test]
fn session_start_injects_the_bundle_and_names_the_tab() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    let (bundle, _) = sessions::bundle(&ctx, "s-main").unwrap();
    let v = context(&sessions::hook_session_start(&ctx, r#"{"session_id":"s-main","source":"startup"}"#));
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert_eq!(v["hookSpecificOutput"]["sessionTitle"], "main");
    assert_eq!(v["hookSpecificOutput"]["additionalContext"].as_str().unwrap(), bundle);
}

#[test]
fn session_start_after_compaction_reinjects_the_same_bundle() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    let a = context(&sessions::hook_session_start(&ctx, r#"{"session_id":"s-main","source":"startup"}"#));
    let b = context(&sessions::hook_session_start(&ctx, r#"{"session_id":"s-main","source":"compact","hook_event_name":"SessionStart"}"#));
    assert_eq!(a["hookSpecificOutput"]["additionalContext"], b["hookSpecificOutput"]["additionalContext"]);
    assert!(b["hookSpecificOutput"]["additionalContext"].as_str().unwrap().contains("open loops: 2 open, 1 due"));
}

#[test]
fn session_start_stamps_and_revives_the_row() {
    let fx = main_fixture();
    sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-acme","source":"resume"}"#);
    let r = find(&fx, "project/acme");
    assert!(recent(&r.last_seen) && r.status == "live", "{r:?}");
    sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-old","source":"resume"}"#);
    assert_eq!(find(&fx, "worker/old").status, "live");
}

#[test]
fn session_start_for_an_unregistered_chat_has_no_title() {
    let fx = main_fixture();
    let v = context(&sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-new","source":"startup"}"#));
    assert!(v["hookSpecificOutput"].get("sessionTitle").is_none());
    assert!(v["hookSpecificOutput"]["additionalContext"].as_str().unwrap().contains("this chat is not registered"));
    assert_eq!(registry_rows(&fx).len(), 3);
}

#[test]
fn session_start_ignores_a_bad_payload() {
    let fx = main_fixture();
    assert_eq!(sessions::hook_session_start(&fx.ctx(), "not json"), HookOutput::quiet());
    assert_eq!(sessions::hook_session_start(&fx.ctx(), "{}"), HookOutput::quiet());
}

#[test]
fn af_hook_session_start_runs_the_handler() {
    let fx = main_fixture();
    let out = fx.af().args(["hook", "session-start"]).write_stdin(r#"{"session_id":"s-main","source":"compact"}"#).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["hookSpecificOutput"]["sessionTitle"], "main");
    let out = fx.af().args(["hook", "session-start"]).write_stdin(r#"{"session_id":"s-fresh","source":"startup"}"#).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["hookSpecificOutput"].get("sessionTitle").is_none());
    assert!(v["hookSpecificOutput"]["additionalContext"].as_str().unwrap().starts_with("af: this chat is not registered"));
}

#[test]
fn session_start_leads_with_the_refresh_line_in_place_of_the_age_line() {
    let fx = main_fixture();
    fx.write("system/index/vault.db", "db");
    set_mtime(&fx, "system/index/vault.db", -3 * 24 * 3600 - 60);
    let ctx = fx.ctx();
    let v = context(&sessions::hook_session_start_with(&ctx, r#"{"session_id":"s-acme","source":"startup"}"#, Some("REFRESHING")));
    let text = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(text.starts_with("REFRESHING\n\naf: you are project/acme (operator)"), "{text}");
    assert!(!text.contains("index: last refresh"), "{text}");
    assert_eq!(v["hookSpecificOutput"]["sessionTitle"], "project/acme");
    let alone = sessions::hook_session_start_with(&ctx, "{}", Some("REFRESHING"));
    assert_eq!(alone, HookOutput { stdout: "REFRESHING".into(), exit: 0 });
}

// ------------------------------------------------------------------ Stop

#[test]
fn stop_hook_blocks_once_for_a_missing_handoff() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    let o = sessions::hook_stop(&ctx, r#"{"session_id":"s-acme","stop_hook_active":false}"#);
    assert_eq!(o.exit, 0);
    let v: serde_json::Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["decision"], "block");
    assert_eq!(
        v["reason"],
        "Write system/sessions/project/acme/handoff.md (State, Open questions, Next step, Files touched) before stopping."
    );
    assert_eq!(sessions::hook_stop(&ctx, r#"{"session_id":"s-acme","stop_hook_active":true}"#), HookOutput::quiet());
}

#[test]
fn stop_hook_is_quiet_when_the_handoff_is_current_or_the_chat_unregistered() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    fx.write("system/sessions/project/acme/handoff.md", &handoff(&ts(60), &["system/backlog.md"]));
    assert_eq!(sessions::hook_stop(&ctx, r#"{"session_id":"s-acme","stop_hook_active":false}"#), HookOutput::quiet());
    assert!(recent(&find(&fx, "project/acme").last_seen), "the hook stamps last_seen");
    assert_eq!(sessions::hook_stop(&ctx, r#"{"session_id":"s-new"}"#), HookOutput::quiet());
}

#[test]
fn af_hook_stop_runs_the_handler() {
    let fx = main_fixture();
    let out = fx.af().args(["hook", "stop"]).write_stdin(r#"{"session_id":"s-acme","stop_hook_active":false}"#).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"decision\":\"block\""));
}

#[test]
fn stop_hook_never_blocks_main_and_still_stamps_it() {
    let fx = main_fixture();
    fx.write("system/sessions/main/handoff.md", &handoff(&ts(-10), &["system/backlog.md"]));
    let (code, out, _) = fx.run(&["session", "stop", "main"]);
    assert_eq!((code, out.trim()), (3, "HANDOFF_REQUIRED system/sessions/main/handoff.md"), "the verb still reports it");
    let o = sessions::hook_stop(&fx.ctx(), r#"{"session_id":"s-main","stop_hook_active":false}"#);
    assert_eq!(o, HookOutput::quiet());
    assert!(recent(&find(&fx, "main").last_seen));
}

#[test]
fn a_handoff_signal_is_not_a_button_refusal() {
    let fx = main_fixture();
    assert_eq!(fx.run(&["session", "stop", "project/acme"]).0, 3);
    assert!(afcore::runlog::button_notes(&fx.ctx()).is_empty(), "exit 3 from session stop is not counted");
    assert_eq!(fx.run(&["session", "end", "nope", "--reason", "exit"]).0, 1);
    let notes = afcore::runlog::button_notes(&fx.ctx());
    assert!(notes.len() == 1 && notes[0].contains("1 refusal(s): session 1"), "{notes:?}");
}

// ------------------------------------------------------------------ SessionEnd

#[test]
fn session_end_hook_maps_the_reason_and_never_blocks() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    assert_eq!(sessions::hook_session_end(&ctx, r#"{"session_id":"s-acme","reason":"prompt_input_exit"}"#), HookOutput::quiet());
    assert!(find(&fx, "project/acme").status.ends_with(" exit"));
    sessions::hook_session_end(&ctx, r#"{"session_id":"s-main","reason":"bypass_permissions_disabled"}"#);
    assert!(find(&fx, "main").status.ends_with(" other"));
    assert_eq!(sessions::end_reason("clear"), "clear");
    assert_eq!(sessions::end_reason("resume"), "resume");
    assert_eq!(sessions::end_reason("logout"), "logout");
}

#[test]
fn session_end_hook_for_an_unregistered_chat_writes_nothing() {
    let fx = Fx::new();
    let o = sessions::hook_session_end(&fx.ctx(), r#"{"session_id":"s-x","reason":"exit"}"#);
    assert_eq!(o, HookOutput::quiet());
    assert!(!fx.exists("system/sessions/registry.md"));
    let out = fx.af().args(["hook", "session-end"]).write_stdin("{}").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

// ------------------------------------------------------------------ Notification

#[test]
fn notification_stamps_a_registered_background_session() {
    let fx = main_fixture();
    let ctx = fx.ctx();
    let o = sessions::hook_notification(&ctx, r#"{"session_id":"s-acme","notification_type":"idle_prompt"}"#);
    assert_eq!(o, HookOutput::quiet());
    assert_eq!(find(&fx, "project/acme").last_seen, ts(-300), "other notification types change nothing");
    sessions::hook_notification(&ctx, r#"{"session_id":"s-acme","notification_type":"agent_completed"}"#);
    let r = find(&fx, "project/acme");
    assert!(recent(&r.last_seen) && r.status == "live", "{r:?}");
    sessions::hook_notification(&ctx, r#"{"session_id":"s-old","notification_type":"agent_needs_input"}"#);
    let r = find(&fx, "worker/old");
    assert!(recent(&r.last_seen));
    assert!(r.is_ended(), "a notification does not revive an ended row");
    let out = fx
        .af()
        .args(["hook", "notification"])
        .write_stdin(r#"{"session_id":"s-main","notification_type":"agent_completed"}"#)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}
