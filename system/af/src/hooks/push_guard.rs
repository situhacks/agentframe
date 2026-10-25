//! A push needs the operator's explicit yes. A Bash or PowerShell command that runs
//! `git push` anywhere in its text turns the pre-tool-use hook into a permission prompt
//! (`permissionDecision: ask`) carrying the reason, so the harness can push, but only after
//! he approves that push. In bypass-permissions mode the prompt does not appear; there the
//! rule in `AGENTS.md` is the gate: push only on his explicit yes in the chat.

use crate::pyjson::PyVal;
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

pub const REASON: &str = "af: this command pushes to a remote. A push happens only on the operator's explicit yes for this push, after he has seen the commits; approve only if he gave it in this chat.";

/// The ask decision when the command pushes; None otherwise.
pub fn decide(payload: &Value) -> Option<PyVal> {
    let command = payload
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if is_push(command) {
        return Some(super::ask(REASON));
    }
    None
}
