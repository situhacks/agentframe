//! `af session reach <name>`: can a message reach this chat and start a turn, and did it.
//!
//! `SendMessage` answers `success: true` for a message the receiver will hold or drop, so its
//! result proves nothing about a turn. The state comes from the local roster (`claude agents
//! --json`), the permission mode each chat last ran in (its transcript), and the user-level
//! `crossSessionInbound` setting. Delivery comes from the target's transcript: a record its
//! Claude read. The line printed names what to do next, so the rule lives in the button.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::outln;
use crate::pyos;
use crate::sessions::{self, Registry, Row};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Seek, SeekFrom};

/// Bytes read from the end of a transcript to find the last permission mode.
const MODE_TAIL: u64 = 512 * 1024;
/// The most a context-size read takes from the end of a transcript before giving up.
const CONTEXT_TAIL_MAX: u64 = 8 * 1024 * 1024;
/// The shortest `--confirm` phrase: long enough to identify one message, not a common word.
pub const MIN_PHRASE: usize = 12;
const OPEN_TAG: &str = "<cross-session-message";
const CLOSE_TAG: &str = "</cross-session-message>";
/// The connector tools that start an order or change a running flow. A resumed chat keeps its
/// connectors, so a turn nobody is watching is denied these by name.
pub const ORDER_TOOLS: [&str; 4] = [
    "mcp__claude_ai_Questrade__create_order_instruction",
    "mcp__claude_ai_Questrade__create_option_instruction",
    "mcp__claude_ai_Questrade__create_flow",
    "mcp__claude_ai_Questrade__update_flow_status",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    /// live-idle, live-busy, live-waiting, holding, refusing, closed, unknown.
    pub state: &'static str,
    pub line: String,
}

/// The inputs a verdict needs, so the decision is testable without a running chat.
#[derive(Debug, Clone, Default)]
pub struct Facts<'a> {
    pub name: &'a str,
    pub session_id: &'a str,
    /// The roster status: Some("idle"), Some("busy"), Some("shell"), Some("absent"), Some("unknown")
    /// for an entry with no status; None when no roster.
    pub roster: Option<&'a str>,
    pub target_mode: Option<&'a str>,
    pub sender_mode: Option<&'a str>,
    /// `crossSessionInbound` from the user settings.
    pub inbound: Option<&'a str>,
    /// The time this reading was taken, for the `--since` the line prints.
    pub since: &'a str,
}

/// Bypass and everything else are the two classes the receiving session compares.
fn is_bypass(mode: &str) -> bool {
    mode == "bypassPermissions"
}

/// The receiver holds by default when the two chats sit in different permission classes.
fn classes_differ(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if is_bypass(a) != is_bypass(b))
}

