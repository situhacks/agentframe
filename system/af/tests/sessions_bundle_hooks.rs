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
fn bundle_for_an_operator_chat_prints_the_loops_and_skips_the_rest_of_the_main_parts() {
    let fx = main_fixture();
    fx.write("system/sessions/project/acme/brief.md", "---\ngoal: posts\n---\n");
    let out = fx.ok(&["session", "bundle", "--session", "s-acme"]);
    assert!(out.starts_with("af: you are project/acme (operator)"));
    assert!(out.contains("brief (system/sessions/project/acme/brief.md):"));
    assert!(out.contains("open loops: 2 open, 1 due"), "the loop rule covers every interactive chat: {out}");
    for absent in ["lives in the city", "backlog:", "sessions:", "last handoff"] {
        assert!(!out.contains(absent), "{absent} in {out}");
    }
}

#[test]
fn bundle_for_a_worker_leaves_the_loops_out() {
    let fx = main_fixture();
    fx.ok(&["session", "register", "worker/job", "--session", "s-job"]);
    let out = fx.ok(&["session", "bundle", "--session", "s-job"]);
    assert!(out.starts_with("af: you are worker/job (worker)"));
    assert!(!out.contains("open loops"), "{out}");
}

fn directive(out: &str) -> &str {
    out.split("\n\n").next().unwrap().trim_end()
}

