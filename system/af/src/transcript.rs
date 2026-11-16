//! `af session transcript <name> --around <time> [--minutes N]`: the conversation around one moment of
//! a registered chat, read from its transcript. A bug row's `Surfaced:` line names the chat and the
//! time; this prints what was said and done then, so the fix answers what happened and not only how
//! the row words it.

use crate::core::Ctx;
use crate::error::R;
use crate::out;
use crate::outln;
use crate::sessions::Registry;
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Utc};
use serde_json::Value;
use std::io::BufRead;

pub const DEFAULT_MINUTES: i64 = 20;
/// The most words one print holds; the oldest words go first.
pub const WORD_CAP: usize = 6000;
/// A tool call prints its name and this many characters of what it was given.
pub const TOOL_CHARS: usize = 120;
/// The fields that say most about a call, in the order they are looked for.
const SALIENT: [&str; 8] = ["command", "file_path", "path", "pattern", "url", "query", "description", "prompt"];

/// RFC 3339 (`2026-11-15T23:12:00Z`, or without seconds: `2026-11-15T23:12Z`), or a local time:
/// `2026-11-15 23:12`, with seconds or a `T`.
pub fn parse_around(text: &str) -> Option<DateTime<Utc>> {
    let t = text.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(t) {
        return Some(d.with_timezone(&Utc));
    }
    if let Ok(d) = DateTime::parse_from_str(t, "%Y-%m-%dT%H:%M%:z") {
        return Some(d.with_timezone(&Utc));
    }
    if let Some(n) = t.strip_suffix('Z').and_then(|u| NaiveDateTime::parse_from_str(u, "%Y-%m-%dT%H:%M").ok()) {
        return Some(Utc.from_utc_datetime(&n));
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(t, fmt) {
            return Local.from_local_datetime(&n).earliest().map(|d| d.with_timezone(&Utc));
        }
    }
    None
}

fn local_minute(t: &DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A tool call as one line: its name and the first characters of its salient input (else its JSON).
fn tool_line(block: &Value) -> String {
    let name = block.get("name").and_then(Value::as_str).unwrap_or("?");
    let input = block.get("input");
    let detail = SALIENT
        .iter()
        .find_map(|k| input.and_then(|i| i.get(*k)).and_then(Value::as_str))
        .map(str::to_string)
        .or_else(|| input.filter(|i| !i.is_null()).map(|i| i.to_string()))
        .unwrap_or_default();
    format!("tool: {name} {}", clip(&one_line(&detail), TOOL_CHARS)).trim_end().to_string()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn flag(rec: &Value, key: &str) -> bool {
    rec.get(key).and_then(Value::as_bool) == Some(true)
}

/// The lines one record prints: user text, assistant text, one line per tool call. Tool output,
/// thinking, sidechains, meta records and a compaction's summary print nothing.
fn record_lines(rec: &Value) -> Vec<String> {
    if flag(rec, "isSidechain") || flag(rec, "isMeta") || flag(rec, "isCompactSummary") {
        return Vec::new();
    }
    let kind = rec.get("type").and_then(Value::as_str).unwrap_or("");
    let Some(content) = rec.pointer("/message/content") else { return Vec::new() };
    let mut out = Vec::new();
    match kind {
        "user" => {
            let t = text_of(content);
            if !t.trim().is_empty() {
                out.push(format!("user: {}", t.trim()));
            }
        }
        "assistant" => {
            if let Value::Array(blocks) = content {
                for b in blocks {
                    match b.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let t = b.get("text").and_then(Value::as_str).unwrap_or("").trim();
                            if !t.is_empty() {
                                out.push(format!("assistant: {t}"));
                            }
                        }
                        Some("tool_use") => out.push(tool_line(b)),
                        _ => {}
                    }
                }
            } else {
                let t = text_of(content);
                if !t.trim().is_empty() {
                    out.push(format!("assistant: {}", t.trim()));
                }
            }
        }
        _ => {}
    }
    out
}

/// One printed line: `[local time] user: ...`, its header apart from its body so a trim keeps the header.
struct Block {
    header: String,
    body: String,
}

impl Block {
    fn words(&self) -> usize {
        self.header.split_whitespace().count() + self.body.split_whitespace().count()
    }

    fn text(&self) -> String {
        format!("{} {}", self.header, self.body)
    }
}

pub struct Window {
    pub lines: Vec<String>,
    /// Whole lines the cap dropped, oldest first.
    pub left_out: usize,
    /// Words the cap cut from the front of the oldest line it kept; None when it kept that line whole.
    pub trimmed: Option<usize>,
    /// The first and last timestamps anywhere in the transcript.
    pub first: Option<DateTime<Utc>>,
    pub last: Option<DateTime<Utc>>,
}

