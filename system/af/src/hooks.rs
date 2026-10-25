//! `af hook <event>`: the Claude Code hooks in one process start. The payload arrives on
//! stdin; the hook's JSON (or nothing) goes to stdout. The tool events route to their
//! guards by `tool_name`; the session events call the handlers in `sessions`.

pub mod index_refresh;
pub mod pathlib;
pub mod ppt_guard;
pub mod publish_guard;
pub mod push_guard;
pub mod shlex;
pub mod version_guard;
pub mod voice_guard;

use crate::core::Ctx;
use crate::out;
use crate::pyjson::{self, PyVal};
use serde_json::Value;
use std::io::{Read, Write};

pub const INTERPRETERS: [&str; 6] = ["python", "python3", "py", "python.exe", "python3.exe", "py.exe"];

pub use crate::pyjson::truthy;

/// The repo root the hook scripts compute (`Path(__file__).resolve().parents[2]`) and af's
/// globals over it.
pub struct HookCtx {
    pub root: String,
    pub ctx: Ctx,
}

impl HookCtx {
    pub fn new(root: &str) -> HookCtx {
        let root = pathlib::resolve(root);
        HookCtx { ctx: Ctx::new(&root), root }
    }
}

/// `payload.get("hook_event_name") or "PreToolUse"`; None for a non-string value.
pub fn event_of(v: Option<&Value>) -> Option<String> {
    pyjson::str_or(v, "PreToolUse").map(|s| s.to_string())
}

pub fn event_is(v: Option<&Value>, want: &str) -> bool {
    event_of(v).as_deref() == Some(want)
}

pub fn deny(reason: &str) -> PyVal {
    PyVal::dict(vec![(
        "hookSpecificOutput",
        PyVal::dict(vec![
            ("hookEventName", PyVal::s("PreToolUse")),
            ("permissionDecision", PyVal::s("deny")),
            ("permissionDecisionReason", PyVal::s(reason)),
        ]),
    )])
}

pub fn context(event: &str, text: &str) -> PyVal {
    PyVal::dict(vec![(
        "hookSpecificOutput",
        PyVal::dict(vec![("hookEventName", PyVal::s(event)), ("additionalContext", PyVal::s(text))]),
    )])
}

fn inner(v: &PyVal) -> Option<&Vec<(String, PyVal)>> {
    match v {
        PyVal::Dict(items) => match items.first() {
            Some((k, PyVal::Dict(inner))) if k == "hookSpecificOutput" => Some(inner),
            _ => None,
        },
        _ => None,
    }
}

fn field<'a>(items: &'a [(String, PyVal)], key: &str) -> Option<&'a str> {
    items.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
        PyVal::Str(s) => Some(s.as_str()),
        _ => None,
    })
}

/// Several guards on one event, as Claude Code combines several hook commands: the first
/// deny wins; otherwise the additional contexts join in wiring order.
pub fn combine(results: Vec<PyVal>) -> Option<PyVal> {
    if results.len() <= 1 {
        return results.into_iter().next();
    }
    if let Some(d) = results.iter().find(|r| inner(r).and_then(|i| field(i, "permissionDecision")) == Some("deny")) {
        return Some(d.clone());
    }
    let event = results.iter().find_map(|r| inner(r).and_then(|i| field(i, "hookEventName"))).unwrap_or("PreToolUse").to_string();
    let texts: Vec<&str> = results.iter().filter_map(|r| inner(r).and_then(|i| field(i, "additionalContext"))).collect();
    if texts.is_empty() {
        return results.into_iter().next();
    }
    Some(context(&event, &texts.join("\n\n")))
}

/// The guards one tool event runs, by tool name: Ok(result) or Err(exit code) when a
/// guard ends the process the way its Python does.
pub fn dispatch(h: &HookCtx, event: &str, stdin_text: &str) -> Result<Option<PyVal>, i32> {
    let payload: Value = match serde_json::from_str(stdin_text) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let tool = payload.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let mut results = Vec::new();
    match (event, tool) {
        ("pre-tool-use", "Edit" | "Write") => results.extend(version_guard::decide(h, &payload)),
        ("pre-tool-use" | "post-tool-use", "Bash" | "PowerShell") => {
            if event == "pre-tool-use" {
                push_guard::check(&payload)?;
            }
            results.extend(ppt_guard::decide(h, &payload));
            results.extend(voice_guard::decide(h, &payload));
        }
        ("pre-tool-use", publish_guard::GATED_TOOL) => results.extend(publish_guard::decide(h, &payload)?),
        _ => {}
    }
    Ok(combine(results))
}

pub const EVENTS: [&str; 6] = ["pre-tool-use", "post-tool-use", "session-start", "stop", "session-end", "notification"];

fn emit(o: crate::sessions::HookOutput) -> i32 {
    if !o.stdout.is_empty() {
        out::line(&o.stdout);
    }
    o.exit
}

/// `af hook <event>`: one process start per event.
pub fn main(args: &[String]) -> i32 {
    let event = match args {
        [e] if EVENTS.contains(&e.as_str()) => e.as_str(),
        _ => {
            out::err_line(&format!("af hook: give one event: {}", EVENTS.join(", ")));
            return 1;
        }
    };
    let h = HookCtx::new(&crate::cli::discover_root());
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    use crate::sessions;
    match event {
        "session-start" => {
            let refresh = index_refresh::refresh(&h.root);
            return emit(sessions::hook_session_start_with(&h.ctx, &text, refresh));
        }
        "stop" => return emit(sessions::hook_stop(&h.ctx, &text)),
        "session-end" => return emit(sessions::hook_session_end(&h.ctx, &text)),
        "notification" => return emit(sessions::hook_notification(&h.ctx, &text)),
        _ => {}
    }
    match dispatch(&h, event, &text) {
        Ok(result) => {
            let s = result.map(|r| pyjson::dumps(&r)).unwrap_or_else(|| "{}".to_string());
            print!("{s}");
            let _ = std::io::stdout().flush();
            0
        }
        Err(code) => code,
    }
}

/// `af hook ...` runs outside the button wrapper: no run-log row, no argument errors.
pub fn intercept(argv: &[String]) -> Option<i32> {
    if argv.first().map(String::as_str) == Some("hook") {
        Some(main(&argv[1..]))
    } else {
        None
    }
}
