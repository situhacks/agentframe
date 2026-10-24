//! The push block: a Bash or PowerShell command that runs `git push` ends the pre-tool-use
//! hook with exit 2. Post-tool-use never blocks; the Edit and Write events never see it.

mod common;

use afcore::hooks::{dispatch, push_guard, HookCtx};
use serde_json::json;

fn repo() -> HookCtx {
    HookCtx::new(&common::repo_root().to_string_lossy())
}

fn bash(command: &str, event: &str, tool: &str) -> String {
    json!({"hook_event_name": event, "tool_name": tool, "tool_input": {"command": command}, "cwd": "."}).to_string()
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
fn test_pre_tool_use_push_exits_2_for_bash_and_powershell() {
    let h = repo();
    for tool in ["Bash", "PowerShell"] {
        assert_eq!(dispatch(&h, "pre-tool-use", &bash("git push origin master", "PreToolUse", tool)), Err(2), "{tool}");
        assert_eq!(dispatch(&h, "pre-tool-use", &bash("cd a && git push", "PreToolUse", tool)), Err(2), "{tool}");
    }
}

#[test]
fn test_push_passes_post_tool_use_and_other_tools() {
    let h = repo();
    assert_eq!(dispatch(&h, "post-tool-use", &bash("git push origin master", "PostToolUse", "Bash")), Ok(None));
    assert_eq!(dispatch(&h, "pre-tool-use", &bash("git push", "PreToolUse", "Edit")), Ok(None));
    assert_eq!(dispatch(&h, "pre-tool-use", &bash("git status && git log -1", "PreToolUse", "Bash")), Ok(None));
}