/// The conversation in `lo..=hi` of a transcript, oldest first, under `cap` words. Over the cap the oldest
/// words go: whole lines while a line is wholly over, then the front of the line that is partly over.
pub fn window(reader: impl BufRead, lo: DateTime<Utc>, hi: DateTime<Utc>, cap: usize) -> Window {
    let mut blocks: Vec<Block> = Vec::new();
    let (mut first, mut last): (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = (None, None);
    for line in reader.lines().map_while(Result::ok) {
        let Ok(rec) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(at) = rec.get("timestamp").and_then(Value::as_str).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|d| d.with_timezone(&Utc))
        else {
            continue;
        };
        first = Some(first.map_or(at, |f| f.min(at)));
        last = Some(last.map_or(at, |l| l.max(at)));
        if at < lo || at > hi {
            continue;
        }
        for l in record_lines(&rec) {
            let (who, body) = l.split_once(' ').unwrap_or((l.as_str(), ""));
            blocks.push(Block { header: format!("[{}] {who}", local_minute(&at)), body: body.to_string() });
        }
    }
    let mut total: usize = blocks.iter().map(Block::words).sum();
    let mut left_out = 0;
    let mut trimmed = None;
    while total > cap && !blocks.is_empty() {
        let excess = total - cap;
        let body_words: Vec<&str> = blocks[0].body.split_whitespace().collect();
        // Keep the newest words of the oldest line when most of it fits; the marker is counted in what is kept.
        const MARK: usize = 7;
        if body_words.len() > excess + MARK {
            let keep = body_words.len() - excess - MARK;
            let kept = body_words[body_words.len() - keep..].join(" ");
            let cut = body_words.len() - keep;
            blocks[0].body = format!("(trimmed: its first {cut} words left out) {kept}");
            trimmed = Some(cut);
            break;
        }
        total -= blocks.remove(0).words();
        left_out += 1;
    }
    Window { lines: blocks.iter().map(Block::text).collect(), left_out, trimmed, first, last }
}

fn bad(msg: &str) -> R<i32> {
    out::err_line(&format!("af session transcript: {msg}"));
    Ok(2)
}

pub fn cmd_transcript(ctx: &Ctx, name: &str, around: &str, minutes: Option<i64>) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let Some(row) = reg.find_name(name) else {
        return bad(&format!("no registry row for {name}; af session list shows the names"));
    };
    let Some(center) = parse_around(around) else {
        return bad(&format!("--around is a time such as '2026-11-15 23:12' (local) or 2026-11-15T23:12:00Z, got '{around}'"));
    };
    let minutes = minutes.unwrap_or(DEFAULT_MINUTES);
    let span = Duration::try_minutes(minutes).filter(|_| minutes >= 1);
    let (Some(lo), Some(hi)) = (
        span.and_then(|s| center.checked_sub_signed(s)),
        span.and_then(|s| center.checked_add_signed(s)),
    ) else {
        return bad("--minutes is a whole number of minutes, 1 or more, small enough to make a real time either side of --around");
    };
    let Some(path) = crate::reach::transcript_path(&row.session_id) else {
        return bad(&format!("no transcript on this machine for {name} (session {})", row.session_id));
    };
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => return bad(&format!("cannot read {path}: {e}")),
    };
    let w = window(std::io::BufReader::new(file), lo, hi, WORD_CAP);
    outln!("af session transcript: {name}, {} to {} (local), {minutes} minutes either side of {}", local_minute(&lo), local_minute(&hi), local_minute(&center));
    if w.lines.is_empty() {
        match (w.first, w.last) {
            (Some(first), _) if hi < first => outln!(
                "that window is before this chat's transcript begins ({}). A name holds only its latest session, so an earlier session is not readable here.",
                local_minute(&first)
            ),
            (_, Some(last)) if lo > last => outln!("that window is after this chat's last record ({}).", local_minute(&last)),
            _ => outln!("nothing said or done in that window."),
        }
        return Ok(0);
    }
    if w.left_out > 0 || w.trimmed.is_some() {
        outln!(
            "(over {WORD_CAP} words: {} earlier lines left out{}; narrow --minutes to see them)",
            w.left_out,
            w.trimmed.map(|n| format!(", the first {n} words of the oldest line shown cut")).unwrap_or_default()
        );
    }
    for l in w.lines {
        outln!("{l}");
    }
    Ok(0)
}
