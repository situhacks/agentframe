//! Deny unsafe writes to versioned files.

use super::pathlib::{is_absolute, is_under, name, pjoin, pstr};
use super::{deny, event_is, HookCtx};
use crate::fm::rx;
use crate::pyjson::{str_or, PyVal};
use crate::pyos::{self, py_strip};
use serde_json::Value;
use std::path::Path;

fn version_re() -> regex::Regex {
    rx(r"\A(.+)-v(\d+)\.md\z")
}

fn within_managed_work(h: &HookCtx, path: &str) -> bool {
    [
        pjoin(&h.root, "workspace/projects"),
        pjoin(&h.root, "workspace/pipeline/applications"),
        pjoin(&h.root, "workspace/studio/posts"),
    ]
    .iter()
    .any(|r| is_under(path, r))
}

/// `Path.read_text(encoding="utf-8-sig")`: Ok(None) on OSError, Err(()) on a decode error
/// (the Python lets that one escape, so the whole guard fails open).
fn read_text(path: &str) -> Result<Option<String>, ()> {
    match std::fs::read(path) {
        Err(_) => Ok(None),
        Ok(bytes) => match pyos::decode_text(&bytes) {
            Ok(t) => Ok(Some(t)),
            Err(_) => Err(()),
        },
    }
}

fn status(path: &str) -> Result<Option<String>, ()> {
    let Some(text) = read_text(path)? else { return Ok(None) };
    let head: String = text.chars().take(4000).collect();
    Ok(rx(r"(?m)^status:\s*([A-Za-z_-]+)\s*$").captures(&head).map(|c| c[1].to_string()))
}

fn has_drafted_body(path: &str) -> Result<bool, ()> {
    let Some(text) = read_text(path)? else { return Ok(false) };
    let rest = rx(r"(?s)\A---\n.*?\n---\n?").replace(&text, "");
    Ok(!py_strip(&rest).is_empty())
}

/// `_versions(path, name)`. `Path.iterdir()` is lazy, so an unlistable folder raises
/// outside the Python's `except OSError` and the whole guard fails open: Err(()).
fn versions(path: &str, stem: &str) -> Result<Vec<u64>, ()> {
    let parent = pyos::dirname(path);
    let parent = if parent.is_empty() { ".".to_string() } else { parent };
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&parent) else { return Err(()) };
    let re = version_re();
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if let Some(c) = re.captures(&n) {
            if &c[1] == stem {
                out.push(c[2].parse::<u64>().unwrap_or(u64::MAX));
            }
        }
    }
    Ok(out)
}

fn decide_target(h: &HookCtx, path: &str, write: bool) -> Result<Option<PyVal>, ()> {
    if !within_managed_work(h, path) {
        return Ok(None);
    }
    let fname = name(path);
    let exists = Path::new(path).exists();
    if exists && status(path)?.as_deref() == Some("published") {
        let exit_path = if version_re().is_match(&fname) {
            "run `af version` with the row or nested-artifact address; \
the command creates a new drafting head while preserving the published version."
        } else {
            "create a new tracked edition; this unversioned published record cannot be reopened."
        };
        return Ok(Some(deny(&format!("{fname} is published and immutable. Direct edits are not allowed. {exit_path}"))));
    }
    let Some(c) = version_re().captures(&fname) else { return Ok(None) };
    let (stem, number) = (c[1].to_string(), c[2].parse::<u64>().unwrap_or(u64::MAX));
    let versions = versions(path, &stem)?;
    if !exists {
        if !versions.is_empty() {
            return Ok(Some(deny(&format!(
                "Do not hand-create {fname}; '{stem}' already has a version chain. \
Use `af version <project> <row>` or add \
`--artifact <name>` for a nested artifact so numbering and tracker state stay safe."
            ))));
        }
        return Ok(Some(deny(&format!(
            "Do not hand-create first draft {fname}. Use \
`af draft <project> <row> --file <path>` or \
`--artifact <name>` so v1 and tracker state are created together."
        ))));
    }
    if let Some(&max) = versions.iter().max() {
        if number < max {
            return Ok(Some(deny(&format!(
                "{fname} is an immutable prior version; {stem}-v{max}.md is the head. \
Restore by creating a new version, never by editing history."
            ))));
        }
    }
    if write && has_drafted_body(path)? {
        return Ok(Some(deny(&format!(
            "Full-file Write would clobber {fname}'s drafted content, and workspace files have \
no git history to restore from. If this head is already the fresh copy from \
`af version`, write the change as targeted Edit calls on it. Otherwise snapshot first \
(`af version <project> <row>`, `--artifact <name>` for nested), then \
apply the rewrite as one Edit replacing the new head's body."
        ))));
    }
    Ok(None)
}

/// `decide(payload)`; None for no decision and for every exception the Python swallows.
pub fn decide(h: &HookCtx, payload: &Value) -> Option<PyVal> {
    let obj = payload.as_object()?;
    if !event_is(obj.get("hook_event_name"), "PreToolUse") {
        return None;
    }
    let tool = obj.get("tool_name");
    let tool = match tool {
        Some(Value::Array(_)) | Some(Value::Object(_)) if super::truthy(tool) => return None, // unhashable
        _ => str_or(tool, "").unwrap_or(""),
    };
    if tool != "Edit" && tool != "Write" {
        return None;
    }
    let ti = obj.get("tool_input");
    let input = if super::truthy(ti) { ti?.as_object()? } else { return None };
    let raw = if super::truthy(input.get("file_path")) { input.get("file_path") } else { input.get("path") };
    let raw = str_or(raw, "")?;
    if raw.is_empty() {
        return None;
    }
    let cleaned = py_strip(raw).trim_matches('"').to_string();
    let mut path = pstr(&cleaned);
    if !is_absolute(&path) {
        let base = str_or(obj.get("cwd"), &h.root)?;
        path = pjoin(base, &path);
    }
    decide_target(h, &path, tool == "Write").unwrap_or(None)
}
