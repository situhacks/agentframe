//! The `af hook` surface and its wiring: one process per event, payload on stdin, `{}`
//! when no rule matches.

mod common;

use afcore::hooks::{combine, context, deny};
use common::Fx;
use serde_json::{json, Value};

#[test]
fn test_claude_reference_wires_all_guards() {
    let text = std::fs::read_to_string(common::repo_root().join(".claude").join("settings.json")).unwrap();
    let config: Value = serde_json::from_str(&text).unwrap();
    let mut commands = Vec::new();
    for groups in config["hooks"].as_object().unwrap().values() {
        for group in groups.as_array().unwrap() {
            for hook in group["hooks"].as_array().unwrap() {
                commands.push(hook["command"].as_str().unwrap().to_string());
            }
        }
    }
    for event in ["session-start", "stop", "session-end", "notification", "pre-tool-use", "post-tool-use"] {
        let want = format!("AF_ROOT=\"${{CLAUDE_PROJECT_DIR}}\" af hook {event}");
        assert!(commands.iter().any(|c| c == &want), "{event}");
    }
    assert!(!commands.iter().any(|c| c.contains("python")), "every hook runs the binary");
    let matchers = |ev: &str| -> Vec<String> {
        config["hooks"][ev].as_array().unwrap().iter().map(|g| g["matcher"].as_str().unwrap_or("").to_string()).collect()
    };
    for m in ["Edit|Write", "Bash|PowerShell", "mcp__substack__update_draft"] {
        assert!(matchers("PreToolUse").iter().any(|x| x == m), "{m}");
    }
    assert_eq!(matchers("SessionStart"), ["startup|resume|clear|compact"]);
    assert_eq!(matchers("Notification"), ["agent_completed|agent_needs_input"]);
    assert_eq!(config["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 10);
    assert_eq!(config["permissions"]["deny"], serde_json::json!(["Bash(git push)", "Bash(git push *)"]));
    assert_eq!(config["modelSettings"]["claude-opus-5-5"]["autoCompactWindow"], 200000);
    assert_eq!(config["worktree"]["bgIsolation"], "none");
}

fn fixture() -> Fx {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    fx.mkdir("system");
    fx
}

fn hook(fx: &Fx, args: &[&str], stdin: &str) -> (i32, String) {
    let mut cmd = fx.af();
    cmd.arg("hook").args(args).write_stdin(stdin.to_string());
    let o = cmd.output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn test_guard_entrypoints_emit_valid_empty_json_when_no_rule_matches() {
    let fx = fixture();
    for tool in ["Bash", "PowerShell", "Edit", "Write", "mcp__substack__update_draft", "Read"] {
        let p = json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {"command": "git status"}, "cwd": fx.root()});
        for event in ["pre-tool-use", "post-tool-use"] {
            let (code, out) = hook(&fx, &[event], &p.to_string());
            assert_eq!(code, 0);
            assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), json!({}), "{tool} {event}");
        }
    }
}

#[test]
fn test_unknown_event_or_flag_is_a_non_blocking_error() {
    let fx = fixture();
    assert_eq!(hook(&fx, &["pre-compact"], "{}").0, 1);
    assert_eq!(hook(&fx, &[], "{}").0, 1);
    assert_eq!(hook(&fx, &["pre-tool-use", "--verbose"], "{}").0, 1, "an unknown flag fails like an unknown event");
    assert_eq!(hook(&fx, &["pre-tool-use", "stop"], "{}").0, 1, "one event per call");
}

#[test]
fn test_hooks_write_no_run_log_row() {
    let fx = fixture();
    hook(&fx, &["pre-tool-use"], "{}");
    assert!(fx.run_log_rows().is_empty());
}

#[test]
fn test_combine_lets_a_deny_win_and_joins_contexts() {
    let d = deny("no");
    assert_eq!(combine(vec![context("PreToolUse", "a"), d.clone()]), Some(d));
    assert_eq!(
        combine(vec![context("PostToolUse", "a"), context("PostToolUse", "b")]),
        Some(context("PostToolUse", "a\n\nb"))
    );
    assert_eq!(combine(vec![]), None);
}
