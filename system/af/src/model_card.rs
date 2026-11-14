//! The model card a build chat is given at session start: `library/models/<alias>.md`, the
//! standing lines for the model the chat runs on. `library/schema/models.md` says what a card is.

use crate::core::Ctx;
use crate::life::{vpath, TextFile};
use crate::reach;
use crate::sessions::{self, Row, SessionClass};

/// The Claude Code aliases, one card each.
pub const ALIASES: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];

/// The alias for a model id (`claude-fable-5-2`, with or without a `[1m]` suffix), for an alias, or for
/// Claude Code's `best` (which is Opus). `default`, `opusplan` and unknown ids have none.
pub fn alias_of(model: &str) -> Option<&'static str> {
    let m = model.split('[').next().unwrap_or(model).trim().to_ascii_lowercase();
    if m == "best" {
        return Some("opus");
    }
    ALIASES.iter().copied().find(|a| m == *a || m.starts_with(&format!("claude-{a}-")))
}

/// The alias of the model the chat runs on: the first source that maps to one, in this order: the hook
/// input's `model`, the model of the last assistant line in the transcript, the registry row's model. A
/// value that maps to none (`default`, `opusplan`, an unknown id) passes the search on to the next source.
fn alias_for(row: &Row, hook_model: Option<&str>, transcript: Option<&str>) -> Option<&'static str> {
    if let Some(a) = hook_model.and_then(alias_of) {
        return Some(a);
    }
    let usage = transcript.and_then(reach::tail).and_then(|text| reach::last_usage(&text));
    if let Some(a) = usage.and_then(|u| [u.model, u.requested].into_iter().flatten().find_map(|m| alias_of(&m))) {
        return Some(a);
    }
    alias_of(&row.model)
}

/// The bundle's card block for a build chat (never a worker), or None: no card file, no known model, or
/// another class.
pub fn card_block(ctx: &Ctx, row: &Row, hook_model: Option<&str>, transcript: Option<&str>) -> Option<String> {
    if row.role == "worker" || sessions::class_of(ctx, &row.name) != SessionClass::Build {
        return None;
    }
    let found;
    let transcript = match transcript {
        Some(t) => Some(t),
        None => {
            found = reach::transcript_path(&row.session_id);
            found.as_deref()
        }
    };
    let alias = alias_for(row, hook_model, transcript)?;
    let text = TextFile::load(&vpath(ctx, &format!("library/models/{alias}.md"))).ok()??.text;
    let text = text.trim_end();
    (!text.is_empty()).then(|| format!("model card ({alias}):\n{text}"))
}
