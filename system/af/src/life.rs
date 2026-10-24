//! The life layer's shared mechanics: newline-preserving text files, local ISO-minute
//! clocks, the month log, the index-age line, backlog counts, and the doctor checks for
//! `workspace/life/` and `system/sessions/`. Every file here is plain Markdown edited as
//! text; a write never changes a line the verb does not own.

use crate::core::Ctx;
use crate::error::{crash, CrashKind, R};
use crate::fm::{self, rx};
use crate::pyos::{self, join_all};
use chrono::{NaiveDateTime, Timelike};

pub const LIFE_CAP_DEFAULT: usize = 200;
pub const INDEX_STALE_HOURS: i64 = 24;
pub const LOG_KINDS: [&str; 5] = ["closed", "decision", "event", "project_closed", "session_ended"];

// ------------------------------------------------------------------ paths

/// A vault-relative path (forward slashes) as an absolute path under the root.
pub fn vpath(ctx: &Ctx, rel: &str) -> String {
    let parts: Vec<&str> = std::iter::once(ctx.root.as_str()).chain(rel.split('/').filter(|p| !p.is_empty())).collect();
    join_all(&parts)
}

pub fn life_md(ctx: &Ctx) -> String {
    vpath(ctx, "workspace/life/life.md")
}

pub fn month_log_rel(month: &str) -> String {
    format!("workspace/life/log/{month}.md")
}

// ------------------------------------------------------------------ text files

/// A text file read with its newline style and BOM remembered, so a rewrite leaves every
/// line it did not touch byte-identical.
#[derive(Debug, Clone)]
pub struct TextFile {
    /// The content with `\n` line ends.
    pub text: String,
    pub crlf: bool,
    pub bom: bool,
}

impl TextFile {
    pub fn new(text: &str) -> TextFile {
        TextFile { text: text.to_string(), crlf: false, bom: false }
    }

    /// None when the file does not exist.
    pub fn load(path: &str) -> R<Option<TextFile>> {
        if !pyos::isfile(path) {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(|e| pyos::os_error(&e, path))?;
        let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let body = if bom { &bytes[3..] } else { &bytes[..] };
        let raw = match std::str::from_utf8(body) {
            Ok(s) => s,
            Err(_) => return Err(crash(CrashKind::Decode, format!("UnicodeDecodeError: {path} is not UTF-8"))),
        };
        Ok(Some(TextFile { crlf: raw.contains("\r\n"), text: pyos::universal_newlines(raw), bom }))
    }

    pub fn save(&self, path: &str) -> R<()> {
        let dir = pyos::dirname(path);
        if !dir.is_empty() {
            pyos::makedirs(&dir)?;
        }
        let mut out = if self.crlf { self.text.replace('\n', "\r\n") } else { self.text.clone() };
        if self.bom {
            out.insert(0, '\u{feff}');
        }
        pyos::write(path, &out)
    }

    /// The lines without their ends (a final newline does not make an empty last line).
    pub fn lines(&self) -> Vec<String> {
        if self.text.is_empty() {
            return Vec::new();
        }
        let t = self.text.strip_suffix('\n').unwrap_or(&self.text);
        t.split('\n').map(|s| s.to_string()).collect()
    }

    /// Replace the content with these lines, each ended by a newline.
    pub fn set_lines(&mut self, lines: &[String]) {
        self.text = if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) };
    }

    /// Append one line, adding the missing newline before it when the file lacks one.
    pub fn push_line(&mut self, line: &str) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        self.text.push_str(line);
        self.text.push('\n');
    }
}

// ------------------------------------------------------------------ clocks

pub fn now() -> NaiveDateTime {
    floor_minute(chrono::Local::now().naive_local())
}

pub fn floor_minute(t: NaiveDateTime) -> NaiveDateTime {
    t.with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap_or(t)
}

/// Local ISO minutes: `2026-10-25T09:12`.
pub fn fmt_min(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M").to_string()
}

pub fn now_min() -> String {
    fmt_min(now())
}

