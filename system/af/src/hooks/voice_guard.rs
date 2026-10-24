//! Port of system/hooks/voice_guard.py: the voice lint gate around `af ready` (deny on
//! hard findings) and the report after `af version`.

use super::pathlib::name;
use super::shlex::tokens;
use super::{context, deny, event_of, HookCtx, INTERPRETERS};
use crate::fm::rx;
use crate::pyjson::{py_str, str_or, PyVal};
use crate::pyos;
use crate::voice::lint::{self, Prev};
use crate::voice::resolve_head_quiet;
use serde_json::Value;

const READY_DENY_TAIL: &str = "Fix the hard findings in the head (anti-patterns.md § Banned tics and § Punctuation), \
then run af ready again. Operator override: prefix the command with AF_VOICE_LINT=skip.";

fn version_context(report: &str, new: &str, project: &str, deliverable: &str) -> String {
    format!(
        "{report}\n{new} now holds that text. If a VOICE BUNDLE for this register is already in your context with no \
compaction since, it is current: continue. Otherwise run `python system/af.py voice bundle {project} {deliverable} \
--tier core` for a copyedit pass over his text, or without --tier before drafting a new section, and read the \
file it names whole (voice/README.md and the rest of the voice system in one read). Hard findings above are \
yours to fix in this version, and every region you rewrite gets anti-patterns.md before it is surfaced."
    )
}

/// (verb, project, deliverable) when the command actually invokes af.py.
fn af_call(toks: &[String]) -> Option<(String, String, String)> {
    for (i, token) in toks.iter().enumerate() {
        let norm = token.replace('\\', "/");
        if !(norm == "af.py" || norm.ends_with("/af.py")) {
            continue;
        }
        if i != 0 {
            let prev = name(&toks[i - 1].replace('\\', "/")).to_lowercase();
            if !INTERPRETERS.contains(&prev.as_str()) {
                return None;
            }
        }
        let rest: Vec<&String> = toks[i + 1..]
            .iter()
            .filter(|t| !t.starts_with('-') && !["&&", ";", "|", "||"].contains(&t.as_str()))
            .collect();
        if rest.len() < 3 {
            return None;
        }
        return Some((rest[0].clone(), rest[1].clone(), rest[2].clone()));
    }
    None
}

/// `_tool_output(payload)`
fn tool_output(resp: Option<&Value>) -> Option<String> {
    match resp {
        None | Some(Value::Null) => None,
        Some(Value::Object(o)) => Some(
            o.values()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        Some(v) => Some(py_str(v)),
    }
}

pub fn decide(h: &HookCtx, payload: &Value) -> Option<PyVal> {
    let obj = payload.as_object()?;
    let ti = obj.get("tool_input");
    let command = if super::truthy(ti) {
        let input = ti?.as_object()?;
        str_or(input.get("command"), "")?
    } else {
        ""
    };
    if !command.replace('\\', "/").contains("af.py") {
        return None;
    }
    let event = event_of(obj.get("hook_event_name"));
    let (verb, project, deliverable) = af_call(&tokens(command))?;

    if verb == "ready" && event.as_deref() == Some("PreToolUse") {
        if rx(r#"(?i)AF_VOICE_LINT\s*=\s*['"]?skip"#).is_match(command) {
            return None;
        }
        let head = resolve_head_quiet(&h.ctx, &project, &deliverable).ok()??;
        let result = lint::lint_file(&head, Prev::Auto).ok()?;
        if !result.user_voiced {
            return None;
        }
        let report = lint::format_report(&result, &h.ctx.root);
        if !result.hard.is_empty() {
            return Some(deny(&format!("{report}\n{READY_DENY_TAIL}")));
        }
        if !result.soft.is_empty() {
            return Some(context("PreToolUse", &report));
        }
        return None;
    }

    if verb == "version" && event.as_deref() == Some("PostToolUse") {
        if let Some(output) = tool_output(obj.get("tool_response")) {
            if !output.contains("af version:") {
                return None; // the button refused; nothing new to lint
            }
        }
        let head = resolve_head_quiet(&h.ctx, &project, &deliverable).ok()??;
        let closed = lint::previous_version(&head);
        let before = closed.as_deref().and_then(lint::previous_version);
        let prev = match &before {
            Some(b) => Prev::Path(b),
            None => Prev::None,
        };
        let mut result = lint::lint_file(&head, prev).ok()?;
        if !result.user_voiced {
            return None;
        }
        result.path = Some(closed.clone().unwrap_or_else(|| head.clone()));
        let report = lint::format_report(&result, &h.ctx.root);
        return Some(context(
            "PostToolUse",
            &version_context(&report, &pyos::basename(&head), &project, &deliverable),
        ));
    }
    None
}