fn assert_side_and_one_off(d: &str, sid: &str) {
    assert!(d.contains("Before anything else, ask the operator in one line, lettered:"), "{d}");
    assert!(d.contains("(B) make it a side chat (name it life/<topic>, project/<slug>, or system/<topic>;"), "{d}");
    assert!(d.contains(&format!("\"af session register <name> --session {sid}\"")), "{d}");
    assert!(d.contains("write system/sessions/<name>/brief.md, and tell him to send main the name)"), "{d}");
    assert!(d.contains("(C) keep it a one-off (nothing is registered; a loop, a life fact, or a handoff it turns up goes to af-main)"), "{d}");
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
    assert!(d.contains("lettered: (A) make this chat main (rename it af-main with /rename af-main and register it with \"af session register af-main --session s-new\"), (B) "), "{d}");
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
    // A build chat: an explicit transcript keeps the card's lookup off the real `~/.claude`.
    let transcript = fx.write("t/s-acme.jsonl", "{\"type\":\"user\"}\n");
    sessions::hook_session_start(&fx.ctx(), &serde_json::json!({"session_id": "s-acme", "source": "resume", "transcript_path": transcript}).to_string());
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
fn an_unregistered_chat_is_asked_once_then_only_reminded_after_compaction_or_resume() {
    let fx = main_fixture();
    for source in ["startup", "clear"] {
        let v = context(&sessions::hook_session_start(&fx.ctx(), &format!(r#"{{"session_id":"s-new","source":"{source}"}}"#)));
        let ctx = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(ctx.starts_with("af: this chat is not registered") && ctx.contains("Before anything else"), "{source}: {ctx}");
    }
    for source in ["compact", "resume"] {
        let v = context(&sessions::hook_session_start(&fx.ctx(), &format!(r#"{{"session_id":"s-new","source":"{source}"}}"#)));
        let ctx = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(ctx.starts_with(sessions::OFF_SYSTEM_REMINDER), "{source}: {ctx}");
        assert!(!ctx.contains("(A)") && !ctx.contains("Before anything else"), "{source}: {ctx}");
    }
    let v = context(&sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-main","source":"compact"}"#));
    assert!(v["hookSpecificOutput"]["additionalContext"].as_str().unwrap().starts_with("af: you are main (main)"));
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
    let reason = v["reason"].as_str().unwrap();
    assert!(reason.starts_with("Write system/sessions/project/acme/handoff.md before stopping: the file does not exist."), "{reason}");
    assert!(reason.contains("State, Next step and Files touched"), "{reason}");
    assert!(reason.contains(&format!("`written: {}", &ts(0)[..11])), "the current local time fills in the shape: {reason}");
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
    assert_eq!(code, 3, "the verb still reports it");
    assert!(out.starts_with("HANDOFF_REQUIRED system/sessions/main/handoff.md ("), "{out}");
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

// ------------------------------------------------------------------ the main chat's name

#[test]
fn the_main_chat_resolves_by_role_under_af_main_and_the_legacy_main_name() {
    for name in [sessions::MAIN_NAME, sessions::LEGACY_MAIN_NAME] {
        let fx = Fx::new();
        write_registry(
            &fx,
            &[row("life/fitness", "main", "s-life", &ts(-1), "live"), row(name, "main", "s-main", &ts(-5), "live")],
        );
        let reg = sessions::Registry::load(&fx.ctx()).unwrap();
        let m = sessions::live_main(&reg, afcore::life::now()).unwrap_or_else(|| panic!("{name} is not the live main"));
        assert_eq!(m.name, name);
        let d = fx.ok(&["session", "bundle", "--session", "s-new"]);
        assert!(directive(&d).contains(&format!("main is live ({name}, last seen ")), "{d}");
        let own = fx.ok(&["session", "bundle", "--session", "s-main"]);
        assert!(own.starts_with(&format!("af: you are {name} (main)")), "{own}");
        assert_eq!(
            own.contains(
                "Rename this chat to af-main (VS Code: /rename af-main), then run \"af session register af-main --session s-main\" and \"af session end main --reason other\"."
            ),
            name == sessions::LEGACY_MAIN_NAME,
            "{own}"
        );
    }
}

#[test]
fn a_life_chat_alone_is_not_the_live_main() {
    let fx = Fx::new();
    write_registry(&fx, &[row("life/fitness", "main", "s-life", &ts(-1), "live")]);
    let reg = sessions::Registry::load(&fx.ctx()).unwrap();
    assert!(sessions::live_main(&reg, afcore::life::now()).is_none());
    let d = fx.ok(&["session", "bundle", "--session", "s-new"]);
    assert!(directive(&d).contains("(A) make this chat main (rename it af-main with /rename af-main and register it with \"af session register af-main --session s-new\""), "{d}");
}

// A handoff that is required says which of the three cases applies and how to fix it.

fn stop_reason(fx: &Fx) -> String {
    let o = sessions::hook_stop(&fx.ctx(), r#"{"session_id":"s-acme","stop_hook_active":false}"#);
    let v: serde_json::Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["decision"], "block");
    v["reason"].as_str().unwrap().to_string()
}

const HANDOFF: &str = "system/sessions/project/acme/handoff.md";

#[test]
fn the_stop_reason_for_a_handoff_with_no_written_line_gives_the_shape() {
    let fx = main_fixture();
    fx.write(HANDOFF, "## State\nwork\n## Open questions\n## Next step\n## Files touched\n");
    let reason = stop_reason(&fx);
    assert!(reason.starts_with("Fix system/sessions/project/acme/handoff.md before stopping: it has no `written:` line"), "{reason}");
    assert!(reason.contains(&format!("`written: {}", &ts(0)[..11])) && reason.contains("YYYY-MM-DDTHH:MM"), "{reason}");
    let (code, out, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!(code, 3);
    assert!(out.starts_with(&format!("HANDOFF_REQUIRED {HANDOFF} (no `written:` line; add written: {}", &ts(0)[..11])), "{out}");
    // An unparseable stamp is the same case.
    fx.write(HANDOFF, "---\nwritten: yesterday\n---\n## State\n");
    assert!(stop_reason(&fx).contains("no `written:` line that parses"));
}

#[test]
fn the_stop_reason_for_a_stale_handoff_names_the_newest_listed_path_and_its_time() {
    let fx = main_fixture();
    fx.write("workspace/projects/acme/old.md", "x");
    fx.write("workspace/projects/acme/draft-v1.md", "x");
    set_mtime_at(&fx, "workspace/projects/acme/old.md", "2026-05-01T08:00:30");
    set_mtime_at(&fx, "workspace/projects/acme/draft-v1.md", "2026-06-01T10:30:45");
    fx.write(HANDOFF, &handoff("2026-06-01T10:00", &["workspace/projects/acme/old.md", "workspace/projects/acme/draft-v1.md"]));
    let reason = stop_reason(&fx);
    assert!(reason.starts_with("Refresh system/sessions/project/acme/handoff.md before stopping: workspace/projects/acme/draft-v1.md changed at 2026-06-01T10:30, after its `written: 2026-06-01T10:00`."), "{reason}");
    assert!(reason.contains("set `written:` to the current time"), "{reason}");
    let (code, out, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!(code, 3);
    assert!(
        out.starts_with(&format!("HANDOFF_REQUIRED {HANDOFF} (stale: workspace/projects/acme/draft-v1.md changed at 2026-06-01T10:30, after written: 2026-06-01T10:00;")),
        "{out}"
    );
}

#[test]
fn the_verb_names_a_missing_file() {
    let fx = main_fixture();
    let (code, out, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!((code, out.trim()), (3, format!("HANDOFF_REQUIRED {HANDOFF} (file missing)").as_str()));
}

// ------------------------------------------------------------------ the model card of a build chat

fn with_cards(fx: &Fx) {
    for a in ["fable", "opus", "sonnet", "haiku"] {
        fx.write(&format!("library/models/{a}.md"), &format!("# Model card: {a}\n\nchecked: 2026-11-14\n\n- the {a} line\n"));
    }
}

/// The additionalContext the session-start handler gives for a hook input. The handler runs in this
/// process, where the subprocess-only `CLAUDE_CONFIG_DIR` does not reach, so an input with no
/// `transcript_path` gets one for an empty transcript: the lookup by session id never lists `~/.claude`.
fn start_text(fx: &Fx, mut input: serde_json::Value) -> String {
    if input.get("transcript_path").is_none() {
        input["transcript_path"] = serde_json::json!(fx.write("t/no-reply-yet.jsonl", "{\"type\":\"user\"}\n"));
    }
    let o = sessions::hook_session_start(&fx.ctx(), &input.to_string());
    context(&o)["hookSpecificOutput"]["additionalContext"].as_str().unwrap().to_string()
}

fn card_fixture() -> Fx {
    let fx = main_fixture();
    with_cards(&fx);
    fx
}

#[test]
fn a_build_chat_whose_hook_input_names_a_model_gets_that_models_card() {
    let fx = card_fixture();
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": "claude-fable-5-2"}));
    assert!(text.contains("model card (fable):\n# Model card: fable\n\nchecked: 2026-11-14\n\n- the fable line"), "{text}");
    assert!(!text.contains("the opus line"), "the registry row says opus, the input wins: {text}");
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "compact", "model": "claude-opus-5-5[1m]"}));
    assert!(text.contains("model card (opus):") && text.contains("- the opus line"), "{text}");
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": "haiku"}));
    assert!(text.contains("model card (haiku):"), "an alias maps to itself: {text}");
}

#[test]
fn without_a_model_in_the_input_the_transcripts_last_reply_decides_then_the_registry_row() {
    let fx = card_fixture();
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-300), "live").replace("| opus |", "| sonnet |"),
        ],
    );
    let line = |model: &str| format!("{{\"type\":\"assistant\",\"message\":{{\"model\":\"{model}\",\"usage\":{{\"input_tokens\":5}}}}}}\n");
    let transcript = fx.write("t/s-acme.jsonl", &format!("{}{}", line("claude-fable-5-2"), line("claude-opus-5-5")));
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "resume", "transcript_path": transcript}));
    assert!(text.contains("model card (opus):") && !text.contains("the sonnet line"), "{text}");
    // No transcript and no model in the input: the row's model.
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup"}));
    assert!(text.contains("model card (sonnet):"), "{text}");
    // `af session bundle` reads the same chain.
    let out = fx.ok(&["session", "bundle", "--session", "s-acme"]).replace("\r\n", "\n");
    assert!(out.contains("model card (sonnet):\n# Model card: sonnet"), "{out}");
}

#[test]
fn a_main_chat_a_side_chat_and_a_worker_get_no_card() {
    let fx = card_fixture();
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("life/trips", "main", "s-life", &ts(-5), "live"),
            row("worker/job", "worker", "s-job", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-5), "live"),
            row("system/cut", "builder", "s-cut", &ts(-5), "live"),
        ],
    );
    fx.write("system/sessions/project/acme/brief.md", "---\ngoal: x\nclass: side\n---\n");
    fx.write("system/sessions/worker/job/brief.md", "---\ngoal: x\nclass: build\n---\n");
    for id in ["s-main", "s-life", "s-job", "s-acme"] {
        let text = start_text(&fx, serde_json::json!({"session_id": id, "source": "startup", "model": "claude-fable-5-2"}));
        assert!(!text.contains("model card"), "{id}: {text}");
    }
    let text = start_text(&fx, serde_json::json!({"session_id": "s-cut", "source": "startup", "model": "claude-fable-5-2"}));
    assert!(text.contains("model card (fable):"), "a builder chat is a build chat: {text}");
    let text = start_text(&fx, serde_json::json!({"session_id": "s-unregistered", "source": "startup", "model": "claude-fable-5-2"}));
    assert!(!text.contains("model card"), "{text}");
}

