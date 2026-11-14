//! Context size per chat: the nudge on `post-tool-use`, the standing summary instructions on
//! `pre-compact`, the re-arm on `post-compact`, and `af session context`. A hook can measure a
//! chat's context and add text to it; it cannot set the window or start a compaction.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::life::vpath;
use crate::out;
use crate::outln;
use crate::pyos;
use crate::reach;
use crate::sessions::{self, ClassLimits, Registry, Row, SessionClass};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// While a chat stays above its hard threshold, the hard nudge repeats every this many tokens.
pub const HARD_REPEAT: u64 = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Soft,
    Hard,
}

fn k(tokens: u64) -> u64 {
    (tokens + 500) / 1000
}

pub fn nudge_text(class: SessionClass, name: &str, limits: &ClassLimits, tokens: u64, tier: Tier) -> String {
    let (n, c) = (k(tokens), class.name());
    match tier {
        Tier::Soft => format!(
            "Context: about {n}k tokens; this {c} chat's soft limit is {}k. Do not compact in the middle of a task: finish the task in hand first. When it is done, and before you start the next large task, bring handoff.md and the plan up to date and run `af session compact {name} --when-idle` (it compacts this chat once its turn ends). Compacting mid-task costs more than it saves, because the files must be read again.",
            k(limits.soft.unwrap_or(0))
        ),
        Tier::Hard => format!(
            "Context: about {n}k tokens, past this {c} chat's {}k limit. Finish the task in hand if it is close to done; start no new large task in this context. Then write the current state, decisions and next step into handoff.md and the plan, file anything that exists only in this conversation, and run `af session compact {name} --when-idle`.",
            k(limits.hard.unwrap_or(0))
        ),
    }
}

// ------------------------------------------------------------------ the markers

/// What has been said to one chat is a folder of marker files: `soft`, and `hard-<tokens>` for
/// each hard bucket (the hard threshold plus a multiple of 50k). Creating a marker claims that
/// firing, so of several hook processes started at the same instant exactly one emits it.
fn marker_dir(ctx: &Ctx, session_id: &str) -> PathBuf {
    let safe: String = session_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    PathBuf::from(vpath(ctx, &format!("system/logs/context-nudge/{safe}")))
}

/// True for the one process that creates the marker.
fn claim(dir: &Path, name: &str) -> bool {
    std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(name)).is_ok()
}

