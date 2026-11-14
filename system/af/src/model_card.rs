//! The model card a build chat is given at session start: `library/models/<alias>.md`, the
//! standing lines for the model the chat runs on. `library/schema/models.md` says what a card is.

use crate::core::Ctx;
use crate::life::{vpath, TextFile};
use crate::reach;
use crate::sessions::{self, Row, SessionClass};

/// The Claude Code aliases, one card each.
pub const ALIASES: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];

/// The alias for a model id (`claude-fable-5-2`, with or without a `[1m]` suffix) or for an alias.
pub fn alias_of(model: &str) -> Option<&'static str> {
    let m = model.split('[').next().unwrap_or(model).trim().to_ascii_lowercase();
    ALIASES.iter().copied().find(|a| m == *a || m.starts_with(&format!("claude-{a}-")))
}

/// The model the chat runs on: the hook input's `model`, else the model of the last assistant line in
/// its transcript, else its registry row's model. The first one that is there decides, so an unknown
/// model gets no card rather than another source's.
fn model_of(row: &Row, hook_model: Option<&str>, transcript: Option<&str>) -> Option<String> {
    if let Some(m) = hook_model.map(str::trim).filter(|m| !m.is_empty()) {
        return Some(m.to_string());
    }
    let from_transcript = transcript
        .and_then(reach::tail)
        .and_then(|text| reach::last_usage(&text))
        .and_then(|u| u.model.or(u.requested));
    if from_transcript.is_some() {
        return from_transcript;
    }
    let m = row.model.trim();
    (!m.is_empty() && m != "-").then(|| m.to_string())
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
    let alias = alias_of(&model_of(row, hook_model, transcript)?)?;
    let text = TextFile::load(&vpath(ctx, &format!("library/models/{alias}.md"))).ok()??.text;
    let text = text.trim_end();
    (!text.is_empty()).then(|| format!("model card ({alias}):\n{text}"))
}
