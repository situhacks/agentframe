//! The push gate: a Bash or PowerShell command that runs `git push` turns the pre-tool-use
//! hook into a permission prompt. Post-tool-use never asks; the Edit and Write events never
//! see it; an ask outranks the other guards' contexts.

mod common;

use afcore::hooks::{combine, context, dispatch, push_guard, HookCtx};
use afcore::pyjson::dumps;
use serde_json::{json, Value};

fn repo() -> HookCtx {
    HookCtx::new(&common::repo_root().to_string_lossy())
}

fn bash(command: &str, event: &str, tool: &str) -> String {
    json!({"hook_event_name": event, "tool_name": tool, "tool_input": {"command": command}, "cwd": "."}).to_string()
}

fn as_json(v: afcore::pyjson::PyVal) -> Value {
    serde_json::from_str(&dumps(&v)).unwrap()
}

#[test]
fn test_push_forms_match() {
    for c in [
        "git push",
        "git push origin master",
        "git push -u origin master",
        "git push --force-with-lease",
        "git  push",
        "cd x && git push",
        "git add -A; git push origin HEAD",
        "git commit -m x | tee log && git push",
        "cmd /c \"git push\"",
        "git push\n",
    ] {
        assert!(push_guard::is_push(c), "{c:?}");
    }
}

#[test]
fn test_non_push_forms_pass() {
    for c in [
        "git status",
        "git commit -m 'guard the push'",
        "git push-options",
        "git pushx",
        "agit push",
        "git-push",
        "echo git pushed",
        "",
    ] {
        assert!(!push_guard::is_push(c), "{c:?}");
    }
}

#[test]
fn test_pre_tool_use_push_asks_for_bash_and_powershell() {
    let h = repo();
    for tool in ["Bash", "PowerShell"] {
        for cmd in ["git push origin master", "cd a && git push"] {
            let v = as_json(dispatch(&h, "pre-tool-use", &bash(cmd, "PreToolUse", tool)).unwrap().unwrap());
            assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask", "{tool}: {cmd}");
            assert_eq!(v["hookSpecificOutput"]["permissionDecisionReason"], push_guard::REASON);
            assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        }
    }
}

#[test]
fn test_push_passes_post_tool_use_and_other_tools() {
    let h = repo();
    assert_eq!(dispatch(&h, "post-tool-use", &bash("git push origin master", "PostToolUse", "Bash")), Ok(None));
    assert_eq!(dispatch(&h, "pre-tool-use", &bash("git push", "PreToolUse", "Edit")), Ok(None));
    assert_eq!(dispatch(&h, "pre-tool-use", &bash("git status && git log -1", "PreToolUse", "Bash")), Ok(None));
}

#[test]
fn test_an_ask_outranks_contexts_when_guards_combine() {
    let ask = push_guard::decide(&json!({"tool_input": {"command": "git push"}})).unwrap();
    let v = as_json(combine(vec![context("PreToolUse", "a note"), ask, context("PreToolUse", "another")]).unwrap());
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask");
    assert!(push_guard::decide(&json!({"tool_input": {"command": "git status"}})).is_none());
}
