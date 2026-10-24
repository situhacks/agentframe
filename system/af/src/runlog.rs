//! The button run log (system/logs/af-runs.jsonl) and the doctor note that reads it.

use crate::core::{read, Ctx};
use crate::fm::rx;
use crate::pyos;
use serde_json::Value;
use std::io::Write;

pub const BUTTON_WINDOW_DAYS: i64 = 14;
pub const READ_ONLY_VERBS: [&str; 4] = ["doctor", "index", "search", "pipe board"];
pub const SUBCOMMAND_VERBS: [&str; 3] = ["index", "pipe", "studio"];

pub fn verb_of(argv: &[String]) -> String {
    let toks: Vec<&String> = argv.iter().filter(|t| !t.starts_with('-')).collect();
    if toks.is_empty() {
        return "?".to_string();
    }
    if SUBCOMMAND_VERBS.contains(&toks[0].as_str()) && toks.len() > 1 {
        return format!("{} {}", toks[0], toks[1]);
    }
    toks[0].clone()
}

/// `json.dumps(s, ensure_ascii=False)` for a string.
pub fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn now_utc_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
}

/// One JSON line per run, keys in the Python's order; a logging failure is swallowed.
pub fn log_run(ctx: &Ctx, verb: &str, argv: &[String], exit_code: i32, refusal: Option<&str>, crash: Option<&str>) {
    let mut row = format!(
        "{{\"ts\": {}, \"verb\": {}, \"argv\": [{}], \"exit\": {}",
        json_str(&now_utc_iso()),
        json_str(verb),
        argv.iter().map(|a| json_str(a)).collect::<Vec<_>>().join(", "),
        exit_code
    );
    if let Some(r) = refusal.filter(|r| !r.is_empty()) {
        row.push_str(&format!(", \"refusal\": {}", json_str(r)));
    }
    if let Some(c) = crash.filter(|c| !c.is_empty()) {
        row.push_str(&format!(", \"crash\": {}", json_str(c)));
    }
    row.push('}');
    row.push_str(crate::out::NL);
    let _ = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(pyos::dirname(&ctx.run_log))?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&ctx.run_log)?;
        f.write_all(row.as_bytes())
    })();
}

/// `datetime.fromisoformat(ts)` for aware timestamps; None for naive or unparsable.
fn parse_aware(ts: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let re = rx(
        r"\A(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2})(?::?(\d{2})(?::?(\d{2})(?:[.,](\d{1,6}))?)?)?)?(?:(Z)|([+-])(\d{2}):?(\d{2})(?::?(\d{2}))?)?\z",
    );
    let c = re.captures(ts)?;
    let num = |i: usize| c.get(i).map(|m| m.as_str().parse::<u32>().unwrap_or(0)).unwrap_or(0);
    let has_offset = c.get(8).is_some() || c.get(9).is_some();
    if !has_offset || c.get(4).is_none() {
        return None;
    }
    let date = chrono::NaiveDate::from_ymd_opt(num(1) as i32, num(2), num(3))?;
    let micros = c
        .get(7)
        .map(|m| {
            let s = format!("{:0<6}", m.as_str());
            s.parse::<u32>().unwrap_or(0)
        })
        .unwrap_or(0);
    let time = chrono::NaiveTime::from_hms_micro_opt(num(4), num(5), num(6), micros)?;
    let secs = if c.get(8).is_some() {
        0
    } else {
        let sign = if &c[9] == "-" { -1 } else { 1 };
        sign * (num(10) as i32 * 3600 + num(11) as i32 * 60 + num(12) as i32)
    };
    let off = chrono::FixedOffset::east_opt(secs)?;
    date.and_time(time).and_local_timezone(off).single()
}

/// Run-log rows at or after `since`, oldest first; unreadable lines are skipped.
pub fn read_runs(ctx: &Ctx, since: chrono::DateTime<chrono::Utc>) -> Vec<serde_json::Map<String, Value>> {
    if !pyos::isfile(&ctx.run_log) {
        return Vec::new();
    }
    let bytes = match std::fs::read(&ctx.run_log) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = pyos::universal_newlines(&String::from_utf8_lossy(&bytes));
    let mut rows = Vec::new();
    for line in text.split_inclusive('\n') {
        let v: Value = match serde_json::from_str(line.trim_end_matches('\n')) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let obj = match v {
            Value::Object(o) => o,
            _ => continue,
        };
        let ts = match obj.get("ts") {
            Some(Value::String(s)) => s.clone(),
            _ => continue,
        };
        match parse_aware(&ts) {
            Some(t) if t >= since => rows.push(obj),
            _ => continue,
        }
    }
    rows
}