pub fn verdict(f: &Facts) -> Reach {
    let (name, id) = (f.name, f.session_id);
    let resume = format!(
        "Start a turn with Bash run_in_background: claude -p \"<message>\" --resume {id} --permission-mode acceptEdits --disallowedTools {} --allowedTools \"<what the job runs>\"; headless, anything else is denied. Its exit notifies you, and its transcript is the chat's own.",
        ORDER_TOOLS.join(" ")
    );
    let status = match f.roster {
        Some(s) => s,
        None => {
            return Reach {
                state: "unknown",
                line: format!("{name}: unknown, the claude agents roster is unavailable. Say the state is unknown; do not say a message started a turn."),
            }
        }
    };
    // A chat running a background shell is working, like a busy one.
    let status = if status == "shell" { "busy" } else { status };
    if status == "unknown" {
        return Reach {
            state: "unknown",
            line: format!("{name}: running, the claude agents roster lists it with no status, so whether it is idle or busy is unknown. Say the state is unknown; do not say a message started a turn."),
        };
    }
    if !matches!(status, "idle" | "busy" | "waiting") {
        return Reach {
            state: "closed",
            line: format!("{name}: closed, not running on this machine. SendMessage will refuse it. {resume}"),
        };
    }
    if f.inbound == Some("refuse") {
        return Reach {
            state: "refusing",
            line: format!("{name}: refusing, crossSessionInbound is refuse in the user settings. A message is dropped. Tell him; the setting is his."),
        };
    }
    let held = f.inbound != Some("accept") && (f.inbound == Some("hold") || classes_differ(f.target_mode, f.sender_mode));
    if held {
        let why = if f.inbound == Some("hold") {
            "crossSessionInbound is hold in the user settings".to_string()
        } else {
            format!(
                "it runs in {} and this chat in {}",
                f.target_mode.unwrap_or("?"),
                f.sender_mode.unwrap_or("?")
            )
        };
        return Reach {
            state: "holding",
            line: format!(
                "{name}: holding, {why}. A message waits unread and, in a VS Code chat, expires after about five minutes; SendMessage still answers success. Do not send. Tell him it needs the modes to match, or crossSessionInbound accept in his /config."
            ),
        };
    }
    let unknown_mode = if f.target_mode.is_none() || f.sender_mode.is_none() {
        " Permission mode unknown, so a hold is possible; watch for a held notice."
    } else {
        ""
    };
    let confirm = format!(
        "Then af session reach {name} --confirm \"<{MIN_PHRASE} or more characters copied from the message>\" --since {} before you say it arrived.",
        f.since
    );
    match status {
        "busy" => Reach {
            state: "live-busy",
            line: format!("{name}: live, busy. SendMessage lands between its tool calls, in the turn it is running; notify_when_idle tells you when it finishes.{unknown_mode} {confirm}"),
        },
        "idle" => Reach {
            state: "live-idle",
            line: format!("{name}: live, idle. SendMessage starts a new turn there.{unknown_mode} {confirm}"),
        },
        _ => Reach {
            state: "live-waiting",
            line: format!("{name}: live, waiting on him at a permission prompt or a question. A message reaches it only after he answers; tell him."),
        },
    }
}

// ------------------------------------------------------------------ the machine's files

pub fn config_dir() -> Option<String> {
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !d.is_empty() {
            return Some(d);
        }
    }
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(pyos::join(&home, ".claude"))
}

/// `<config>/projects/<any>/<session id>.jsonl`.
pub fn transcript_path(session_id: &str) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }
    let projects = pyos::join(&config_dir()?, "projects");
    let name = format!("{session_id}.jsonl");
    for entry in std::fs::read_dir(&projects).ok()?.flatten() {
        let p = entry.path().join(&name);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

/// The last 512 KB of a transcript (all of it when shorter).
pub fn tail(path: &str) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len > MODE_TAIL {
        f.seek(SeekFrom::Start(len - MODE_TAIL)).ok()?;
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// The permission mode of the transcript's last record that names one.
pub fn last_mode(path: &str) -> Option<String> {
    let text = tail(path)?;
    let re = crate::fm::rx(r#""permissionMode":"([A-Za-z]+)""#);
    re.captures_iter(&text).last().map(|c| c[1].to_string())
}

/// The size and models of the last main-chain assistant line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub tokens: u64,
    /// `message.model`: the model that answered, never with a bracket suffix.
    pub model: Option<String>,
    /// The line's top-level `requestedModel`: the id the chat asked for, with `[1m]` when it ran
    /// on the large window.
    pub requested: Option<String>,
}

/// The last main-chain assistant line in these lines: its context tokens (input plus cache read
/// plus cache creation) and `message.model`. Sidechain lines, lines with no usage, and lines
/// that do not parse (a torn tail, a cut head) are skipped; an all-zero usage is an error stub,
/// not a size.
pub fn last_usage(text: &str) -> Option<Usage> {
    match scan_usage(text, false) {
        Scan::Usage(u) => Some(u),
        _ => None,
    }
}

/// What a transcript's lines say about the chat's current size.
enum Scan {
    Usage(Usage),
    /// A `compact_boundary` line comes after the last main-chain assistant line, so the usage
    /// before it describes a context that no longer exists.
    AfterCompact,
    Nothing,
}

fn is_boundary(rec: &Value) -> bool {
    rec.get("type").and_then(Value::as_str) == Some("system")
        && rec.get("subtype").and_then(Value::as_str) == Some("compact_boundary")
        && rec.get("isSidechain").and_then(Value::as_bool) != Some(true)
}

fn scan_usage(text: &str, stop_at_boundary: bool) -> Scan {
    for line in text.rsplit('\n') {
        let boundary = stop_at_boundary && line.contains("compact_boundary");
        if !boundary && !line.contains("\"usage\"") {
            continue;
        }
        let rec: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if boundary && is_boundary(&rec) {
            return Scan::AfterCompact;
        }
        if rec.get("type").and_then(Value::as_str) != Some("assistant") || rec.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let usage = match rec.pointer("/message/usage") {
            Some(u) => u,
            None => continue,
        };
        let tokens: u64 = ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"]
            .iter()
            .map(|k| usage.get(*k).and_then(Value::as_u64).unwrap_or(0))
            .sum();
        if tokens > 0 {
            let model = rec.pointer("/message/model").and_then(Value::as_str).map(str::to_string);
            let requested = rec.get("requestedModel").and_then(Value::as_str).map(str::to_string);
            return Scan::Usage(Usage { tokens, model, requested });
        }
    }
    Scan::Nothing
}

pub fn last_context_tokens(text: &str) -> Option<u64> {
    last_usage(text).map(|u| u.tokens)
}

/// The chat's size as its transcript shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextSize {
    Known(Usage),
    /// The chat was compacted after its last reply: its size is unknown until the next reply.
    AfterCompact,
    /// No assistant line with usage.
    Unknown,
}

/// The chat's current size from its transcript: the last 512 KB, then up to 8 MB when that tail
/// holds no assistant line with usage and no compaction.
pub fn context_size(path: &str) -> ContextSize {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ContextSize::Unknown,
    };
    let Ok(len) = f.metadata().map(|m| m.len()) else { return ContextSize::Unknown };
    let mut tail = MODE_TAIL;
    loop {
        let from = len.saturating_sub(tail);
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(from)).is_err() || (&mut f).take(tail).read_to_end(&mut buf).is_err() {
            return ContextSize::Unknown;
        }
        match scan_usage(&String::from_utf8_lossy(&buf), true) {
            Scan::Usage(u) => return ContextSize::Known(u),
            Scan::AfterCompact => return ContextSize::AfterCompact,
            Scan::Nothing => {}
        }
        if from == 0 || tail >= CONTEXT_TAIL_MAX {
            return ContextSize::Unknown;
        }
        tail = CONTEXT_TAIL_MAX;
    }
}

