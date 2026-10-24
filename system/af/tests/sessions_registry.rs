//! `af session register | list | stop | end`: the registry round trip, the handoff check,
//! and the roster join.

mod common;
mod life_fx;
use afcore::sessions::{self, join_roster, Row};
use common::*;
use life_fx::*;

#[test]
fn register_writes_row_folder_and_derives_role() {
    let fx = Fx::new();
    let out = fx.ok(&["session", "register", "main", "--session", "s-main", "--model", "opus"]);
    assert!(out.starts_with("| main | main | s-main | opus | "), "{out}");
    assert!(fx.path("system/sessions/main").is_dir());
    let text = fx.read("system/sessions/registry.md");
    assert!(text.starts_with(HEADER), "{text}");
    let r = find(&fx, "main");
    assert_eq!(r.status, "live");
    assert_eq!(r.handoff, "system/sessions/main/handoff.md");
    assert_eq!(r.started, r.last_seen);
    for (name, role) in [("life/money", "main"), ("project/acme", "operator"), ("system/cut", "builder"), ("worker/w1", "worker")] {
        fx.ok(&["session", "register", name, "--session", &format!("s-{role}")]);
        assert_eq!(find(&fx, name).role, role);
    }
    assert_eq!(find(&fx, "worker/w1").model, "-");
    assert_eq!(registry_rows(&fx).len(), 5);
}

#[test]
fn register_refuses_names_outside_the_grammar() {
    let fx = Fx::new();
    for bad in ["bad/name", "project/", "project/a/b", "Main", "main/x", "worker/.hidden", "project/a|b"] {
        let err = fx.refused(&["session", "register", bad, "--session", "s1"]);
        assert!(err.contains("outside the grammar"), "{bad}: {err}");
    }
    assert!(!fx.exists("system/sessions/registry.md"));
    fx.refused(&["session", "register", "main", "--session", "a|b"]);
}

#[test]
fn reregister_replaces_the_row_and_keeps_the_folder() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "project/acme", "--session", "s1"]);
    fx.write("system/sessions/project/acme/handoff.md", "kept");
    fx.ok(&["session", "register", "project/acme", "--session", "s2", "--model", "sonnet"]);
    let rows = registry_rows(&fx);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "s2");
    assert_eq!(rows[0].model, "sonnet");
    assert_eq!(fx.read("system/sessions/project/acme/handoff.md"), "kept");
}

#[test]
fn register_copies_the_brief() {
    let fx = Fx::new();
    fx.write("drafts/b.md", "---\ngoal: ship\n---\nbody\n");
    fx.ok(&["session", "register", "system/cut", "--session", "s1", "--brief", "drafts/b.md"]);
    assert_eq!(fx.read("system/sessions/system/cut/brief.md"), "---\ngoal: ship\n---\nbody\n");
    fx.refused(&["session", "register", "system/cut", "--session", "s1", "--brief", "drafts/missing.md"]);
}

#[test]
fn register_leaves_other_lines_and_crlf_untouched() {
    let fx = Fx::new();
    let before = format!(
        "Registry note.\r\n\r\n{}{}",
        HEADER.replace('\n', "\r\n"),
        row("main", "main", "s-main", "2026-10-20T09:00", "live").replace('\n', "\r\n")
    );
    fx.write("system/sessions/registry.md", &before);
    fx.ok(&["session", "register", "project/acme", "--session", "s2"]);
    let raw = std::fs::read_to_string(fx.path("system/sessions/registry.md")).unwrap();
    assert!(raw.starts_with(&before), "{raw}");
    assert!(raw.ends_with("system/sessions/project/acme/handoff.md |\r\n"));
}

#[test]
fn list_turns_stale_live_rows_idle_and_keeps_fresh_ones_live() {
    let fx = Fx::new();
    write_registry(
        &fx,
        &[
            row("main", "main", "s1", &ts(-10), "live"),
            row("project/old", "operator", "s2", &ts(-180), "live"),
            row("worker/done", "worker", "s3", &ts(-600), "ended 2026-10-20T10:00 exit"),
        ],
    );
    let out = fx.ok(&["session", "list"]);
    assert!(out.contains("| project/old |"));
    assert_eq!(find(&fx, "main").status, "live");
    assert_eq!(find(&fx, "project/old").status, "idle");
    assert_eq!(find(&fx, "worker/done").status, "ended 2026-10-20T10:00 exit");
}

