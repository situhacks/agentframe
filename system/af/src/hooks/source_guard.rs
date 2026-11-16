//! Deny a hand-written new source file: ids come from `af source add`, which takes the next
//! one under a lock. Edits to existing files and INDEX.md stay allowed, and so does the staging
//! folder `sources/inbox/`, where `/file` drops inputs before the filer gives them an id.
//!
//! The limit, stated plainly: this is a hook on the `Write` tool. A file made by a Bash or
//! PowerShell command, or reached through a UNC `localhost` share path, is not caught here; `af doctor`'s
//! duplicate-id check is the backstop. `af source add` itself is Rust and never goes through the hook.

use super::pathlib::{is_under, pjoin};
use super::version_guard::edit_target;
use super::{deny, HookCtx};
use crate::pyjson::PyVal;
use serde_json::Value;
use std::path::Path;

pub const REASON: &str = "File sources with `af source add` (it takes the next id under a lock; \
`--body-file -` reads standard input). A hand-written source file can take an id another chat already holds.";

/// A `Write` to a path that does not exist yet under `workspace/life/sources/`, outside `inbox/`.
pub fn decide(h: &HookCtx, payload: &Value) -> Option<PyVal> {
    let (tool, path) = edit_target(h, payload)?;
    let sources = pjoin(&h.root, "workspace/life/sources");
    if tool != "Write" || !is_under(&path, &sources) || is_under(&path, &pjoin(&sources, "inbox")) || Path::new(&path).exists() {
        return None;
    }
    Some(deny(REASON))
}