/// The chat's current usage; None when its size is unknown, a compaction after its last reply
/// included.
pub fn context_usage(path: &str) -> Option<Usage> {
    match context_size(path) {
        ContextSize::Known(u) => Some(u),
        _ => None,
    }
}

pub fn context_tokens(path: &str) -> Option<u64> {
    context_usage(path).map(|u| u.tokens)
}

/// `modelSettings.<model>.autoCompactWindow` from the user-level Claude settings; a bracket
/// suffix on the model id (`[1m]`) is ignored.
pub fn auto_compact_window(model: &str) -> Option<u64> {
    let text = std::fs::read_to_string(pyos::join(&config_dir()?, "settings.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let id = model.split('[').next().unwrap_or(model);
    v.pointer("/modelSettings")?.get(id)?.get("autoCompactWindow")?.as_u64()
}

fn inbound_setting() -> Option<String> {
    let path = pyos::join(&config_dir()?, "settings.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok()?.get("crossSessionInbound")?.as_str().map(str::to_string)
}

/// The text of a record that delivered an inbound message to the chat's Claude. A new turn is a
/// `user` record whose origin is a peer. Mid-turn the message is queued at once (`enqueue`, then
/// an attachment) and read at the next tool round, a `remove` with the reason below. Everything
/// else that can hold the tag (a tool result, a pasted prompt, a held message) delivered nothing.
fn delivered_text(rec: &Value) -> Option<&str> {
    match rec.get("type")?.as_str()? {
        "user" if rec.pointer("/origin/kind")?.as_str()? == "peer" => rec.pointer("/message/content")?.as_str(),
        "queue-operation" if rec.get("operation")?.as_str()? == "remove" && rec.get("reason")?.as_str()? == "absorbed_mid_turn" => {
            rec.get("content")?.as_str()
        }
        _ => None,
    }
}

/// What sits between the message tags, so a phrase cannot match the sender's name or address.
fn message_body(text: &str) -> Option<&str> {
    let open = text.find(OPEN_TAG)?;
    let start = open + text[open..].find('>')? + 1;
    let end = text[start..].rfind(CLOSE_TAG)?;
    Some(&text[start..start + end])
}

/// The UTC timestamp of the first record, at or after `since`, that delivered a message holding
/// this phrase; None when there is none, or the phrase is too short to identify a message.
pub fn delivered_at(path: &str, phrase: &str, since: DateTime<Utc>) -> Option<String> {
    if phrase.trim().chars().count() < MIN_PHRASE {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains(OPEN_TAG) {
            continue;
        }
        let rec: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let holds = delivered_text(&rec).and_then(message_body).is_some_and(|b| b.contains(phrase.trim()));
        if !holds {
            continue;
        }
        let at = rec.get("timestamp").and_then(Value::as_str).unwrap_or("");
        if parse_time(at).is_some_and(|t| t >= since) {
            return Some(at.to_string());
        }
    }
    None
}

fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text.trim()).ok().map(|t| t.with_timezone(&Utc))
}

fn local_minute(utc: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(utc)
        .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| utc.to_string())
}