/// Python truthiness of a JSON value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// `str(value)` for a JSON-decoded Python value.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => py_repr(other),
    }
}

fn py_repr(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                if f.fract() == 0.0 && f.abs() < 1e16 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                }
            }
        }
        Value::String(s) => pyos::py_repr_str(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", pyos::py_repr_str(k), py_repr(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Open BB-* rows whose text names a state verb.
pub fn open_backlog_rows_naming_af(ctx: &Ctx) -> usize {
    let path = pyos::join_all(&[&ctx.root, "system", "backlog.md"]);
    if !pyos::isfile(&path) {
        return 0;
    }
    let text = match read(&path) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let bytes = text.as_bytes();
    let mut cuts = vec![0usize];
    for (i, _) in text.char_indices() {
        if (i == 0 || bytes[i - 1] == b'\n') && (text[i..].starts_with("- id: BB-") || text[i..].starts_with("## BB-")) && i != 0 {
            cuts.push(i);
        }
    }
    cuts.push(text.len());
    let head = rx(r"\A(- id: |## )BB-");
    let pat = rx(r"\baf (ready|publish|version|draft|adopt|pipe|studio|new-project)\b");
    let done = rx(r"(?mi)^\s*-?\s*status:\s*done\b");
    cuts.windows(2)
        .map(|w| &text[w[0]..w[1]])
        .filter(|r| head.is_match(r) && pat.is_match(r) && !done.is_match(r))
        .count()
}

/// `af session stop` exiting 3 is its HANDOFF_REQUIRED signal, not a refusal.
fn is_handoff_signal(r: &serde_json::Map<String, Value>) -> bool {
    let first_two: Vec<&str> = r
        .get("argv")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).filter(|t| !t.starts_with('-')).take(2).collect())
        .unwrap_or_default();
    first_two == ["session", "stop"] && r.get("exit").and_then(Value::as_i64) == Some(3)
}

/// What the buttons did lately, per state verb, against the open rows that name them.
pub fn button_notes(ctx: &Ctx) -> Vec<String> {
    let since = chrono::Utc::now() - chrono::Duration::days(BUTTON_WINDOW_DAYS);
    let runs = read_runs(ctx, since);
    let mut refusals: Vec<(String, usize)> = Vec::new();
    let mut crashes: Vec<(String, usize)> = Vec::new();
    let mut usage = 0usize;
    let bump = |v: &mut Vec<(String, usize)>, k: &str| {
        if let Some(e) = v.iter_mut().find(|e| e.0 == k) {
            e.1 += 1;
        } else {
            v.push((k.to_string(), 1));
        }
    };
    for r in &runs {
        let verb = r.get("verb").map(py_str).unwrap_or_else(|| "?".to_string());
        let first = verb.split(' ').next().unwrap_or("").to_string();
        if !truthy(r.get("exit")) || READ_ONLY_VERBS.contains(&verb.as_str()) || READ_ONLY_VERBS.contains(&first.as_str()) {
            continue;
        }
        if r.get("refusal") == Some(&Value::String("usage".to_string())) {
            usage += 1;
            continue;
        }
        if is_handoff_signal(r) {
            continue;
        }
        if truthy(r.get("crash")) {
            bump(&mut crashes, &verb);
        } else {
            bump(&mut refusals, &verb);
        }
    }
    if refusals.is_empty() && crashes.is_empty() {
        return Vec::new();
    }
    let fmt = |d: &Vec<(String, usize)>| -> String {
        let mut v = d.clone();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")
    };
    let mut bits = Vec::new();
    if !refusals.is_empty() {
        bits.push(format!("{} refusal(s): {}", refusals.iter().map(|e| e.1).sum::<usize>(), fmt(&refusals)));
    }
    if !crashes.is_empty() {
        bits.push(format!("{} crash(es): {}", crashes.iter().map(|e| e.1).sum::<usize>(), fmt(&crashes)));
    }
    if usage > 0 {
        bits.push(format!("{usage} usage slip(s)"));
    }
    vec![format!(
        "buttons ({BUTTON_WINDOW_DAYS}d): {}. Open BB-* rows naming a button: {}. A refusal you worked around needs a row (system/backlog.md); a slip fixed on the retry does not.",
        bits.join("; "),
        open_backlog_rows_naming_af(ctx)
    )]
}