#[test]
fn an_unknown_model_or_a_missing_card_adds_nothing_and_does_not_fail() {
    let fx = card_fixture();
    let plain = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": ""}));
    assert!(plain.contains("model card (opus):"), "an empty model is not there: {plain}");
    // No source maps to an alias: the hook input, no transcript reply, and a registry row with no model.
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-300), "live").replace("| opus |", "| - |"),
        ],
    );
    for model in ["gemini-4-hydrogen-high", "claude-mythical-9-9", "claude-", "default", "opusplan"] {
        let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": model}));
        assert!(!text.contains("model card"), "{model}: {text}");
        assert!(text.starts_with("af: you are project/acme (operator)"), "{text}");
    }
    fx.remove("library/models/fable.md");
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": "claude-fable-5-2"}));
    assert!(!text.contains("model card") && text.contains("open loops"), "{text}");
    let out = fx.af().args(["hook", "session-start"]).write_stdin(r#"{"session_id":"s-acme","source":"startup","model":"claude-nothing-1-1"}"#).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn af_hook_session_start_adds_the_card_and_it_follows_the_handoff() {
    let fx = card_fixture();
    fx.write("system/sessions/project/acme/handoff.md", &handoff(&ts(-10), &[]));
    let out = fx.af().args(["hook", "session-start"]).write_stdin(r#"{"session_id":"s-acme","source":"startup","model":"claude-sonnet-5-5"}"#).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let text = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(pos(text, "last handoff (") < pos(text, "model card (sonnet):"), "{text}");
}

#[test]
fn model_names_map_to_aliases_by_prefix() {
    use afcore::model_card::alias_of;
    for (m, want) in [
        ("claude-fable-5-2", Some("fable")),
        ("claude-opus-5-5[1m]", Some("opus")),
        ("Claude-Sonnet-5-5", Some("sonnet")),
        ("haiku", Some("haiku")),
        ("claude-haiku-latest", Some("haiku")),
        ("claude-opusx-1-1", None),
        ("opus-5", None),
        ("gemini-4-hydrogen-medium", None),
        ("", None),
    ] {
        assert_eq!(alias_of(m), want, "{m}");
    }
}

#[test]
fn a_value_that_maps_to_no_alias_passes_the_search_on_to_the_next_source() {
    let fx = card_fixture();
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-300), "live").replace("| opus |", "| sonnet |"),
        ],
    );
    let line = |model: &str| format!("{{\"type\":\"assistant\",\"message\":{{\"model\":\"{model}\",\"usage\":{{\"input_tokens\":5}}}}}}\n");
    let opus_chat = fx.write("t/opus.jsonl", &line("claude-opus-5-5"));
    for hook_model in ["default", "opusplan", "claude-mythical-9-9"] {
        let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": hook_model, "transcript_path": opus_chat}));
        assert!(text.contains("model card (opus):") && !text.contains("the sonnet line"), "{hook_model}: {text}");
    }
    // The transcript's own unknown model passes the search to the registry row.
    let odd_chat = fx.write("t/odd.jsonl", &line("claude-mythical-9-9"));
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": "default", "transcript_path": odd_chat}));
    assert!(text.contains("model card (sonnet):"), "{text}");
}

#[test]
fn claude_codes_best_alias_is_opus() {
    let fx = card_fixture();
    write_registry(
        &fx,
        &[
            row("main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-300), "live").replace("| opus |", "| sonnet |"),
        ],
    );
    let text = start_text(&fx, serde_json::json!({"session_id": "s-acme", "source": "startup", "model": "best"}));
    assert!(text.contains("model card (opus):") && !text.contains("the sonnet line"), "{text}");
    assert_eq!(afcore::model_card::alias_of("best"), Some("opus"));
    assert_eq!(afcore::model_card::alias_of("default"), None);
    assert_eq!(afcore::model_card::alias_of("opusplan"), None);
}

#[test]
fn the_display_name_has_the_right_article() {
    use afcore::model_card::display_with_article;
    assert_eq!(display_with_article("opus"), "an Opus");
    assert_eq!(display_with_article("fable"), "a Fable");
    assert_eq!(display_with_article("sonnet"), "a Sonnet");
    assert_eq!(display_with_article("haiku"), "a Haiku");
}