// ------------------------------------------------------------------ the verb

pub fn row_or_refuse(reg: &Registry, name: &str) -> R<Row> {
    match reg.find_name(name) {
        Some(r) => Ok(r),
        None => die!(
            "no registry row for {name}. A chat he opened and never registered is not reachable by name here; af session list shows the names"
        ),
    }
}

/// Exit 0 with the state printed; with --confirm, 0 when delivered and 3 when not yet.
pub fn cmd_reach(ctx: &Ctx, name: &str, from: Option<&str>, confirm: Option<&str>, since: Option<&str>, as_json: bool) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let row = row_or_refuse(&reg, name)?;
    let path = transcript_path(&row.session_id);

    if let Some(phrase) = confirm {
        let phrase = phrase.trim();
        if phrase.chars().count() < MIN_PHRASE {
            die!("--confirm needs at least {MIN_PHRASE} characters copied from the message you sent, got '{phrase}'; a short phrase also matches older messages");
        }
        let Some(stamp) = since else {
            die!("--confirm needs --since <time>, so an older message cannot count; af session reach {name} prints the value, and run it before you send");
        };
        let Some(since) = parse_time(stamp) else {
            die!("--since is an RFC 3339 time such as 2026-11-05T04:25:00Z, got '{stamp}'");
        };
        let at = path.as_deref().and_then(|p| delivered_at(p, phrase, since));
        return Ok(match at {
            Some(ts) => {
                if as_json {
                    outln!("{}", json!({"name": name, "delivered": true, "at": ts}));
                } else {
                    outln!("{name}: delivered, its Claude read the message at {}.", local_minute(&ts));
                }
                0
            }
            None => {
                if as_json {
                    outln!("{}", json!({"name": name, "delivered": false}));
                } else {
                    outln!("{name}: not delivered. Its transcript has no record of that phrase reaching its Claude. Say it has not arrived; a busy chat reads it at its next tool round, so check once more before calling it lost.");
                }
                3
            }
        });
    }

    let sender = match from {
        Some(f) => Some(row_or_refuse(&reg, f)?),
        None => reg.rows().into_iter().find(sessions::is_main_chat),
    };
    let target_mode = path.as_deref().and_then(last_mode);
    let sender_mode = sender.as_ref().and_then(|s| transcript_path(&s.session_id)).as_deref().and_then(last_mode);
    let inbound = inbound_setting();
    let roster = sessions::claude_roster();
    let status = roster.as_ref().map(|r| sessions::join_roster(std::slice::from_ref(&row), r).remove(0));
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let reach = verdict(&Facts {
        name,
        session_id: &row.session_id,
        roster: status.as_deref(),
        target_mode: target_mode.as_deref(),
        sender_mode: sender_mode.as_deref(),
        inbound: inbound.as_deref(),
        since: &now,
    });
    if as_json {
        outln!(
            "{}",
            json!({"name": name, "state": reach.state, "session_id": row.session_id, "target_mode": target_mode, "sender_mode": sender_mode, "since": now, "line": reach.line})
        );
    } else {
        outln!("{}", reach.line);
    }
    Ok(0)
}
