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
/// The most words one print holds; the oldest lines are left out first.
pub const WORD_CAP: usize = 6000;
/// A tool call prints its name and this many characters of what it was given.
pub const TOOL_CHARS: usize = 120;
/// The fields that say most about a call, in the order they are looked for.
const SALIENT: [&str; 8] = ["command", "file_path", "path", "pattern", "url", "query", "description", "prompt"];

/// RFC 3339 (`2026-11-15T23:12:00Z`), or a local time: `2026-11-15 23:12`, with seconds or a `T`.
pub fn parse_around(text: &str) -> Option<DateTime<Utc>> {
    let t = text.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(t) {
        return Some(d.with_timezone(&Utc));
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

/// The lines one record prints: user text, assistant text, one line per tool call. Tool output,
/// thinking, sidechains and meta records print nothing.
fn record_lines(rec: &Value) -> Vec<String> {
    if rec.get("isSidechain").and_then(Value::as_bool) == Some(true) || rec.get("isMeta").and_then(Value::as_bool) == Some(true) {
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

fn stamp_of(line: &str) -> Option<DateTime<Utc>> {
    const KEY: &str = "\"timestamp\":\"";
    let at = line.find(KEY)? + KEY.len();
    let end = at + line[at..].find('"')?;
    DateTime::parse_from_rfc3339(&line[at..end]).ok().map(|d| d.with_timezone(&Utc))
}

/// The conversation in `lo..=hi` of a transcript's text, oldest first, as `[local time] line` blocks,
/// and how many lines the cap left out.
pub fn window(reader: impl BufRead, lo: DateTime<Utc>, hi: DateTime<Utc>, cap: usize) -> (Vec<String>, usize) {
    let mut blocks: Vec<(usize, String)> = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let Some(at) = stamp_of(&line) else { continue };
        if at < lo || at > hi {
            continue;
        }
        let Ok(rec) = serde_json::from_str::<Value>(&line) else { continue };
        for l in record_lines(&rec) {
            let text = format!("[{}] {l}", local_minute(&at));
            blocks.push((text.split_whitespace().count(), text));
        }
    }
    let mut total: usize = blocks.iter().map(|b| b.0).sum();
    let mut left_out = 0;
    while total > cap && blocks.len() > 1 {
        total -= blocks.remove(0).0;
        left_out += 1;
    }
    if total > cap {
        // One block alone is over the cap: keep its newest words.
        let words: Vec<&str> = blocks[0].1.split_whitespace().collect();
        let kept = words[words.len() - cap..].join(" ");
        blocks[0].1 = kept;
    }
    (blocks.into_iter().map(|b| b.1).collect(), left_out)
}

pub fn cmd_transcript(ctx: &Ctx, name: &str, around: &str, minutes: Option<i64>) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let Some(row) = reg.find_name(name) else {
        out::err_line(&format!("af session transcript: no registry row for {name}; af session list shows the names"));
        return Ok(2);
    };
    let Some(center) = parse_around(around) else {
        out::err_line(&format!("af session transcript: --around is a time such as '2026-11-15 23:12' (local) or 2026-11-15T23:12:00Z, got '{around}'"));
        return Ok(2);
    };
    let minutes = minutes.unwrap_or(DEFAULT_MINUTES);
    if minutes < 1 {
        out::err_line("af session transcript: --minutes is a whole number of minutes, 1 or more");
        return Ok(2);
    }
    let Some(path) = crate::reach::transcript_path(&row.session_id) else {
        out::err_line(&format!("af session transcript: no transcript on this machine for {name} (session {})", row.session_id));
        return Ok(2);
    };
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            out::err_line(&format!("af session transcript: cannot read {path}: {e}"));
            return Ok(2);
        }
    };
    let (lo, hi) = (center - Duration::minutes(minutes), center + Duration::minutes(minutes));
    let (lines, left_out) = window(std::io::BufReader::new(file), lo, hi, WORD_CAP);
    outln!("af session transcript: {name}, {} to {} (local), {minutes} minutes either side of {}", local_minute(&lo), local_minute(&hi), local_minute(&center));
    if lines.is_empty() {
        outln!("nothing said or done in that window.");
        return Ok(0);
    }
    if left_out > 0 {
        outln!("({left_out} earlier lines left out to stay under {WORD_CAP} words; narrow --minutes to see them)");
    }
    for l in lines {
        outln!("{l}");
    }
    Ok(0)
}
