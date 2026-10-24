//! The push is the operator's. A Bash or PowerShell command that runs `git push` anywhere in
//! its text ends the pre-tool-use hook with exit 2, which Claude Code honours even in
//! bypass-permissions mode, where a `deny` decision or an `ask` from a hook may not hold.
//! `permissions.deny` in `.claude/settings.json` is the first layer; this is the second.

use crate::out;
use regex::Regex;
use serde_json::Value;

/// `git push` as a whole-word subcommand anywhere in the command: at the start, after
/// `&&`, `;`, `|`, a quote, or a space, and followed by a space, a quote, or the end.
/// `git-push`, `git pushx`, and `agit push` do not match.
pub fn is_push(command: &str) -> bool {
    Regex::new(r#"(^|[^A-Za-z0-9_\-])git\s+push([\s"']|$)"#)
        .expect("push guard regex")
        .is_match(command)
}

pub const REASON: &str = "af: git push is the operator's. The harness never pushes; run it from a terminal after reviewing the commits.";

/// Err(2) with the reason on stderr when the command pushes; Ok(()) otherwise.
pub fn check(payload: &Value) -> Result<(), i32> {
    let command = payload
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if is_push(command) {
        out::err_line(REASON);
        return Err(2);
    }
    Ok(())
}
