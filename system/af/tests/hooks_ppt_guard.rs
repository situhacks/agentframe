//! Port of system/tests/test_ppt_master_guard.py (the hook cases; the doctor stray-deck
//! cases were ported in R1 to tests/design_language_check.rs).

mod common;

use afcore::hooks::{dispatch, ppt_guard, HookCtx};
use afcore::pyjson::dumps;
use common::Fx;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime};

fn payload(command: &str, cwd: &str, event: &str) -> Value {
    json!({"hook_event_name": event, "tool_name": "Bash", "tool_input": {"command": command}, "cwd": cwd})
}

fn repo() -> HookCtx {
    HookCtx::new(&common::repo_root().to_string_lossy())
}

fn decide(h: &HookCtx, p: &Value) -> Option<Value> {
    ppt_guard::decide(h, p).map(|v| serde_json::from_str(&dumps(&v)).unwrap())
}

fn skill_dir(h: &HookCtx) -> String {
    format!("{}\\system\\vendor\\ppt-master", h.root).replace('\\', std::path::MAIN_SEPARATOR_STR)
}

// TestStagingGuard

#[test]
fn test_init_without_dir_in_skill_cwd_denied() {
    let h = repo();
    let o = decide(&h, &payload("python3 scripts/project_manager.py init demo_deck --format 16:9", &skill_dir(&h), "PreToolUse")).unwrap();
    assert_eq!(o["hookSpecificOutput"]["permissionDecision"], "deny");
    let r = o["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(r.contains("--dir"));
    let sep = std::path::MAIN_SEPARATOR;
    assert!(r.contains(&format!("{}{sep}system{sep}projects", h.root)));
}

#[test]
fn test_init_with_dir_inside_skill_denied() {
    let h = repo();
    let cmd = format!("python scripts/project_manager.py init demo --dir \"{}\\projects\"", skill_dir(&h));
    let o = decide(&h, &payload(&cmd, &h.root, "PreToolUse")).unwrap();
    assert_eq!(o["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn test_init_with_workspace_dir_allowed() {
    let h = repo();
    let target = format!("{}\\workspace\\projects\\demo\\phase-4-production\\decks", h.root);
    let cmd = format!("python scripts/project_manager.py init demo --dir \"{target}\"");
    assert!(decide(&h, &payload(&cmd, &skill_dir(&h), "PreToolUse")).is_none());
}

#[test]
fn test_init_with_tmp_dir_allowed() {
    let h = repo();
    let cmd = "python scripts/project_manager.py init demo --dir \"C:\\tmp\\decks\"";
    assert!(decide(&h, &payload(cmd, &skill_dir(&h), "PreToolUse")).is_none());
}

#[test]
fn test_non_init_subcommand_ignored() {
    let h = repo();
    assert!(decide(&h, &payload("python scripts/project_manager.py info projects/demo", &skill_dir(&h), "PreToolUse")).is_none());
}

#[test]
fn test_unrelated_command_ignored() {
    let h = repo();
    assert!(decide(&h, &payload("git status", &h.root, "PreToolUse")).is_none());
}

// TestExportGuard

struct E {
    fx: Fx,
    h: HookCtx,
}

impl E {
    fn new() -> E {
        let fx = Fx::new();
        fx.mkdir("proj/svg_output");
        E { fx, h: repo() }
    }
    fn proj(&self) -> String {
        self.fx.p("proj")
    }
    fn export_payload(&self, event: &str) -> Value {
        payload(&format!("python3 scripts/svg_to_pptx.py \"{}\" --no-notes", self.proj()), &self.fx.root(), event)
    }
    fn write_export(&self, age_s: u64) {
        let deck = self.fx.write_bytes("proj/exports/deck_20260827_120000.pptx", b"pptx");
        if age_s > 0 {
            let f = std::fs::File::options().write(true).open(&deck).unwrap();
            f.set_modified(SystemTime::now() - Duration::from_secs(age_s)).unwrap();
        }
    }
}

#[test]
fn test_export_is_not_gated_at_pretooluse() {
    let e = E::new();
    assert!(decide(&e.h, &e.export_payload("PreToolUse")).is_none());
}

#[test]
fn test_post_export_promotion_reminder() {
    let e = E::new();
    e.write_export(0);
    let o = decide(&e.h, &e.export_payload("PostToolUse")).unwrap();
    let ctx = o["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(ctx.to_lowercase().contains("promote"));
    assert!(ctx.contains("exports/"));
}

#[test]
fn test_post_unrelated_command_ignored() {
    let h = repo();
    assert!(decide(&h, &payload("git status", &h.root, "PostToolUse")).is_none());
}

#[test]
fn test_post_crash_produces_no_promotion_claim() {
    let e = E::new();
    assert!(decide(&e.h, &e.export_payload("PostToolUse")).is_none());
}

#[test]
fn test_post_help_produces_no_promotion_claim() {
    let e = E::new();
    assert!(decide(&e.h, &payload("python3 scripts/svg_to_pptx.py --help", &e.fx.root(), "PostToolUse")).is_none());
}

#[test]
fn test_post_command_that_only_names_the_script_is_ignored() {
    let e = E::new();
    e.write_export(0);
    let cmd = format!("grep -rn spec_lock {}/scripts/svg_to_pptx.py", e.proj());
    assert!(decide(&e.h, &payload(&cmd, &e.fx.root(), "PostToolUse")).is_none());
}

#[test]
fn test_post_stale_export_does_not_count() {
    let e = E::new();
    e.write_export(ppt_guard::EXPORT_FRESH_WINDOW_S as u64 + 60);
    assert!(decide(&e.h, &e.export_payload("PostToolUse")).is_none());
}

#[test]
fn test_post_export_with_flags_before_the_project_resolves() {
    let e = E::new();
    e.write_export(0);
    let cmd = format!("python3 scripts/svg_to_pptx.py --quick-generate \"{}\"", e.proj());
    let o = decide(&e.h, &payload(&cmd, &e.fx.root(), "PostToolUse")).unwrap();
    assert!(o["hookSpecificOutput"]["additionalContext"].as_str().unwrap().to_lowercase().contains("promote"));
}

// TestGuardMain

#[test]
fn test_main_emits_json_on_deny() {
    let h = repo();
    let p = payload("python scripts/project_manager.py init demo", &skill_dir(&h), "PreToolUse");
    let r = dispatch(&h, "pre-tool-use", &p.to_string()).unwrap().unwrap();
    let parsed: Value = serde_json::from_str(&dumps(&r)).unwrap();
    assert_eq!(parsed["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn test_main_silent_on_pass() {
    let h = repo();
    assert_eq!(dispatch(&h, "pre-tool-use", &payload("git status", &h.root, "PreToolUse").to_string()), Ok(None));
}

#[test]
fn test_main_silent_on_garbage() {
    let h = repo();
    assert_eq!(dispatch(&h, "pre-tool-use", "not json"), Ok(None));
}
