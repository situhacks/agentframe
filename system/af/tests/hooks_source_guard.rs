//! The source guard on Write: a new file under workspace/life/sources/ is denied.

mod common;

use afcore::hooks::{dispatch, source_guard, HookCtx};
use afcore::pyjson::PyVal;
use common::Fx;
use serde_json::{json, Value};

fn payload(tool: &str, path: &str, cwd: &str) -> Value {
    json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {"file_path": path}, "cwd": cwd})
}

fn reason(v: &Option<PyVal>) -> (String, String) {
    let s = afcore::pyjson::dumps(v.as_ref().expect("expected a decision"));
    let j: Value = serde_json::from_str(&s).unwrap();
    let o = &j["hookSpecificOutput"];
    (o["permissionDecision"].as_str().unwrap_or("").to_string(), o["permissionDecisionReason"].as_str().unwrap_or("").to_string())
}

struct G {
    fx: Fx,
    h: HookCtx,
}

impl G {
    fn new() -> G {
        let fx = Fx::new();
        fx.write("workspace/life/sources/INDEX.md", "# index\n");
        fx.write("workspace/life/sources/orders/src-2026-11-15-08.md", "---\nid: src-2026-11-15-08\n---\nbody\n");
        let h = HookCtx::new(&fx.root());
        G { fx, h }
    }

    fn decide(&self, tool: &str, path: &str) -> Option<PyVal> {
        source_guard::decide(&self.h, &payload(tool, path, &self.h.root))
    }
}

#[test]
fn test_new_source_file_is_denied_with_the_fix_named() {
    let g = G::new();
    let (d, r) = reason(&g.decide("Write", &g.fx.p("workspace/life/sources/orders/src-2026-11-15-09.md")));
    assert_eq!(d, "deny");
    assert!(r.contains("af source add"));
    assert!(r.contains("--body-file -"));
}

#[test]
fn test_existing_source_file_and_index_stay_allowed() {
    let g = G::new();
    assert!(g.decide("Write", &g.fx.p("workspace/life/sources/orders/src-2026-11-15-08.md")).is_none());
    assert!(g.decide("Write", &g.fx.p("workspace/life/sources/INDEX.md")).is_none());
    assert!(g.decide("Edit", &g.fx.p("workspace/life/sources/orders/src-2026-11-15-08.md")).is_none());
}

#[test]
fn test_a_path_outside_sources_is_allowed() {
    let g = G::new();
    assert!(g.decide("Write", &g.fx.p("workspace/life/knowledge/new-topic.md")).is_none());
    assert!(g.decide("Write", &g.fx.p("workspace/life/sources-extra/x.md")).is_none());
    assert!(g.decide("Write", &g.fx.p("system/sources/x.md")).is_none());
    assert!(g.decide("Write", &g.fx.p("workspace/life/other/../knowledge/x.md")).is_none());
}

#[test]
fn test_edit_of_a_missing_file_is_not_this_guards_to_deny() {
    let g = G::new();
    assert!(g.decide("Edit", &g.fx.p("workspace/life/sources/orders/src-2026-11-15-09.md")).is_none());
}

#[test]
fn test_forward_and_back_slash_and_dotdot_paths_are_denied() {
    let g = G::new();
    let bs = char::from(92u8).to_string();
    let root = g.h.root.clone();
    let fwd = format!("{}/workspace/life/sources/orders/new.md", root.replace(&bs, "/"));
    assert_eq!(reason(&g.decide("Write", &fwd)).0, "deny");
    if cfg!(windows) {
        let back = format!(r"{}\workspace\life\sources\orders\new.md", root.replace('/', &bs));
        assert_eq!(reason(&g.decide("Write", &back)).0, "deny");
    }
    let dots = format!("{}/workspace/life/knowledge/../sources/orders/new.md", root.replace(&bs, "/"));
    assert_eq!(reason(&g.decide("Write", &dots)).0, "deny");
}

#[test]
fn test_relative_path_resolves_against_the_cwd_and_the_project_dir() {
    let g = G::new();
    let rel = payload("Write", "workspace/life/sources/orders/new.md", &g.h.root);
    assert_eq!(reason(&source_guard::decide(&g.h, &rel)).0, "deny");
    let from_orders = payload("Write", "new.md", &g.fx.p("workspace/life/sources/orders"));
    assert_eq!(reason(&source_guard::decide(&g.h, &from_orders)).0, "deny");
    let elsewhere = payload("Write", "new.md", &g.fx.p("workspace/life/knowledge"));
    assert!(source_guard::decide(&g.h, &elsewhere).is_none());
    let no_cwd = json!({"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"file_path": "workspace/life/sources/orders/new.md"}});
    assert_eq!(reason(&source_guard::decide(&g.h, &no_cwd)).0, "deny", "no cwd: the project dir");
}

#[cfg(windows)]
#[test]
fn test_drive_letter_case_does_not_matter_on_windows() {
    let g = G::new();
    let p = g.fx.p("workspace/life/sources/orders/new.md");
    let flipped: String = p
        .chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() } } else { c })
        .collect();
    assert_ne!(p, flipped);
    assert_eq!(reason(&g.decide("Write", &flipped)).0, "deny");
}

#[test]
fn test_dispatch_denies_a_write_and_nothing_is_written() {
    let g = G::new();
    let target = g.fx.p("workspace/life/sources/orders/src-2026-11-15-10.md");
    let out = dispatch(&g.h, "pre-tool-use", &payload("Write", &target, &g.h.root).to_string()).unwrap();
    assert_eq!(reason(&out).0, "deny");
    assert!(!std::path::Path::new(&target).exists());
    let ok = dispatch(&g.h, "pre-tool-use", &payload("Write", &g.fx.p("workspace/life/knowledge/x.md"), &g.h.root).to_string()).unwrap();
    assert!(ok.is_none());
}

#[test]
fn test_the_inbox_staging_folder_is_not_denied_but_a_look_alike_is() {
    let g = G::new();
    for p in [
        "workspace/life/sources/inbox/email-2026-11-15-thread.md",
        "workspace/life/sources/inbox/order-2026-11-15-note.md",
        "workspace/life/sources/inbox/deeper/x.md",
    ] {
        assert!(g.decide("Write", &g.fx.p(p)).is_none(), "{p}");
    }
    let rel = payload("Write", "workspace/life/sources/inbox/order-2026-11-15-note.md", &g.h.root);
    assert!(source_guard::decide(&g.h, &rel).is_none(), "relative too");
    assert_eq!(reason(&g.decide("Write", &g.fx.p("workspace/life/sources/inbox-extra/x.md"))).0, "deny");
    assert_eq!(reason(&g.decide("Write", &g.fx.p("workspace/life/sources/orders/inbox/x.md"))).0, "deny");
    assert_eq!(reason(&g.decide("Write", &g.fx.p("workspace/life/sources/inbox.md"))).0, "deny");
}