/// A local timestamp: ISO minutes, with seconds, or with a space; a bare date is midnight.
pub fn parse_ts(s: &str) -> Option<NaiveDateTime> {
    let s = fm::clean_value(s);
    let s = s.trim_matches('\'');
    for f in ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(t) = NaiveDateTime::parse_from_str(s, f) {
            return Some(t);
        }
    }
    // An offset-carrying stamp converts to local time.
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(t.with_timezone(&chrono::Local).naive_local());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0))
}

/// A file's mtime in local time, floored to the minute (the precision of `written`).
pub fn mtime_min(path: &str) -> Option<NaiveDateTime> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(floor_minute(chrono::DateTime::<chrono::Local>::from(t).naive_local()))
}

/// The newest mtime of a file, or of any file under a directory (`.git` skipped).
pub fn newest_mtime(path: &str, skip: Option<&str>) -> Option<NaiveDateTime> {
    let skip = skip.map(pyos::abspath);
    let mut best: Option<NaiveDateTime> = None;
    for e in walkdir::WalkDir::new(path).into_iter().filter_entry(|e| e.file_name() != ".git").flatten() {
        if !e.file_type().is_file() {
            continue;
        }
        let p = e.path().to_string_lossy().into_owned();
        if skip.as_deref() == Some(pyos::abspath(&p).as_str()) {
            continue;
        }
        if let Some(t) = mtime_min(&p) {
            best = Some(best.map_or(t, |b| b.max(t)));
        }
    }
    best
}

// ------------------------------------------------------------------ the month log

/// Append `- YYYY-MM-DD HH:MM · <kind>: <text>` to this month's log; the log's rel path.
pub fn append_log(ctx: &Ctx, kind: &str, text: &str) -> R<String> {
    let n = chrono::Local::now();
    let rel = month_log_rel(&n.format("%Y-%m").to_string());
    let path = vpath(ctx, &rel);
    let mut tf = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
    tf.push_line(&format!("- {} · {kind}: {}", n.format("%Y-%m-%d %H:%M"), one_line(text)));
    tf.save(&path)?;
    Ok(rel)
}

pub fn one_line(s: &str) -> String {
    pyos::py_split_ws(s).join(" ")
}

// ------------------------------------------------------------------ surfaces read by brief and bundle

/// (body, body line count, cap) of `life.md`; None when it does not exist.
pub fn life_body(ctx: &Ctx) -> R<Option<(String, usize, usize)>> {
    let tf = match TextFile::load(&life_md(ctx))? {
        Some(t) => t,
        None => return Ok(None),
    };
    let (fmt, body) = match fm::split_fm_optional(&tf.text) {
        Some((f, b)) => (Some(f), b),
        None => (None, tf.text.clone()),
    };
    let cap = fmt
        .as_deref()
        .and_then(|f| fm::get_scalar(f, "cap_lines"))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(LIFE_CAP_DEFAULT);
    let lines = pyos::py_splitlines(&body).len();
    Ok(Some((body, lines, cap)))
}

/// The ids of open `system/backlog.md` rows; None when there is no backlog.
pub fn backlog_open_ids(ctx: &Ctx) -> Option<Vec<String>> {
    let tf = TextFile::load(&vpath(ctx, "system/backlog.md")).ok()??;
    let head = rx(r"\A(?:## |- id: )(BB-[0-9A-Za-z-]+)");
    let done = rx(r"(?i)\A\s*-?\s*status:\s*done\b");
    let mut ids = Vec::new();
    let mut cur: Option<(String, bool)> = None;
    for line in tf.lines() {
        if let Some(c) = head.captures(&line) {
            if let Some((id, d)) = cur.take() {
                if !d {
                    ids.push(id);
                }
            }
            cur = Some((c[1].to_string(), false));
        } else if line.starts_with("## ") || line.starts_with("# ") {
            if let Some((id, d)) = cur.take() {
                if !d {
                    ids.push(id);
                }
            }
        } else if done.is_match(&line) {
            if let Some(c) = cur.as_mut() {
                c.1 = true;
            }
        }
    }
    if let Some((id, d)) = cur {
        if !d {
            ids.push(id);
        }
    }
    Some(ids)
}

