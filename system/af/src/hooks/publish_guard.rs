//! Refuse a Substack draft push whose tracker row
//! sits below `ready`.

use super::{deny, event_is, truthy, HookCtx};
use crate::core;
use crate::fm::{self, rx};
use crate::out;
use crate::pyglob;
use crate::pyjson::{py_str, PyVal};
use crate::pyos::{self, py_strip};
use serde_json::Value;

pub const GATED_TOOL: &str = "mcp__substack__update_draft";
const CONTENT_FIELDS: [&str; 3] = ["body", "title", "subtitle"];
const TRACKER_GLOBS: [&str; 2] = ["workspace/projects/*/project.md", "workspace/pipeline/applications/*/application.md"];

fn reason(source: &str, slug: &str, status: &str, draft_id: &str, project: &str) -> String {
    format!(
        "Substack push refused: {source} row '{slug}' is '{status}', not ready. Pushing the \
body into draft {draft_id} is the last point AgentFrame can see this copy - the \
operator publishes from the Substack editor after this, where nothing can check it. \
Readiness is what runs the voice lint, the voice harvest, and the template's \
readiness criteria; skipping it here skips all three on the one artifact that reaches \
the public feed.\n\
Run `af ready {project} {slug}` first, or version and finish the head \
if it is not actually done. If this push is deliberate and the row is wrong, fix the \
row rather than working around this."
    )
}

/// The owning (source, project, row, status) of a tracked draft id. Err(1) is af's
/// `die()` on a tracker with no frontmatter: SystemExit escapes the guard's
/// `except Exception`, so the process exits 1 with nothing on stdout.
fn draft_owner(h: &HookCtx, draft_id: &str) -> Result<Option<(String, String, String, Option<String>)>, i32> {
    let field_re = rx(r"(?m)^    ([A-Za-z0-9_]*(?:_url|_draft))\s*:\s*(\S.*?)\s*$");
    for pattern in TRACKER_GLOBS {
        let mut paths = pyglob::glob(&super::pathlib::pjoin(&h.root, pattern), false);
        paths.sort();
        for path in paths {
            let Ok(text) = core::read(&path) else { continue };
            let Some((cfm, _)) = fm::split_fm_optional(&text) else {
                out::err_line(&format!("af: ERROR: {path} has no frontmatter block"));
                return Err(1);
            };
            for slug in fm::all_rows(&cfm) {
                let Some((s, e)) = fm::row_span(&cfm, &slug) else { continue };
                let block = &cfm[s..e];
                for c in field_re.captures_iter(block) {
                    if !c[1].ends_with("_draft") {
                        continue;
                    }
                    if py_strip(&fm::clean_value(&c[2])) != draft_id {
                        continue;
                    }
                    return Ok(Some((
                        pyos::basename(&path),
                        pyos::basename(&pyos::dirname(&path)),
                        slug.clone(),
                        fm::row_get(&cfm, &slug, "status"),
                    )));
                }
            }
        }
    }
    Ok(None)
}

pub fn decide(h: &HookCtx, payload: &Value) -> Result<Option<PyVal>, i32> {
    let Some(obj) = payload.as_object() else { return Ok(None) };
    if !event_is(obj.get("hook_event_name"), "PreToolUse") {
        return Ok(None);
    }
    if obj.get("tool_name").and_then(Value::as_str) != Some(GATED_TOOL) {
        return Ok(None);
    }
    let ti = obj.get("tool_input");
    let input = if truthy(ti) {
        match ti.and_then(Value::as_object) {
            Some(o) => o.clone(),
            None => return Ok(None),
        }
    } else {
        serde_json::Map::new()
    };
    let has_copy = CONTENT_FIELDS.iter().any(|f| {
        let v = input.get(*f);
        truthy(v) && !py_strip(&py_str(v.unwrap())).is_empty()
    });
    if !has_copy {
        return Ok(None); // metadata-only update carries no copy
    }
    let raw = match input.get("draft_id") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(s)) if s.is_empty() => return Ok(None),
        Some(v) => v,
    };
    let mut draft_id = py_strip(&py_str(raw)).to_string();
    if draft_id.ends_with(".0") {
        draft_id.truncate(draft_id.len() - 2);
    }
    let Some((source, project, slug, status)) = draft_owner(h, &draft_id)? else { return Ok(None) };
    if matches!(status.as_deref(), Some("ready") | Some("published")) {
        return Ok(None);
    }
    let status = status.unwrap_or_else(|| "None".into());
    Ok(Some(deny(&reason(&source, &slug, &status, &draft_id, &project))))
}
