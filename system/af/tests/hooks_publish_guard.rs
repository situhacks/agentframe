//! The Substack publish guard.

mod common;

use afcore::hooks::{publish_guard, HookCtx};
use afcore::pyjson::dumps;
use common::Fx;
use serde_json::{json, Map, Value};

const TRACKER: &str = "---
name: Demo
slug: demo
deliverables:
  substack-essay-11:
    status: {status}
    file: posts/essay-v19.md
    substack_draft: {draft_id}
  other-row:
    status: drafting
    file: posts/other-v1.md
---

# Demo
";

fn payload(draft_id: Value, tool: &str, fields: &[(&str, &str)]) -> Value {
    let mut ti = Map::new();
    ti.insert("draft_id".into(), draft_id);
    for (k, v) in fields {
        ti.insert(k.to_string(), json!(v));
    }
    json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": ti})
}

fn upd(fields: &[(&str, &str)]) -> Value {
    payload(json!(100000001), publish_guard::GATED_TOOL, fields)
}

struct G {
    fx: Fx,
    h: HookCtx,
}

impl G {
    fn new() -> G {
        let fx = Fx::new();
        fx.mkdir("workspace/projects/demo");
        let h = HookCtx::new(&fx.root());
        G { fx, h }
    }
    fn write_tracker(&self, status: &str, draft_id: &str) {
        self.fx.write(
            "workspace/projects/demo/project.md",
            &TRACKER.replace("{status}", status).replace("{draft_id}", draft_id),
        );
    }
    fn decide(&self, p: &Value) -> Option<Value> {
        publish_guard::decide(&self.h, p).unwrap().map(|v| serde_json::from_str(&dumps(&v)).unwrap())
    }
}

#[test]
fn test_push_to_draft_row_below_ready_is_denied() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    let o = g.decide(&upd(&[("body", "the essay")])).unwrap();
    assert_eq!(o["hookSpecificOutput"]["permissionDecision"], "deny");
    let r = o["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(r.contains("substack-essay-11"));
    assert!(r.contains("`af ready "));
}

#[test]
fn test_title_only_push_is_also_gated() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    assert!(g.decide(&upd(&[("title", "New title")])).is_some());
}

#[test]
fn test_ready_row_passes() {
    let g = G::new();
    g.write_tracker("ready", "100000001");
    assert!(g.decide(&upd(&[("body", "the essay")])).is_none());
}

#[test]
fn test_published_row_passes() {
    let g = G::new();
    g.write_tracker("published", "100000001");
    assert!(g.decide(&upd(&[("body", "the essay")])).is_none());
}

#[test]
fn test_metadata_only_update_passes() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    assert!(g.decide(&upd(&[("audience", "everyone")])).is_none());
}

#[test]
fn test_untracked_draft_id_passes() {
    let g = G::new();
    g.write_tracker("drafting", "999");
    assert!(g.decide(&upd(&[("body", "the essay")])).is_none());
}

#[test]
fn test_create_draft_is_out_of_scope() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    assert!(g.decide(&payload(json!(100000001), "mcp__substack__create_draft", &[("body", "the essay")])).is_none());
}

#[test]
fn test_unrelated_tool_passes() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    assert!(g.decide(&payload(json!(100000001), "Edit", &[("body", "x")])).is_none());
}

#[test]
fn test_float_draft_id_still_joins() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    let p: Value = serde_json::from_str(
        r#"{"hook_event_name": "PreToolUse", "tool_name": "mcp__substack__update_draft", "tool_input": {"draft_id": 100000001.0, "body": "x"}}"#,
    )
    .unwrap();
    assert!(g.decide(&p).is_some());
}

#[test]
fn test_missing_draft_id_fails_open() {
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    let p = json!({"hook_event_name": "PreToolUse", "tool_name": "mcp__substack__update_draft", "tool_input": {"body": "the essay"}});
    assert!(g.decide(&p).is_none());
}

#[test]
fn test_malformed_payload_emits_empty_json() {
    let g = G::new();
    let o = g.fx.af().args(["hook", "pre-tool-use"]).write_stdin("not json").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), "{}");
}

// Edge shapes.

#[test]
fn test_tracker_without_frontmatter_exits_1_like_the_python() {
    // af.die() raises SystemExit, which the guard's `except Exception` does not catch.
    let g = G::new();
    g.write_tracker("drafting", "100000001");
    g.fx.write("workspace/projects/aaa/project.md", "no frontmatter\n");
    let o = g.fx.af().args(["hook", "pre-tool-use"]).write_stdin(upd(&[("body", "x")]).to_string()).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains("has no frontmatter block"));
}