/// `index: ...` when the retrieval index is more than a day old (the `vault.db` mtime).
pub fn index_age_line(ctx: &Ctx) -> Option<String> {
    let db = vpath(ctx, "system/index/vault.db");
    let t = std::fs::metadata(&db).ok()?.modified().ok()?;
    let age = chrono::Local::now().naive_local() - chrono::DateTime::<chrono::Local>::from(t).naive_local();
    if age.num_hours() < INDEX_STALE_HOURS {
        return None;
    }
    Some(format!("index: last refresh {}d old; run af index update", age.num_hours() / 24))
}

// ------------------------------------------------------------------ doctor

/// Remotes configured on the private layer's repo: (name, url).
pub fn workspace_remotes(ctx: &Ctx) -> Vec<(String, String)> {
    let dotgit = vpath(ctx, "workspace/.git");
    let config = if pyos::isdir(&dotgit) {
        pyos::join(&dotgit, "config")
    } else if pyos::isfile(&dotgit) {
        // A linked checkout: `.git` is a file naming the real git dir.
        let text = std::fs::read_to_string(&dotgit).unwrap_or_default();
        match text.lines().find_map(|l| l.strip_prefix("gitdir:")) {
            Some(g) => {
                let g = g.trim();
                let g = if pyos::isabs(g) { g.to_string() } else { pyos::join(&vpath(ctx, "workspace"), g) };
                let common = pyos::join(&g, "commondir");
                let base = match std::fs::read_to_string(&common) {
                    Ok(c) => pyos::join(&g, c.trim()),
                    Err(_) => g,
                };
                pyos::join(&base, "config")
            }
            None => return Vec::new(),
        }
    } else {
        return Vec::new();
    };
    let text = std::fs::read_to_string(&config).unwrap_or_default();
    let sect = rx(r#"\A\s*\[remote\s+"([^"]*)"\]"#);
    let url = rx(r"\A\s*url\s*=\s*(.*?)\s*\z");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if let Some(c) = sect.captures(line) {
            cur = Some(c[1].to_string());
            out.push((c[1].to_string(), String::new()));
        } else if line.trim_start().starts_with('[') {
            cur = None;
        } else if let (Some(_), Some(c)) = (&cur, url.captures(line)) {
            if let Some(last) = out.last_mut() {
                if last.1.is_empty() {
                    last.1 = c[1].to_string();
                }
            }
        }
    }
    out
}

/// The life-layer and sessions checks `af doctor` adds to the system surfaces.
pub fn doctor_checks(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let (mut issues, mut notes) = (Vec::new(), Vec::new());

    if let Some((_, lines, cap)) = life_body(ctx)? {
        if lines > cap {
            issues.push(format!("workspace/life/life.md: body {lines} lines, over its cap of {cap}; cut it before adding"));
        } else if lines * 10 >= cap * 9 {
            notes.push(format!("workspace/life/life.md: body {lines} of {cap} lines (90%+); cut stale facts soon"));
        }
    }

    if let Some(doc) = crate::open::OpenDoc::load(ctx)? {
        for (lineno, text, why) in &doc.bad {
            issues.push(format!("{}:{lineno}: {why}: {text}", crate::open::OPEN_REL));
        }
    }

    let reg = crate::sessions::Registry::load(ctx)?;
    let now = now();
    for row in reg.rows() {
        if crate::sessions::effective_status(&row, now) != "idle" {
            continue;
        }
        if let Some(seen) = parse_ts(&row.last_seen) {
            let days = (now - seen).num_days();
            if days >= crate::sessions::IDLE_NOTE_DAYS {
                notes.push(format!(
                    "{}: '{}' idle {days}d (last seen {}); end it (af session end {} --reason other) or resume it",
                    crate::sessions::REGISTRY_REL,
                    row.name,
                    row.last_seen,
                    row.name
                ));
            }
        }
    }

    for (name, url) in workspace_remotes(ctx) {
        issues.push(format!(
            "workspace/.git has remote '{name}' ({url}); the private layer keeps no remote (git -C workspace remote remove {name})"
        ));
    }

    notes.extend(crate::sessions::stale_handoff_notes(ctx)?);
    Ok((issues, notes))
}
