//! Deny a hand-written new source file: ids come from `af source add`, which takes the next
//! one under a lock. Edits to existing files and INDEX.md stay allowed.

use super::pathlib::{is_under, pjoin};
use super::version_guard::edit_target;
use super::{deny, HookCtx};
use crate::pyjson::PyVal;
use serde_json::Value;
use std::path::Path;

pub const REASON: &str = "File sources with `af source add` (it takes the next id under a lock; \
`--body-file -` reads standard input). A hand-written source file can take an id another chat already holds.";

/// A `Write` to a path that does not exist yet under `workspace/life/sources/`.
pub fn decide(h: &HookCtx, payload: &Value) -> Option<PyVal> {
    let (tool, path) = edit_target(h, payload)?;
    if tool != "Write" || !is_under(&path, &pjoin(&h.root, "workspace/life/sources")) || Path::new(&path).exists() {
        return None;
    }
    Some(deny(REASON))
}
