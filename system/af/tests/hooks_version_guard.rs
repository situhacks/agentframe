//! Port of system/tests/test_version_guard.py.

mod common;

use afcore::hooks::{dispatch, version_guard, HookCtx};
use afcore::pyjson::PyVal;
use common::Fx;
use serde_json::{json, Value};

fn payload(h: &HookCtx, path: &str, tool: &str) -> Value {
    json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {"file_path": path}, "cwd": h.root})
}

/// (decision, reason) out of a hook result.
fn reason(v: &Option<PyVal>) -> (String, String) {
    let s = afcore::pyjson::dumps(v.as_ref().expect("expected a decision"));
    let j: Value = serde_json::from_str(&s).unwrap();
    let o = &j["hookSpecificOutput"];
    (
        o["permissionDecision"].as_str().unwrap_or("").to_string(),
        o["permissionDecisionReason"].as_str().unwrap_or("").to_string(),
    )
}

struct G {
    fx: Fx,
    h: HookCtx,
}

impl G {
    fn new() -> G {
        let fx = Fx::new();
        fx.mkdir("workspace/projects/demo/draft");
        let h = HookCtx::new(&fx.root());
        G { fx, h }
    }

    fn write_version(&self, n: u32, status: &str, body: &str) -> String {
        self.fx.write(
            &format!("workspace/projects/demo/draft/copy-v{n}.md"),
            &format!("---\nstatus: {status}\nlast_updated: 2026-07-14\n---\n\n{body}"),
        )
    }

    fn folder(&self, name: &str) -> String {
        self.fx.p(&format!("workspace/projects/demo/draft/{name}"))
    }

    fn decide(&self, path: &str, tool: &str) -> Option<PyVal> {
        version_guard::decide(&self.h, &payload(&self.h, path, tool))
    }
}

#[test]
fn test_current_drafting_head_allows_edit() {
    let g = G::new();
    let head = g.write_version(2, "drafting", "body\n");
    g.write_version(1, "drafting", "body\n");
    assert!(g.decide(&head, "Edit").is_none());
}

#[test]
fn test_full_write_over_nonempty_head_body_is_denied() {
    let g = G::new();
    let head = g.write_version(1, "drafting", "body\n");
    let (d, r) = reason(&g.decide(&head, "Write"));
    assert_eq!(d, "deny");
    assert!(r.contains("targeted Edit calls"));
    assert!(r.contains("af.py version"));
}

#[test]
fn test_full_write_into_empty_scaffold_head_is_allowed() {
    let g = G::new();
    let head = g.write_version(1, "drafting", "");
    assert!(g.decide(&head, "Write").is_none());
}

#[test]
fn test_edit_to_nonempty_head_body_stays_allowed() {
    let g = G::new();
    let head = g.write_version(1, "drafting", "body\n");
    assert!(g.decide(&head, "Edit").is_none());
}

#[test]
fn test_prior_version_is_denied() {
    let g = G::new();
    let prior = g.write_version(1, "drafting", "body\n");
    g.write_version(2, "drafting", "body\n");
    let (d, r) = reason(&g.decide(&prior, "Edit"));
    assert_eq!(d, "deny");
    assert!(r.contains("immutable prior version"));
}

#[test]
fn test_ready_head_full_write_is_denied_as_clobber() {
    let g = G::new();
    let head = g.write_version(1, "ready", "body\n");
    let (d, r) = reason(&g.decide(&head, "Write"));
    assert_eq!(d, "deny");
    assert!(r.contains("Full-file Write would clobber"));
}

#[test]
fn test_ready_head_allows_edit() {
    let g = G::new();
    let head = g.write_version(1, "ready", "body\n");
    assert!(g.decide(&head, "Edit").is_none());
}

#[test]
fn test_published_head_denies_edit() {
    let g = G::new();
    let head = g.write_version(1, "published", "body\n");
    assert!(reason(&g.decide(&head, "Edit")).1.contains("published and immutable"));
}

#[test]
fn test_unversioned_published_assembly_denies_edit() {
    let g = G::new();
    let assembly = g.fx.write("workspace/projects/demo/draft/post-FINAL.md", "---\nstatus: published\nlast_updated: 2026-07-19\n---\n\nbody\n");
    let (_, r) = reason(&g.decide(&assembly, "Edit"));
    assert!(r.contains("published and immutable"));
    assert!(r.contains("new tracked edition"));
}

#[test]
fn test_direct_new_version_is_denied() {
    let g = G::new();
    g.write_version(1, "drafting", "body\n");
    let (d, r) = reason(&g.decide(&g.folder("copy-v2.md"), "Write"));
    assert_eq!(d, "deny");
    assert!(r.contains("Do not hand-create"));
}

#[test]
fn test_direct_v1_is_denied() {
    let g = G::new();
    let (d, r) = reason(&g.decide(&g.folder("copy-v1.md"), "Write"));
    assert_eq!(d, "deny");
    assert!(r.contains("af.py draft"));
}

#[test]
fn test_non_versioned_and_non_project_files_pass() {
    let g = G::new();
    assert!(g.decide(&g.folder("post-FINAL.md"), "Edit").is_none());
    assert!(g.decide(&g.fx.p("README-v1.md"), "Edit").is_none());
}

#[test]
fn test_invalid_payload_fails_open() {
    let g = G::new();
    assert_eq!(dispatch(&g.h, "pre-tool-use", "not json"), Ok(None));
    assert_eq!(dispatch(&g.h, "pre-tool-use", &json!({"tool_name": "Edit"}).to_string()), Ok(None));
}

#[test]
fn test_tracked_claude_settings_wires_guard() {
    let text = std::fs::read_to_string(common::repo_root().join(".claude").join("settings.json")).unwrap();
    let settings: Value = serde_json::from_str(&text).unwrap();
    let commands: Vec<String> = settings["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["hooks"].as_array().unwrap().iter().map(|h| h["command"].as_str().unwrap().to_string()))
        .collect();
    assert!(commands.iter().any(|c| c.contains("version_guard.py")));
}

// Beyond the pytest file: shapes the Python handles by failing open.

#[test]
fn test_new_version_in_a_missing_folder_fails_open_like_the_python() {
    // Path.iterdir() is lazy, so the missing folder raises outside the Python's try.
    let g = G::new();
    assert!(g.decide(&g.fx.p("workspace/projects/demo/nofolder/copy-v1.md"), "Write").is_none());
}

#[test]
fn test_relative_path_joins_the_payload_cwd() {
    let g = G::new();
    g.write_version(1, "drafting", "body\n");
    g.write_version(2, "drafting", "body\n");
    let p = json!({"hook_event_name": "PreToolUse", "tool_name": "Edit",
                   "tool_input": {"file_path": "\"draft/copy-v1.md\" "}, "cwd": g.fx.p("workspace/projects/demo")});
    assert!(reason(&version_guard::decide(&g.h, &p)).1.contains("immutable prior version"));
    let post = json!({"hook_event_name": "PostToolUse", "tool_name": "Edit", "tool_input": {"file_path": g.folder("copy-v1.md")}});
    assert!(version_guard::decide(&g.h, &post).is_none());
}
