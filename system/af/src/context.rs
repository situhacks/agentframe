//! Context size per chat: the nudge on `post-tool-use`, the standing summary instructions on
//! `pre-compact`, and `af session context`. A hook can measure a chat's context and add text to
//! it; it cannot set the window or start a compaction.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::life::vpath;
use crate::out;
use crate::outln;
use crate::pyos;
use crate::reach;
use crate::sessions::{self, ClassLimits, Registry, Row, SessionClass};
use serde_json::{json, Value};

/// While a chat stays above its hard threshold, the hard nudge repeats every this many tokens.
pub const HARD_REPEAT: u64 = 50_000;

/// What has already been said to one chat: soft once, hard at the context size it last fired at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NudgeState {
    pub soft: bool,
    pub hard_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Soft,
    Hard,
}

/// The tier to say now, if any; updates the state. Below the soft threshold both re-arm (the
/// chat was compacted). A jump past hard says hard only.
pub fn decide(limits: &ClassLimits, tokens: u64, st: &mut NudgeState) -> Option<Tier> {
    let floor = limits.soft.or(limits.hard)?;
    if tokens < floor {
        *st = NudgeState::default();
        return None;
    }
    if let Some(hard) = limits.hard {
        if tokens >= hard && (st.hard_at == 0 || tokens >= st.hard_at + HARD_REPEAT) {
            st.hard_at = tokens;
            st.soft = true;
            return Some(Tier::Hard);
        }
    }
    match limits.soft {
        Some(soft) if tokens >= soft && !st.soft => {
            st.soft = true;
            Some(Tier::Soft)
        }
        _ => None,
    }
}

fn k(tokens: u64) -> u64 {
    (tokens + 500) / 1000
}

pub fn nudge_text(class: SessionClass, limits: &ClassLimits, tokens: u64, tier: Tier) -> String {
    let (n, c) = (k(tokens), class.name());
    match tier {
        Tier::Soft => format!(
            "Context: about {n}k tokens; this {c} chat's soft limit is {}k. At the next stable point, bring handoff.md and the plan up to date so a fresh context could continue, then consider compacting. Not urgent: finish work that benefits from what is loaded.",
            k(limits.soft.unwrap_or(0))
        ),
        Tier::Hard => format!(
            "Context: about {n}k tokens, past this {c} chat's {}k limit. At the next natural stopping point: write the current state, decisions and next step into handoff.md and the plan, file anything that exists only in this conversation, then compact before starting new large work. If you cannot compact yourself, tell the operator in one line that this chat is ready for /compact.",
            k(limits.hard.unwrap_or(0))
        ),
    }
}

// ------------------------------------------------------------------ the state file

fn state_rel(session_id: &str) -> String {
    let safe: String = session_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("system/logs/context-nudge-{safe}.json")
}

/// A missing or corrupt file is nothing fired yet.
fn load_state(path: &str) -> NudgeState {
    let read = || -> Option<NudgeState> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        Some(NudgeState { soft: v.get("soft")?.as_bool()?, hard_at: v.get("hard_at")?.as_u64()? })
    };
    read().unwrap_or_default()
}

/// Write beside the file, then rename over it.
fn save_state(path: &str, st: &NudgeState) -> R<()> {
    let tmp = format!("{path}.{}.tmp", std::process::id());
    let dir = pyos::dirname(path);
    if !dir.is_empty() {
        pyos::makedirs(&dir)?;
    }
    pyos::write(&tmp, &json!({"soft": st.soft, "hard_at": st.hard_at}).to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        pyos::os_error(&e, path)
    })
}

// ------------------------------------------------------------------ the hooks

/// The transcript to measure: the hook payload's path when it names a file, else the
/// lookup by session id.
fn transcript_for(payload: &Value, session_id: &str) -> Option<String> {
    let given = payload.get("transcript_path").and_then(Value::as_str).unwrap_or("");
    if !given.is_empty() && pyos::isfile(given) {
        return Some(given.to_string());
    }
    reach::transcript_path(session_id)
}

/// The nudge for this tool call, or None. Reads the registry, the brief and the transcript tail;
/// writes one small state file when something fired or re-armed. Every failure is silence.
pub fn nudge(ctx: &Ctx, payload: &Value) -> Option<String> {
    let id = payload.get("session_id").and_then(Value::as_str).unwrap_or("");
    // A subagent's tool call carries the parent's session id; the nudge is for the main chain.
    if id.is_empty() || payload.get("agent_id").is_some() {
        return None;
    }
    let row = Registry::load(ctx).ok()?.find_session(id)?;
    let class = sessions::class_of(ctx, &row.name);
    let limits = class.limits();
    if limits.soft.is_none() && limits.hard.is_none() {
        return None;
    }
    let tokens = reach::context_tokens(&transcript_for(payload, id)?)?;
    let path = vpath(ctx, &state_rel(id));
    let before = load_state(&path);
    let mut st = before;
    let tier = decide(&limits, tokens, &mut st);
    if st != before {
        let _ = save_state(&path, &st);
    }
    tier.map(|t| nudge_text(class, &limits, tokens, t))
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

fn tokens_of(row: &Row) -> Option<u64> {
    reach::context_tokens(&reach::transcript_path(&row.session_id)?)
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
            let lim = class.limits();
            outln!("name: {}", row.name);
            outln!("class: {}", class.name());
            match tokens_of(&row) {
                Some(t) => outln!("context: {} tokens ({t})", kfmt(t)),
                None => outln!("context: unknown (no transcript with usage)"),
            }
            outln!("window: {}", kfmt(lim.window));
            outln!("soft: {}", opt_k(lim.soft));
            outln!("hard: {}", opt_k(lim.hard));
        }
        None => {
            let mut lines = Vec::new();
            for row in reg.rows().iter().filter(|r| !r.is_ended()) {
                let Some(t) = tokens_of(row) else { continue };
                let class = sessions::class_of(ctx, &row.name);
                let lim = class.limits();
                lines.push(format!("| {} | {} | {} | {} | {} |", row.name, class.name(), kfmt(t), opt_k(lim.soft), opt_k(lim.hard)));
            }
            if lines.is_empty() {
                out::line("no registered chat has a transcript");
            } else {
                outln!("| name | class | context | soft | hard |");
                outln!("|---|---|---|---|---|");
                for l in lines {
                    out::line(&l);
                }
            }
        }
    }
    Ok(0)
}