/// Forget everything said to this chat: both tiers fire again.
pub fn rearm(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn forget_hard(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with("hard-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The tier this measurement fires, if any, claiming its marker. Below the soft threshold both
/// tiers re-arm (a compaction); below the hard threshold the hard markers go. A jump over several
/// hard buckets fires the highest and claims the lower ones silently.
pub fn settle(limits: &ClassLimits, tokens: u64, dir: &Path) -> Option<Tier> {
    let floor = limits.soft.or(limits.hard)?;
    if tokens < floor {
        rearm(dir);
        return None;
    }
    std::fs::create_dir_all(dir).ok()?;
    if let Some(hard) = limits.hard {
        if tokens >= hard {
            let top = (tokens - hard) / HARD_REPEAT;
            claim(dir, "soft");
            for n in 0..top {
                claim(dir, &format!("hard-{}", hard + n * HARD_REPEAT));
            }
            return claim(dir, &format!("hard-{}", hard + top * HARD_REPEAT)).then_some(Tier::Hard);
        }
        forget_hard(dir);
    }
    match limits.soft {
        Some(soft) if tokens >= soft => claim(dir, "soft").then_some(Tier::Soft),
        _ => None,
    }
}

// ------------------------------------------------------------------ the hooks

/// The transcript to measure: the hook payload's path when it names a file, else the
/// lookup by session id.
pub fn transcript_for(payload: &Value, session_id: &str) -> Option<String> {
    let given = payload.get("transcript_path").and_then(Value::as_str).unwrap_or("");
    if !given.is_empty() && pyos::isfile(given) {
        return Some(given.to_string());
    }
    reach::transcript_path(session_id)
}

/// The nudge for this tool call, or None. Reads the registry, the brief and the transcript tail;
/// creates marker files when something fires. Every failure is silence.
pub fn nudge(ctx: &Ctx, payload: &Value) -> Option<String> {
    let id = payload.get("session_id").and_then(Value::as_str).unwrap_or("");
    // A subagent's tool call carries the parent's session id; the nudge is for the main chain.
    if id.is_empty() || payload.get("agent_id").is_some() {
        return None;
    }
    let row = Registry::load(ctx).ok()?.find_session(id)?;
    let class = sessions::class_of(ctx, &row.name);
    let limits = sessions::limits_of(ctx, &row.name, class);
    let tokens = reach::context_tokens(&transcript_for(payload, id)?)?;
    let tier = settle(&limits, tokens, &marker_dir(ctx, id))?;
    Some(nudge_text(class, &row.name, &limits, tokens, tier))
}

// ------------------------------------------------------------------ a failed --when-idle waiter

/// The one-line result a failed `--when-idle` waiter leaves for its chat, next to the chat's nudge markers.
pub fn waiter_result_path(ctx: &Ctx, session_id: &str) -> PathBuf {
    marker_dir(ctx, session_id).join("waiter-result.txt")
}

/// Leave the line for the chat; the context hook shows it once. A success leaves nothing.
pub fn write_waiter_result(ctx: &Ctx, session_id: &str, line: &str) {
    let path = waiter_result_path(ctx, session_id);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let flat = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = std::fs::write(path, format!("{} {flat}\n", crate::core::now_iso()));
}

/// The waiter's result for this chat, once: the process that removes the file shows it. Read before
/// the nudge, whose re-arm removes the markers' folder. None for a subagent's call or when there is none.
pub fn take_waiter_result(ctx: &Ctx, payload: &Value) -> Option<String> {
    let id = payload.get("session_id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() || payload.get("agent_id").is_some() {
        return None;
    }
    let path = waiter_result_path(ctx, id);
    let text = std::fs::read_to_string(&path).ok()?;
    std::fs::remove_file(&path).ok()?;
    let line = text.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// Stop: when a waiter's result is waiting and the stop is not already a continuation, the chat is kept
/// going once to read it (added to the handoff reason when the stop already blocks for that).
pub fn stop_with_waiter_result(ctx: &Ctx, payload_text: &str, o: sessions::HookOutput) -> sessions::HookOutput {
    let p: Value = serde_json::from_str(payload_text).unwrap_or(Value::Null);
    if p.get("stop_hook_active").and_then(Value::as_bool).unwrap_or(false) {
        return o;
    }
    let Some(line) = take_waiter_result(ctx, &p) else { return o };
    let note = format!("{line} (This is the outcome of the --when-idle waiter this chat started; it is shown once. Say it to the operator in one line if it matters, then stop.)");
    let reason = match serde_json::from_str::<Value>(&o.stdout).ok().and_then(|v| v.get("reason").and_then(Value::as_str).map(str::to_string)) {
        Some(r) => format!("{r}\n\n{note}"),
        None => note,
    };
    sessions::HookOutput { stdout: serde_json::json!({"decision": "block", "reason": reason}).to_string(), exit: o.exit }
}

/// PostCompact: a registered chat's nudges re-arm at once. Prints nothing.
pub fn postcompact(ctx: &Ctx, payload_text: &str) {
    let p: Value = serde_json::from_str(payload_text).unwrap_or(Value::Null);
    let id = p.get("session_id").and_then(Value::as_str).unwrap_or("");
    if Registry::load(ctx).ok().and_then(|r| r.find_session(id)).is_some() {
        rearm(&marker_dir(ctx, id));
    }
}

/// PreCompact: the standing instructions for the summary, as plain stdout (the harness hands
/// it to the summarizer). Empty for an unregistered chat.
pub fn precompact(ctx: &Ctx, payload_text: &str) -> String {
    let p: Value = serde_json::from_str(payload_text).unwrap_or(Value::Null);
    let id = p.get("session_id").and_then(Value::as_str).unwrap_or("");
    let row = match Registry::load(ctx).ok().and_then(|r| r.find_session(id)) {
        Some(r) => r,
        None => return String::new(),
    };
    let (name, role) = (&row.name, &row.role);
    let dir = sessions::session_rel(name);
    format!(
        "Standing instructions for this summary. This chat is the AgentFrame chat \"{name}\" (role {role}); its session folder is {dir}/.\n\
Keep: the chat's registered name ({name}) and role ({role}); the paths of its brief ({dir}/brief.md), its handoff ({dir}/handoff.md) and its plan; the current step and what remains; the decisions made and the reasons for them; the operator's open questions and standing instructions; the paths of the files touched.\n\
Drop: tool output already written to files, and abandoned dead ends."
    )
}

// ------------------------------------------------------------------ af session context

fn kfmt(tokens: u64) -> String {
    format!("{}k", k(tokens))
}

fn opt_k(v: Option<u64>) -> String {
    v.map(kfmt).unwrap_or_else(|| "none".to_string())
}

fn size_of(row: &Row) -> Option<reach::ContextSize> {
    Some(reach::context_size(&reach::transcript_path(&row.session_id)?))
}

/// The auto-compact window that applies to the chat's model, from the user settings.
enum AutoCompact {
    Unknown,
    Default(String),
    Set(u64, String),
}

fn auto_compact_of(usage: Option<&reach::Usage>) -> AutoCompact {
    match usage.and_then(|u| u.model.as_deref()) {
        None => AutoCompact::Unknown,
        Some(m) => {
            let id = m.split('[').next().unwrap_or(m).to_string();
            match reach::auto_compact_window(m) {
                Some(w) => AutoCompact::Set(w, id),
                None => AutoCompact::Default(id),
            }
        }
    }
}

/// The ids `system/models.json` names as current (the file maps an alias to an id), or None when the file
/// is missing or unreadable.
fn current_model_ids(ctx: &Ctx) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(crate::life::vpath(ctx, "system/models.json")).ok()?;
    let v = serde_json::from_str::<Value>(&text).ok()?;
    Some(v.as_object()?.values().filter_map(|id| id.as_str().map(str::to_string)).collect())
}

/// `af doctor`: any `modelSettings.<id>.autoCompactWindow` (for an id current in `system/models.json`) or
/// top-level `autoCompactWindow` in the user settings, of any size. AgentFrame nudges each chat by its
/// goal, so a blanket window, which also cuts every subagent on that model, is the operator's to remove.
/// With no readable `models.json`, any id that maps to an alias counts. Reads the settings file only
/// (`CLAUDE_CONFIG_DIR` or `~/.claude`); absent or unreadable is silence. The settings are never written.
pub fn doctor_notes(ctx: &Ctx) -> Vec<String> {
    let Some(dir) = reach::config_dir() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(pyos::join(&dir, "settings.json")) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let tail = "Claude Code compacts every chat and subagent there. AgentFrame sets this per chat with nudges; the operator removes the setting.";
    let mut notes = Vec::new();
    if let Some(window) = v.get("autoCompactWindow").and_then(Value::as_u64) {
        notes.push(format!("user settings hold a blanket auto-compact window of {}: {tail}", kfmt(window)));
    }
    let current = current_model_ids(ctx);
    for (key, s) in v.get("modelSettings").and_then(Value::as_object).into_iter().flatten() {
        let Some(window) = s.get("autoCompactWindow").and_then(Value::as_u64) else { continue };
        let id = key.split('[').next().unwrap_or(key);
        let known = match &current {
            Some(ids) => ids.iter().any(|c| c == id),
            None => crate::model_card::alias_of(id).is_some(),
        };
        if known {
            notes.push(format!(
                "user settings hold a blanket auto-compact window of {} for `{id}`: Claude Code compacts every chat and subagent on that model there. AgentFrame sets this per chat with nudges; the operator removes the setting.",
                kfmt(window)
            ));
        }
    }
    notes
}

pub fn cmd_context(ctx: &Ctx, name: Option<&str>) -> R<i32> {
    let reg = Registry::load(ctx)?;
    match name.filter(|n| !n.is_empty()) {
        Some(n) => {
            let row = match reg.find_name(n) {
                Some(r) => r,
                None => die!("no registry row for {n}"),
            };
            let class = sessions::class_of(ctx, &row.name);
            let lim = sessions::limits_of(ctx, &row.name, class);
            let size = size_of(&row);
            let usage = match &size {
                Some(reach::ContextSize::Known(u)) => Some(u.clone()),
                _ => None,
            };
            outln!("name: {}", row.name);
            outln!("class: {}", class.name());
            match (&usage, &size) {
                (Some(u), _) => outln!("context: {} tokens ({})", kfmt(u.tokens), u.tokens),
                (None, Some(reach::ContextSize::AfterCompact)) => {
                    outln!("context: unknown until its next reply (it was compacted after its last one)")
                }
                (None, _) => outln!("context: unknown (no transcript with usage)"),
            }
            outln!("window: {}", kfmt(lim.window));
            outln!("soft: {}", opt_k(lim.soft));
            outln!("hard: {}", opt_k(lim.hard));
            match auto_compact_of(usage.as_ref()) {
                AutoCompact::Unknown => outln!("auto-compact: unknown"),
                AutoCompact::Default(m) => outln!("auto-compact: default (not set for {m})"),
                AutoCompact::Set(w, m) => {
                    outln!("auto-compact: {} (user settings, modelSettings.{m})", kfmt(w));
                    outln!(
                        "that window is a blanket setting for every chat and subagent on {m}; AgentFrame nudges this chat at {} and {} instead, so the operator removes it",
                        opt_k(lim.soft),
                        opt_k(lim.hard)
                    );
                }
            }
        }
        None => {
            let mut lines = Vec::new();
            for row in reg.rows().iter().filter(|r| !r.is_ended()) {
                let (context, auto) = match size_of(row) {
                    Some(reach::ContextSize::Known(u)) => {
                        let auto = match auto_compact_of(Some(&u)) {
                            AutoCompact::Unknown => "unknown".to_string(),
                            AutoCompact::Default(_) => "default".to_string(),
                            AutoCompact::Set(w, _) => kfmt(w),
                        };
                        (kfmt(u.tokens), auto)
                    }
                    Some(reach::ContextSize::AfterCompact) => ("unknown".to_string(), "unknown".to_string()),
                    _ => continue,
                };
                let class = sessions::class_of(ctx, &row.name);
                let lim = sessions::limits_of(ctx, &row.name, class);
                lines.push(format!(
                    "| {} | {} | {context} | {} | {} | {auto} |",
                    row.name,
                    class.name(),
                    opt_k(lim.soft),
                    opt_k(lim.hard)
                ));
            }
            if lines.is_empty() {
                out::line("no registered chat has a transcript");
            } else {
                outln!("| name | class | context | soft | hard | auto-compact |");
                outln!("|---|---|---|---|---|---|");
                for l in lines {
                    out::line(&l);
                }
            }
        }
    }
    Ok(0)
}