#[test]
fn list_without_the_roster_says_so_and_continues() {
    let fx = Fx::new();
    write_registry(&fx, &[row("main", "main", "s1", &ts(-1), "live")]);
    let out = fx.af().env("PATH", "").args(["session", "list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("| main | main | s1 |"));
    assert!(stdout.contains("roster unavailable"), "{stdout}");
    assert!(!stdout.contains("live_state"));

    let out = fx.af().env("PATH", "").args(["session", "list", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["name"], "main");
    assert!(v[0].get("live_state").is_none());
    assert!(String::from_utf8_lossy(&out.stderr).contains("roster unavailable"));
}

#[test]
fn roster_join_adds_live_state_by_session_id() {
    let rows: Vec<Row> = [("main", "s1"), ("worker/a", "s2")]
        .iter()
        .map(|(n, s)| Row::parse(&row(n, sessions::role_of(n), s, "2026-10-24T09:00", "live")).unwrap())
        .collect();
    let roster = serde_json::json!([{"sessionId": "s1", "status": "busy", "name": "x"}, {"sessionId": "zz", "status": "idle"}]);
    assert_eq!(join_roster(&rows, &roster), vec!["busy".to_string(), "absent".to_string()]);
}

#[test]
fn stop_is_silent_for_an_unregistered_session() {
    let fx = Fx::new();
    let (code, out, _) = fx.run(&["session", "stop", "--session", "nobody"]);
    assert_eq!((code, out.as_str()), (0, ""));
    let (code, out, _) = fx.run(&["session", "stop", "project/none"]);
    assert_eq!((code, out.as_str()), (0, ""));
}

#[test]
fn stop_requires_a_missing_handoff_and_stamps_last_seen() {
    let fx = Fx::new();
    write_registry(&fx, &[row("project/acme", "operator", "s1", "2026-10-01T09:00", "idle")]);
    let (code, out, _) = fx.run(&["session", "stop", "--session", "s1"]);
    assert_eq!(code, 3);
    assert_eq!(out.trim(), "HANDOFF_REQUIRED system/sessions/project/acme/handoff.md");
    let r = find(&fx, "project/acme");
    assert_ne!(r.last_seen, "2026-10-01T09:00");
    assert_eq!(r.status, "live");
}

#[test]
fn stop_compares_written_with_the_files_touched() {
    let fx = Fx::new();
    write_registry(&fx, &[row("project/acme", "operator", "s1", &ts(-5), "live")]);
    fx.write("workspace/projects/acme/draft-v1.md", "x");
    let h = "system/sessions/project/acme/handoff.md";

    fx.write(h, &handoff(&ts(24 * 60), &["workspace/projects/acme/draft-v1.md"]));
    let (code, out, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!((code, out.as_str()), (0, ""), "a fresh handoff needs nothing");

    fx.write(h, &handoff("2026-01-01T00:00", &["workspace/projects/acme/draft-v1.md"]));
    let (code, out, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!(code, 3);
    assert!(out.contains("HANDOFF_REQUIRED system/sessions/project/acme/handoff.md"));

    // The same minute counts as covered: written has minute precision.
    fx.write(h, &handoff("2026-06-01T10:00", &["workspace/projects/acme/draft-v1.md"]));
    set_mtime_at(&fx, "workspace/projects/acme/draft-v1.md", "2026-06-01T10:00:45");
    let (code, _, _) = fx.run(&["session", "stop", "project/acme"]);
    assert_eq!(code, 0);
}

#[test]
fn stop_falls_back_to_the_brief_write_surface() {
    let fx = Fx::new();
    write_registry(&fx, &[row("system/cut", "builder", "s1", &ts(-5), "live")]);
    fx.write("system/sessions/system/cut/brief.md", "---\ngoal: cut\nwrite_surface: [library/process]\n---\n");
    fx.write("library/process/a.md", "x");
    fx.write("system/sessions/system/cut/handoff.md", &handoff("2026-06-01T00:00", &[]));
    let (code, _, _) = fx.run(&["session", "stop", "system/cut"]);
    assert_eq!(code, 3, "a write under the surface after the handoff");
    set_mtime(&fx, "library/process/a.md", -400 * 24 * 3600);
    let (code, _, _) = fx.run(&["session", "stop", "system/cut"]);
    assert_eq!(code, 0, "nothing under the surface moved since the handoff");
}

#[test]
fn stop_never_requires_a_handoff_from_an_ended_row() {
    let fx = Fx::new();
    write_registry(&fx, &[row("worker/w", "worker", "s1", &ts(-5), "ended 2026-10-24T08:00 exit")]);
    let (code, out, _) = fx.run(&["session", "stop", "worker/w"]);
    assert_eq!((code, out.as_str()), (0, ""));
}

#[test]
fn end_stamps_the_status_and_logs_workers_and_builders_only() {
    let fx = Fx::new();
    write_registry(
        &fx,
        &[row("worker/w", "worker", "s1", &ts(-5), "live"), row("project/acme", "operator", "s2", &ts(-5), "live")],
    );
    let out = fx.ok(&["session", "end", "worker/w", "--reason", "clear"]);
    assert!(out.contains("| worker/w |"));
    let st = find(&fx, "worker/w").status;
    assert!(st.starts_with("ended ") && st.ends_with(" clear"), "{st}");
    let log = fx.read(&format!("workspace/life/log/{}.md", month()));
    assert!(log.contains(" · session_ended: worker/w\n"), "{log}");
    assert!(regex::Regex::new(r"(?m)^- \d{4}-\d{2}-\d{2} \d{2}:\d{2} · session_ended: worker/w$").unwrap().is_match(&log));

    fx.ok(&["session", "end", "--session", "s2", "--reason", "exit"]);
    assert!(find(&fx, "project/acme").status.ends_with(" exit"));
    assert_eq!(fx.read(&format!("workspace/life/log/{}.md", month())).lines().count(), 1);
}

#[test]
fn end_refuses_an_unknown_row_and_a_bad_reason() {
    let fx = Fx::new();
    write_registry(&fx, &[row("main", "main", "s1", &ts(-5), "live")]);
    fx.refused(&["session", "end", "project/none", "--reason", "exit"]);
    let (code, _, _) = fx.run(&["session", "end", "main", "--reason", "bored"]);
    assert_eq!(code, 2);
    fx.refused(&["session", "stop"]);
}

#[test]
fn verbs_write_run_log_rows() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "main", "--session", "s1"]);
    fx.run(&["session", "stop", "main"]);
    let rows = fx.run_log_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["verb"], "session");
    assert_eq!(rows[1]["exit"], 3);
}
