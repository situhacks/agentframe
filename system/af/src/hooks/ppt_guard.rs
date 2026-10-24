//! The PPT Master staging deny and the export
//! promotion reminder.

use super::pathlib::{is_under, name, pjoin};
use super::shlex::tokens;
use super::{context, deny, event_of, HookCtx, INTERPRETERS};
use crate::pyjson::{str_or, PyVal};
use serde_json::Value;
use std::path::Path;

pub const EXPORT_FRESH_WINDOW_S: f64 = 1800.0;

const PROMOTE_CONTEXT: &str = "Deck export finished. Overlay contract (system/vendor/ppt-master/AGENTS.md, \
Outputs): promote the new .pptx - copy it from the working folder's exports/ \
into the calling deliverable folder, keeping its timestamped filename. The \
exports/ twin stays frozen as the agent's reference. Versioning and round-trip \
rules: library/process/deck-production.md.";

fn staging_reason(target: &str) -> String {
    format!(
        "PPT Master staging guard: this init would create the deck project at {target} - \
inside system/, outside any calling project, where af doctor flags it as a stray. Per \
system/vendor/ppt-master/AGENTS.md, stage deck projects inside the calling \
campaign (workspace/projects/<slug>/phase-4-production/decks/<deck-name>/) or \
C:\\tmp for throwaway runs. Re-run with an explicit --dir, e.g.: \
python scripts/project_manager.py init <name> --dir \"<absolute path outside system/>\""
    )
}

/// Index of a token that actually invokes `script`: argv[0], or right after an interpreter.
fn script_index(toks: &[String], script: &str) -> Option<usize> {
    for (i, token) in toks.iter().enumerate() {
        if !token.replace('\\', "/").ends_with(script) {
            continue;
        }
        if i == 0 {
            return Some(i);
        }
        let prev = name(&toks[i - 1].replace('\\', "/")).to_lowercase();
        if INTERPRETERS.contains(&prev.as_str()) {
            return Some(i);
        }
    }
    None
}

/// True when the project's exports/ holds a .pptx written within the window.
pub fn fresh_export(project: &str) -> bool {
    let exports = pjoin(project, "exports");
    let Ok(rd) = std::fs::read_dir(&exports) else { return false };
    let mut newest: Option<f64> = None;
    for e in rd.flatten() {
        let raw = e.file_name().to_string_lossy().into_owned();
        // pathlib globs case-insensitively on Windows
        let n = if cfg!(windows) { raw.to_lowercase() } else { raw };
        if !n.ends_with(".pptx") {
            continue;
        }
        let Ok(md) = std::fs::metadata(e.path()) else { return false };
        let Ok(t) = md.modified() else { return false };
        let secs = match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs_f64(),
            Err(e) => -e.duration().as_secs_f64(),
        };
        newest = Some(newest.map_or(secs, |n: f64| n.max(secs)));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    matches!(newest, Some(n) if now - n <= EXPORT_FRESH_WINDOW_S)
}

fn exported_project(toks: &[String], cwd: &str) -> Option<String> {
    let idx = script_index(toks, "svg_to_pptx.py")?;
    for token in &toks[idx + 1..] {
        if token.starts_with('-') {
            continue;
        }
        let candidate = pjoin(cwd, token);
        if Path::new(&candidate).is_dir() {
            return Some(candidate);
        }
    }
    None
}

fn staging_guard(h: &HookCtx, toks: &[String], cwd: &str) -> Option<PyVal> {
    let idx = script_index(toks, "project_manager.py")?;
    let rest = &toks[idx + 1..];
    if !rest.iter().any(|t| t == "init") {
        return None;
    }
    let mut target: Option<String> = None;
    for (i, tok) in rest.iter().enumerate() {
        if tok == "--dir" && i + 1 < rest.len() {
            target = Some(rest[i + 1].clone());
        } else if let Some(v) = tok.strip_prefix("--dir=") {
            target = Some(v.to_string());
        }
    }
    let vendor_repo_root = pjoin(&h.root, "system");
    let resolved = match target.filter(|t| !t.is_empty()) {
        Some(t) => pjoin(cwd, &t),
        None if is_under(cwd, &vendor_repo_root) => pjoin(&vendor_repo_root, "projects"),
        None => pjoin(cwd, "projects"),
    };
    if is_under(&resolved, &pjoin(&h.root, "system")) {
        return Some(deny(&staging_reason(&resolved)));
    }
    None
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
    if !command.contains("project_manager.py") && !command.contains("svg_to_pptx.py") {
        return None;
    }
    let cwd = str_or(obj.get("cwd"), ".")?;
    let event = event_of(obj.get("hook_event_name"));
    let toks = tokens(command);
    match event.as_deref() {
        Some("PreToolUse") => staging_guard(h, &toks, cwd),
        Some("PostToolUse") => {
            let project = exported_project(&toks, cwd)?;
            if !fresh_export(&project) {
                return None; // no observable export: say nothing rather than claim one
            }
            Some(context("PostToolUse", PROMOTE_CONTEXT))
        }
        _ => None,
    }
}
