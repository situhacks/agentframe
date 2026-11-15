//! The sessions registry (`system/sessions/registry.md`), the `af session` verbs, and the
//! hook handlers the `af hook` dispatcher calls. The registry is one Markdown table owned
//! by these verbs; each verb rewrites only the rows it changes.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::fm::{self, rx};
use crate::life::{self, fmt_min, parse_ts, vpath, TextFile};
use crate::open;
use crate::out;
use crate::page;
use crate::outln;
use crate::pyos;
use chrono::NaiveDateTime;
use clap::Subcommand;
use serde_json::{json, Value};

pub const REGISTRY_REL: &str = "system/sessions/registry.md";
pub const HEADER: &str = "| name | role | session_id | model | started | last_seen | status | handoff |";
pub const SEPARATOR: &str = "|---|---|---|---|---|---|---|---|";
pub const STALE_AFTER_MINUTES: i64 = 120;
pub const IDLE_NOTE_DAYS: i64 = 7;
pub const END_REASONS: [&str; 5] = ["clear", "resume", "logout", "exit", "other"];
pub const NAME_GRAMMAR: &str = "af-main, life/<topic>, project/<slug>, system/<topic>, worker/<slug>";
/// The main chat's name (its VS Code session title) and registry name. Not `main`: SendMessage reserves that word for
/// the sender's own conversation, so side chats could not address it.
pub const MAIN_NAME: &str = "af-main";
/// The main chat's earlier name, still accepted so an existing row keeps working.
pub const LEGACY_MAIN_NAME: &str = "main";
/// The side-chat names of the grammar, as the off-system directive offers them.
pub const SIDE_NAMES: [&str; 3] = ["life/<topic>", "project/<slug>", "system/<topic>"];

// ------------------------------------------------------------------ names

pub fn valid_name(name: &str) -> bool {
    rx(r"\A(?:af-main|main|(?:life|project|system|worker)/[A-Za-z0-9][A-Za-z0-9_.-]*)\z").is_match(name)
}

pub fn role_of(name: &str) -> &'static str {
    match name.split('/').next().unwrap_or("") {
        "project" => "operator",
        "system" => "builder",
        "worker" => "worker",
        _ => "main",
    }
}

/// Whether a row is the main chat: role `main` and no topic (`life/*` shares the role but is
/// a side chat). The chat's name does not matter, so `af-main` and a legacy `main` both are.
pub fn is_main_chat(row: &Row) -> bool {
    row.role == "main" && !row.name.contains('/')
}

/// What a main chat still registered under the reserved name is told at session start. The
/// chat's name is its VS Code session title, so a rename in place moves it; no relaunch.
pub fn main_rename_line(session_id: &str) -> String {
    format!(
        "af: side chats cannot SendMessage a chat named {LEGACY_MAIN_NAME}. Rename this chat to {MAIN_NAME} \
(VS Code: /rename {MAIN_NAME}), then run \"af session register {MAIN_NAME} --session {session_id}\" and \
\"af session end {LEGACY_MAIN_NAME} --reason other\"."
    )
}

pub fn session_rel(name: &str) -> String {
    format!("system/sessions/{name}")
}

pub fn handoff_rel(name: &str) -> String {
    format!("system/sessions/{name}/handoff.md")
}

pub fn brief_rel(name: &str) -> String {
    format!("system/sessions/{name}/brief.md")
}

// ------------------------------------------------------------------ session classes

/// How much context a chat is meant to carry. Not the permission class in `reach`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionClass {
    Main,
    Side,
    Build,
}

/// One class's window and nudge thresholds, in tokens. Every class is nudged: AgentFrame decides per
/// chat when it should compact, and Claude Code's own window is left at its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassLimits {
    pub class: SessionClass,
    pub soft: Option<u64>,
    pub hard: Option<u64>,
}

/// The one table of session classes.
pub const CLASS_LIMITS: [ClassLimits; 3] = [
    ClassLimits { class: SessionClass::Main, soft: Some(350_000), hard: Some(450_000) },
    ClassLimits { class: SessionClass::Side, soft: Some(300_000), hard: Some(400_000) },
    ClassLimits { class: SessionClass::Build, soft: Some(400_000), hard: Some(600_000) },
];

pub const CLASS_NAMES: [&str; 3] = ["main", "side", "build"];

impl SessionClass {
    pub fn name(self) -> &'static str {
        match self {
            SessionClass::Main => "main",
            SessionClass::Side => "side",
            SessionClass::Build => "build",
        }
    }

    pub fn parse(s: &str) -> Option<SessionClass> {
        CLASS_LIMITS.iter().map(|l| l.class).find(|c| c.name() == s.trim())
    }

    pub fn limits(self) -> ClassLimits {
        *CLASS_LIMITS.iter().find(|l| l.class == self).expect("every class has a row")
    }
}

/// The class a name gets with no brief override: the main chat is `main`, `project/*` and
/// `system/*` are `build`, `life/*` and `worker/*` are `side`.
pub fn default_class(name: &str) -> SessionClass {
    match (role_of(name), name.contains('/')) {
        ("main", false) => SessionClass::Main,
        ("builder" | "operator", _) => SessionClass::Build,
        _ => SessionClass::Side,
    }
}

/// A brief's `soft:` or `hard:` value: a number with `k`, 100k to 1000k, in tokens.
fn parse_limit(value: &str) -> Option<u64> {
    let n: u64 = value.trim().strip_suffix('k')?.parse().ok()?;
    (100..=1000).contains(&n).then_some(n * 1000)
}

/// The class limits with a brief's optional `soft:` and `hard:` lines applied, and what is wrong with
/// those lines. A bad value falls back to the class's own; a pair with soft not below hard (either one
/// checked against the other's class value when alone) falls back to the class's pair.
fn override_limits(class: SessionClass, soft: Option<&str>, hard: Option<&str>) -> (ClassLimits, Vec<String>) {
    let base = class.limits();
    let mut problems = Vec::new();
    let mut read = |key: &str, raw: Option<&str>| {
        let raw = raw?;
        let n = parse_limit(raw);
        if n.is_none() {
            problems.push(format!(
                "{key}: '{raw}' is not a limit; use a number with k, 100k to 1000k (the {} class's {} applies until then)",
                class.name(),
                opt_limit(if key == "soft" { base.soft } else { base.hard })
            ));
        }
        n
    };
    let (soft, hard) = (read("soft", soft), read("hard", hard));
    let (s, h) = (soft.or(base.soft), hard.or(base.hard));
    if let (Some(s), Some(h)) = (s, h) {
        if (soft.is_some() || hard.is_some()) && s >= h {
            problems.push(format!(
                "soft {}k is not below hard {}k; the {} class's limits apply until then",
                s / 1000,
                h / 1000,
                class.name()
            ));
            return (base, problems);
        }
    }
    (ClassLimits { soft: s, hard: h, ..base }, problems)
}

fn opt_limit(v: Option<u64>) -> String {
    v.map(|n| format!("{}k", n / 1000)).unwrap_or_else(|| "none".to_string())
}

/// A brief's frontmatter `soft:` and `hard:` values as written, when it has a frontmatter.
fn brief_limit_lines(text: &str) -> (Option<String>, Option<String>) {
    match fm::split_fm_optional(text) {
        Some((bfm, _)) => (fm::get_scalar(&bfm, "soft"), fm::get_scalar(&bfm, "hard")),
        None => (None, None),
    }
}

/// The limits in force for a chat: its class's, with its brief's `soft:` and `hard:` lines applied.
pub fn limits_of(ctx: &Ctx, name: &str, class: SessionClass) -> ClassLimits {
    let text = TextFile::load(&vpath(ctx, &brief_rel(name))).ok().flatten().map(|t| t.text).unwrap_or_default();
    let (soft, hard) = brief_limit_lines(&text);
    override_limits(class, soft.as_deref(), hard.as_deref()).0
}

/// Doctor: a brief whose `class:` line is not one of the three classes, or whose `soft:` or `hard:` line
/// is not a usable limit. Reading falls back to the name default or the class limit, so this is the only
/// place the typo shows.
pub fn bad_class_issues(ctx: &Ctx) -> R<Vec<String>> {
    let root = vpath(ctx, "system/sessions");
    let mut issues = Vec::new();
    if !pyos::isdir(&root) {
        return Ok(issues);
    }
    let mut briefs: Vec<String> = walkdir::WalkDir::new(&root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.file_name() == "brief.md")
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    briefs.sort();
    for b in briefs {
        let Some(tf) = TextFile::load(&b)? else { continue };
        let Some((bfm, _)) = fm::split_fm_optional(&tf.text) else { continue };
        let rel = ctx.rel(&b)?;
        let value = fm::get_scalar(&bfm, "class");
        if let Some(v) = value.as_deref().filter(|v| SessionClass::parse(v).is_none()) {
            issues.push(format!(
                "{rel}: class: '{v}' is not a session class; use one of {} (the name's default applies until then)",
                CLASS_NAMES.join(", ")
            ));
        }
        let name = rel.strip_prefix("system/sessions/").and_then(|r| r.strip_suffix("/brief.md")).unwrap_or("");
        let class = value.as_deref().and_then(SessionClass::parse).unwrap_or_else(|| default_class(name));
        let (soft, hard) = (fm::get_scalar(&bfm, "soft"), fm::get_scalar(&bfm, "hard"));
        for problem in override_limits(class, soft.as_deref(), hard.as_deref()).1 {
            issues.push(format!("{rel}: {problem}"));
        }
    }
    Ok(issues)
}

/// The brief's `class:` line, when it holds one of the three classes.
pub fn brief_class(ctx: &Ctx, name: &str) -> Option<SessionClass> {
    let tf = TextFile::load(&vpath(ctx, &brief_rel(name))).ok()??;
    let (bfm, _) = fm::split_fm_optional(&tf.text)?;
    SessionClass::parse(&fm::get_scalar(&bfm, "class")?)
}

/// A chat's class: its brief's `class:` line, else the default for its name.
pub fn class_of(ctx: &Ctx, name: &str) -> SessionClass {
    brief_class(ctx, name).unwrap_or_else(|| default_class(name))
}

/// Write or update `class: <c>` in a brief's frontmatter, adding the block when it has none.
fn write_brief_class(path: &str, class: SessionClass) -> R<()> {
    let mut tf = TextFile::load(path)?.unwrap_or_else(|| TextFile::new(""));
    tf.text = match fm::split_fm_optional(&tf.text) {
        Some((bfm, body)) => fm::join_fm(fm::upsert_scalar(&bfm, "class", class.name(), None)?.trim_end(), &body),
        None => fm::join_fm(&format!("class: {}", class.name()), &tf.text),
    };
    tf.save(path)
}

// ------------------------------------------------------------------ rows

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub role: String,
    pub session_id: String,
    pub model: String,
    pub started: String,
    pub last_seen: String,
    pub status: String,
    pub handoff: String,
}

impl Row {
    pub fn line(&self) -> String {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            self.name, self.role, self.session_id, self.model, self.started, self.last_seen, self.status, self.handoff
        )
    }

    pub fn parse(line: &str) -> Option<Row> {
        let t = line.trim();
        let inner = t.strip_prefix('|')?.strip_suffix('|')?;
        let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
        if cells.len() != 8 || cells[0].is_empty() || cells[0] == "name" {
            return None;
        }
        let mut it = cells.into_iter();
        let mut next = || it.next().unwrap();
        Some(Row {
            name: next(),
            role: next(),
            session_id: next(),
            model: next(),
            started: next(),
            last_seen: next(),
            status: next(),
            handoff: next(),
        })
    }

    pub fn is_ended(&self) -> bool {
        self.status.starts_with("ended")
    }

    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name, "role": self.role, "session_id": self.session_id, "model": self.model,
            "started": self.started, "last_seen": self.last_seen, "status": self.status, "handoff": self.handoff,
        })
    }

    /// Stamp `last_seen` now; a row that has not ended becomes `live`.
    pub fn touch(&mut self, revive: bool) {
        self.last_seen = life::now_min();
        if revive || !self.is_ended() {
            self.status = "live".to_string();
        }
    }
}

/// `live` within `stale_after` of the last hook touch, `idle` past it, `ended ...` as stored.
pub fn effective_status(row: &Row, now: NaiveDateTime) -> String {
    if row.is_ended() {
        return row.status.clone();
    }
    match parse_ts(&row.last_seen) {
        Some(seen) if (now - seen).num_minutes() <= STALE_AFTER_MINUTES => "live".to_string(),
        _ => "idle".to_string(),
    }
}

// ------------------------------------------------------------------ the registry file

pub struct Registry {
    path: String,
    file: TextFile,
    lines: Vec<String>,
    exists: bool,
}

fn is_separator(line: &str) -> bool {
    rx(r"\A\s*\|[\s:|-]+\|\s*\z").is_match(line) && line.contains('-')
}

impl Registry {
    pub fn load(ctx: &Ctx) -> R<Registry> {
        let path = vpath(ctx, REGISTRY_REL);
        let (file, exists) = match TextFile::load(&path)? {
            Some(f) => (f, true),
            None => (TextFile::new(""), false),
        };
        let lines = file.lines();
        Ok(Registry { path, file, lines, exists })
    }

    fn header_idx(&self) -> Option<usize> {
        self.lines.iter().position(|l| {
            let cells: Vec<&str> = l.trim().trim_matches('|').split('|').map(|c| c.trim()).collect();
            cells.first() == Some(&"name") && cells.len() == 8
        })
    }

    /// (line index, row) for every table row, in file order.
    fn indexed(&self) -> Vec<(usize, Row)> {
        let h = match self.header_idx() {
            Some(h) => h,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for (i, l) in self.lines.iter().enumerate().skip(h + 1) {
            if !l.trim_start().starts_with('|') {
                break;
            }
            if is_separator(l) {
                continue;
            }
            if let Some(r) = Row::parse(l) {
                out.push((i, r));
            }
        }
        out
    }

    pub fn rows(&self) -> Vec<Row> {
        self.indexed().into_iter().map(|(_, r)| r).collect()
    }

    pub fn find_name(&self, name: &str) -> Option<Row> {
        self.rows().into_iter().find(|r| r.name == name)
    }

    /// The row for a session id: the last one that has not ended, else the last one.
    pub fn find_session(&self, id: &str) -> Option<Row> {
        if id.is_empty() {
            return None;
        }
        let rows: Vec<Row> = self.rows().into_iter().filter(|r| r.session_id == id).collect();
        rows.iter().rev().find(|r| !r.is_ended()).or_else(|| rows.last()).cloned()
    }

    /// Replace the row with this name, or append it to the table.
    pub fn upsert(&mut self, row: &Row) {
        let idx = self.indexed();
        if let Some((i, _)) = idx.iter().find(|(_, r)| r.name == row.name) {
            self.lines[*i] = row.line();
            return;
        }
        if let Some((i, _)) = idx.last() {
            self.lines.insert(i + 1, row.line());
            return;
        }
        match self.header_idx() {
            Some(h) => {
                let at = if self.lines.get(h + 1).map(|l| is_separator(l)).unwrap_or(false) { h + 2 } else { h + 1 };
                self.lines.insert(at, row.line());
            }
            None => {
                if self.lines.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
                    self.lines.push(String::new());
                }
                self.lines.push(HEADER.to_string());
                self.lines.push(SEPARATOR.to_string());
                self.lines.push(row.line());
            }
        }
    }

    pub fn save(&mut self) -> R<()> {
        self.file.set_lines(&self.lines);
        self.file.save(&self.path)?;
        self.exists = true;
        Ok(())
    }

    pub fn exists(&self) -> bool {
        self.exists
    }
}

// ------------------------------------------------------------------ handoffs

/// `## Files touched` entries of a handoff body, one path per line.
pub fn files_touched(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("## ") {
            inside = t == "## Files touched";
            continue;
        }
        if !inside || t.is_empty() {
            continue;
        }
        let p = t.trim_start_matches(['-', '*']).trim().trim_matches('`').trim();
        if !p.is_empty() {
            out.push(p.replace('\\', "/"));
        }
    }
    out
}

fn abs_of(ctx: &Ctx, p: &str) -> String {
    if pyos::isabs(p) {
        p.to_string()
    } else {
        vpath(ctx, p)
    }
}

/// Why a chat must write or refresh its handoff before it stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffNeed {
    Missing,
    /// The file has no `written:` line that parses.
    NoWritten,
    /// A listed path (or write-surface path) changed after `written:`.
    Stale { path: String, at: NaiveDateTime, written: NaiveDateTime },
    /// The page has `[fix]` issues (`page::Page::issues`); `[review]` issues never land here.
    Fix(Vec<String>),
}

impl HandoffNeed {
    /// The Stop hook's reason: what is wrong and the edit that fixes it.
    pub fn hook_reason(&self, rel: &str) -> String {
        let now = life::now_min();
        match self {
            HandoffNeed::Missing => format!(
                "Write {rel} before stopping: the file does not exist. It needs the sections State, Next step and Files touched (one path per line) and `written: {now}` (YYYY-MM-DDTHH:MM, local time) in its frontmatter. Doing, To do, Gated, Done, Rules and Open questions are optional; library/schema/handoff.md has the rest."
            ),
            HandoffNeed::NoWritten => format!(
                "Fix {rel} before stopping: it has no `written:` line that parses. Add `written: {now}` (YYYY-MM-DDTHH:MM, local time) to its frontmatter."
            ),
            HandoffNeed::Stale { path, at, written } => format!(
                "Refresh {rel} before stopping: {path} changed at {}, after its `written: {}`. Update the handoff to match, then set `written:` to the current time ({now}).",
                fmt_min(*at),
                fmt_min(*written)
            ),
            HandoffNeed::Fix(issues) => format!(
                "Fix {rel} before stopping: the page has {} issue(s) to fix:
{}
Run `af session tidy`, fix the rest, then set `written:` to the current time ({now}).",
                issues.len(),
                issues.iter().map(|i| format!("- {i}")).collect::<Vec<_>>().join("
")
            ),
        }
    }

    /// The short reason `af session stop` prints after the path.
    pub fn short(&self) -> String {
        let now = life::now_min();
        match self {
            HandoffNeed::Missing => "file missing".to_string(),
            HandoffNeed::NoWritten => format!("no `written:` line; add written: {now}"),
            HandoffNeed::Stale { path, at, written } => format!(
                "stale: {path} changed at {}, after written: {}; refresh the handoff and its written:",
                fmt_min(*at),
                fmt_min(*written)
            ),
            HandoffNeed::Fix(issues) => format!(
                "the page has {} issue(s) to fix, the first: {}; run af session tidy --check for the list",
                issues.len(),
                issues[0]
            ),
        }
    }
}

/// What the chat must fix in its handoff before it stops, if anything.
pub fn handoff_need(ctx: &Ctx, row: &Row) -> R<Option<HandoffNeed>> {
    if row.is_ended() {
        return Ok(None);
    }
    let hpath = vpath(ctx, &handoff_rel(&row.name));
    let tf = match TextFile::load(&hpath)? {
        Some(t) => t,
        None => return Ok(Some(HandoffNeed::Missing)),
    };
    let (hfm, body) = fm::split_fm_optional(&tf.text).unwrap_or_default();
    let written = match fm::get_scalar(&hfm, "written").and_then(|w| parse_ts(&w)) {
        Some(w) => w,
        None => return Ok(Some(HandoffNeed::NoWritten)),
    };
    let touched = files_touched(&body);
    let mut newest: Option<(String, NaiveDateTime)> = None;
    let mut bump = |p: &str, t: Option<NaiveDateTime>| {
        if let Some(t) = t {
            if newest.as_ref().map_or(true, |(_, n)| t > *n) {
                newest = Some((p.to_string(), t));
            }
        }
    };
    if !touched.is_empty() {
        for p in &touched {
            bump(p, life::newest_mtime(&abs_of(ctx, p), Some(&hpath)));
        }
    } else if let Some(btf) = TextFile::load(&vpath(ctx, &brief_rel(&row.name)))? {
        if let Some((bfm, _)) = fm::split_fm_optional(&btf.text) {
            for p in fm::fm_list_values(&bfm, "write_surface") {
                bump(&p, life::newest_mtime(&abs_of(ctx, &p), Some(&hpath)));
            }
        }
    }
    if let Some((path, at)) = newest.filter(|(_, at)| *at > written) {
        return Ok(Some(HandoffNeed::Stale { path, at, written }));
    }
    let fix: Vec<String> = page::issues_of(&tf.text, crate::core::today_date())
        .into_iter()
        .filter(|i| i.kind == page::Kind::Fix)
        .map(|i| i.text)
        .collect();
    Ok(if fix.is_empty() { None } else { Some(HandoffNeed::Fix(fix)) })
}

/// True when the chat must write (or refresh) its handoff before it stops.
pub fn handoff_required(ctx: &Ctx, row: &Row) -> R<bool> {
    Ok(handoff_need(ctx, row)?.is_some())
}

/// Doctor: a handoff whose `written` is older than the newest other file in its folder.
pub fn stale_handoff_notes(ctx: &Ctx) -> R<Vec<String>> {
    let root = vpath(ctx, "system/sessions");
    let mut notes = Vec::new();
    if !pyos::isdir(&root) {
        return Ok(notes);
    }
    let mut handoffs: Vec<String> = walkdir::WalkDir::new(&root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.file_name() == "handoff.md")
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    handoffs.sort();
    for h in handoffs {
        let rel = ctx.rel(&h)?;
        let tf = match TextFile::load(&h)? {
            Some(t) => t,
            None => continue,
        };
        let written = fm::split_fm_optional(&tf.text).and_then(|(f, _)| fm::get_scalar(&f, "written")).and_then(|w| parse_ts(&w));
        let folder = pyos::dirname(&h);
        let newest = std::fs::read_dir(&folder)
            .ok()
            .into_iter()
            .flat_map(|rd| rd.flatten())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false) && e.file_name() != "handoff.md" && e.file_name() != "archive.md")
            .filter_map(|e| life::mtime_min(&e.path().to_string_lossy()))
            .max();
        match (written, newest) {
            (None, _) => notes.push(format!("{rel}: no parseable 'written' stamp; the Stop check cannot trust it")),
            (Some(w), Some(n)) if n > w => notes.push(format!(
                "{rel}: written {} is older than its folder's newest file ({}); the chat should refresh it",
                fmt_min(w),
                fmt_min(n)
            )),
            _ => {}
        }
    }
    Ok(notes)
}

// ------------------------------------------------------------------ the roster

/// `claude agents --json --all`, or None when it is absent, fails, or takes over 5 s.
pub fn claude_roster() -> Option<Value> {
    let mut cmd = std::process::Command::new(crate::proc::claude_bin());
    cmd.args(["agents", "--json", "--all"]);
    let out = crate::proc::run_command(cmd, "claude", Some(std::time::Instant::now() + std::time::Duration::from_secs(5)), false);
    if !out.ok() {
        return None;
    }
    serde_json::from_str::<Value>(&out.stdout).ok().filter(|v| v.is_array())
}

/// The roster `status` for each row's session id: `absent` when the roster lacks it, `unknown`
/// when it lists the chat with no status (claude prints none when it is not busy, shell, idle or
/// waiting), so a chat that is there is never reported as gone.
pub fn join_roster(rows: &[Row], roster: &Value) -> Vec<String> {
    let entries = roster.as_array().cloned().unwrap_or_default();
    rows.iter()
        .map(|r| {
            match entries.iter().find(|e| e.get("sessionId").and_then(|v| v.as_str()) == Some(r.session_id.as_str())) {
                None => "absent".to_string(),
                Some(e) => e
                    .get("status")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            }
        })
        .collect()
}

// ------------------------------------------------------------------ verbs

#[derive(Subcommand, Debug)]
pub enum SessionCmd {
    /// Create system/sessions/<name>/ and write or replace its registry row
    Register {
        name: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        brief: Option<String>,
        /// main, side, or build: written to the brief's `class:` line (default: from the name)
        #[arg(long)]
        class: Option<String>,
    },
    /// Print the registry, joined with the claude agents roster when available
    List {
        #[arg(long)]
        json: bool,
    },
    /// Stamp last_seen; exit 3 with HANDOFF_REQUIRED when the handoff is missing or stale
    Stop {
        name: Option<String>,
        #[arg(long)]
        session: Option<String>,
    },
    /// Check a chat's page (its handoff.md) and move its finished Done items to archive.md; exit 1 when an issue remains
    Tidy {
        /// the chat's registry name, whatever its status; default: the calling chat
        #[arg(conflicts_with = "all")]
        name: Option<String>,
        /// change nothing; print the issues
        #[arg(long)]
        check: bool,
        /// every chat whose registry row has not ended
        #[arg(long)]
        all: bool,
    },
    /// Mark a row ended
    End {
        name: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(END_REASONS))]
        reason: String,
    },
    /// A chat's class, context tokens, window and thresholds; every live chat with no name
    Context { name: Option<String> },
    /// Compact a chat: from outside when it has no live process (claude -p "/compact" --resume), through the VS Code panel when it is running
    Compact {
        name: String,
        /// the model that writes the summary; default: the model of the chat's last reply
        #[arg(long)]
        model: Option<String>,
        /// extra summary instructions, placed after /compact
        #[arg(long)]
        instructions: Option<String>,
        /// compact below 50k tokens, with a haiku model, or when the size is unknown
        #[arg(long)]
        force: bool,
        /// print the command, the model and the tokens before; run nothing
        #[arg(long)]
        dry_run: bool,
        /// a running chat: return at once and compact it from a detached waiter once the chat and the operator are idle
        #[arg(long)]
        when_idle: bool,
        /// minutes the panel driver waits for the operator and the chat (default 20; 60 with --when-idle)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// after the compaction is confirmed, send this file's text to the chat as its next message
        #[arg(long)]
        then_file: Option<String>,
        /// the waiter's job file (internal)
        #[arg(long, hide = true)]
        waiter_job: Option<String>,
        /// stop the chat's queued --when-idle compaction (it must not have begun changing the panel)
        #[arg(long, conflicts_with_all = ["model", "instructions", "force", "dry_run", "when_idle", "wait_minutes", "then_file", "waiter_job"])]
        cancel: bool,
    },
    /// Keep this chat's prompt cache warm through a long wait: run it in the background; it waits until the chat has been idle, then exits, which wakes the chat
    Warm {
        name: String,
        /// which wake this will be, counting from 1 (the re-arm line it prints carries the next number)
        #[arg(long, default_value_t = 1)]
        wake: u32,
        /// idle minutes before it fires, 5 to 56 (the cache lives one hour; the idle clock starts at the chat's last reply, which trails the request that refreshed the cache by up to a couple of minutes)
        #[arg(long, default_value_t = crate::warm::DEFAULT_MINUTES)]
        minutes: u64,
        /// stop after this many wakes
        #[arg(long, default_value_t = crate::warm::DEFAULT_MAX_WAKES)]
        max_wakes: u32,
        /// do not warm a chat below this many context tokens: a cold restart rewrites little
        #[arg(long, default_value_t = crate::warm::DEFAULT_MIN_TOKENS)]
        min_tokens: u64,
    },
    /// Type /rename into a chat through the VS Code panel; the default name is the registry name
    Rename {
        name: String,
        /// the new title (one line)
        #[arg(long)]
        to: Option<String>,
        /// minutes the panel driver waits for the operator and the chat (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
    },
    /// Archive a chat in the VS Code panel's list (its row's Archive button); the registry row is not changed
    Archive {
        /// the chat's registry name; omit it with --session-id for a chat with no registry row
        name: Option<String>,
        /// the chat's session id, for a chat with no registry row
        #[arg(long)]
        session_id: Option<String>,
        /// minutes the panel driver waits for the operator and the chat (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print what would run; change nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// The groups of the VS Code panel's sessions list: list, create, rename, delete, and put a chat in one or take it out
    Group {
        #[command(subcommand)]
        cmd: GroupCmd,
    },
    /// Open a new chat in the VS Code panel with a file as its first message; prints its session id
    Start {
        name: String,
        /// the first message: the brief and the instruction to register
        #[arg(long)]
        brief_prompt_file: String,
        /// the model to pick in the new chat (as the panel's picker names it)
        #[arg(long)]
        model: Option<String>,
        /// put the new chat in this existing session group of the panel (by name): the chat is started, then placed with group add
        #[arg(long)]
        group: Option<String>,
        /// minutes the panel driver waits for the operator (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
    },
    /// The chats in the VS Code panel, with their registry names
    Panel {
        #[arg(long)]
        json: bool,
    },
    /// Print the SessionStart injection for a session id
    Bundle {
        #[arg(long)]
        session: String,
    },
    /// Whether a message reaches this chat and starts a turn, and what to do; --confirm checks one arrived
    Reach {
        name: String,
        /// the sending chat's registry name; defaults to the main chat
        #[arg(long)]
        from: Option<String>,
        /// a phrase from a sent message: exit 0 once its Claude read it, 3 until then
        #[arg(long)]
        confirm: Option<String>,
        /// with --confirm: the time reach printed before you sent; an older record does not count
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

/// What `af session group` does. Every change takes his screen for a few seconds (VS Code comes to the front, the pointer
/// and keyboard are used) while he is idle, and puts it back; `--dry-run` prints the driver call and what would show.
#[derive(Subcommand, Debug)]
pub enum GroupCmd {
    /// The groups in the panel's list and how many chats each holds (reads the screen only)
    List {
        #[arg(long)]
        json: bool,
    },
    /// Add a group: presses New group and types the name
    Create {
        name: String,
        /// minutes the panel driver waits for the operator to be idle (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print the driver call and what would show on his screen; run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Rename a group
    Rename {
        old: String,
        new: String,
        /// minutes the panel driver waits for the operator to be idle (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print the driver call and what would show on his screen; run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a group; its chats move to Ungrouped, and the panel asks nothing, so a group with chats needs --force
    Delete {
        name: String,
        /// delete a group that holds chats
        #[arg(long)]
        force: bool,
        /// minutes the panel driver waits for the operator to be idle (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print the driver call and what would show on his screen; run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Put a chat in a group: `add <chat> <group>`, or `add --session-id <id> <group>` for a chat with no registry row
    Add {
        /// the chat's registry name and the group's name, or only the group's name with --session-id
        #[arg(required = true, num_args = 1..=2)]
        args: Vec<String>,
        /// the chat's session id, for a chat with no registry row
        #[arg(long)]
        session_id: Option<String>,
        /// minutes the panel driver waits for the operator to be idle (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print the driver call and what would show on his screen; run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Take a chat out of its group (it moves to Ungrouped)
    Remove {
        /// the chat's registry name; omit it with --session-id for a chat with no registry row
        chat: Option<String>,
        /// the chat's session id, for a chat with no registry row
        #[arg(long)]
        session_id: Option<String>,
        /// minutes the panel driver waits for the operator to be idle (default 20)
        #[arg(long)]
        wait_minutes: Option<f64>,
        /// print the driver call and what would show on his screen; run nothing
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(ctx: &Ctx, cmd: SessionCmd) -> R<i32> {
    match cmd {
        SessionCmd::Register { name, session, model, brief, class } => {
            cmd_register(ctx, &name, &session, model.as_deref(), brief.as_deref(), class.as_deref())
        }
        SessionCmd::Context { name } => crate::context::cmd_context(ctx, name.as_deref()),
        SessionCmd::List { json } => cmd_list(ctx, json),
        SessionCmd::Stop { name, session } => cmd_stop(ctx, name.as_deref(), session.as_deref()),
        SessionCmd::Tidy { name, check, all } => page::cmd_tidy(ctx, name.as_deref(), check, all),
        SessionCmd::End { name, session, reason } => cmd_end(ctx, name.as_deref(), session.as_deref(), &reason),
        SessionCmd::Reach { name, from, confirm, since, json } => {
            crate::reach::cmd_reach(ctx, &name, from.as_deref(), confirm.as_deref(), since.as_deref(), json)
        }
        SessionCmd::Compact { name, model, instructions, force, dry_run, when_idle, wait_minutes, then_file, waiter_job, cancel } => {
            crate::compact::cmd_compact(
                ctx,
                &crate::compact::Opts {
                    name: &name,
                    model: model.as_deref(),
                    instructions: instructions.as_deref(),
                    force,
                    dry_run,
                    when_idle,
                    wait_minutes,
                    then_file: then_file.as_deref(),
                    waiter_job: waiter_job.as_deref(),
                    cancel,
                },
            )
        }
        SessionCmd::Warm { name, wake, minutes, max_wakes, min_tokens } => {
            crate::warm::cmd_warm(ctx, &crate::warm::Opts { name: &name, wake, minutes, max_wakes, min_tokens })
        }
        SessionCmd::Archive { name, session_id, wait_minutes, dry_run } => {
            crate::panel::cmd_archive(ctx, name.as_deref(), session_id.as_deref(), wait_minutes, dry_run)
        }
        SessionCmd::Rename { name, to, wait_minutes } => crate::panel::cmd_rename(ctx, &name, to.as_deref(), wait_minutes),
        SessionCmd::Group { cmd } => match cmd {
            GroupCmd::List { json } => crate::panel::cmd_group_list(ctx, json),
            GroupCmd::Create { name, wait_minutes, dry_run } => crate::panel::cmd_group_create(ctx, &name, wait_minutes, dry_run),
            GroupCmd::Rename { old, new, wait_minutes, dry_run } => crate::panel::cmd_group_rename(ctx, &old, &new, wait_minutes, dry_run),
            GroupCmd::Delete { name, force, wait_minutes, dry_run } => crate::panel::cmd_group_delete(ctx, &name, force, wait_minutes, dry_run),
            GroupCmd::Add { args, session_id, wait_minutes, dry_run } => crate::panel::cmd_group_add(ctx, &args, session_id.as_deref(), wait_minutes, dry_run),
            GroupCmd::Remove { chat, session_id, wait_minutes, dry_run } => crate::panel::cmd_group_remove(ctx, chat.as_deref(), session_id.as_deref(), wait_minutes, dry_run),
        },
        SessionCmd::Start { name, brief_prompt_file, model, group, wait_minutes } => {
            crate::panel::cmd_start(ctx, &name, &brief_prompt_file, model.as_deref(), group.as_deref(), wait_minutes)
        }
        SessionCmd::Panel { json } => crate::panel::cmd_panel(ctx, json),
        SessionCmd::Bundle { session } => {
            let (text, _) = bundle(ctx, &session)?;
            if !text.is_empty() {
                out::line(&text);
            }
            Ok(0)
        }
    }
}

fn clean_cell(label: &str, v: &str) -> R<String> {
    let v = v.trim();
    if v.contains('|') || v.contains('\n') || v.contains('\r') {
        die!("{label} may not contain '|' or a line break");
    }
    Ok(v.to_string())
}

pub fn cmd_register(ctx: &Ctx, name: &str, session: &str, model: Option<&str>, brief: Option<&str>, class: Option<&str>) -> R<i32> {
    if !valid_name(name) {
        die!("session name '{name}' is outside the grammar: {NAME_GRAMMAR}");
    }
    let session = clean_cell("--session", session)?;
    if session.is_empty() {
        die!("--session needs the chat's session id");
    }
    let model = clean_cell("--model", model.unwrap_or(""))?;
    let src = match brief {
        Some(b) => {
            let p = abs_of(ctx, b);
            if !pyos::isfile(&p) {
                die!("brief not found: {b}");
            }
            Some(p)
        }
        None => None,
    };
    let class = match class {
        Some(c) => match SessionClass::parse(c) {
            Some(c) => Some(c),
            None => die!("unknown --class '{c}': use one of {}", CLASS_NAMES.join(", ")),
        },
        None => None,
    };
    let dir = vpath(ctx, &session_rel(name));
    let dst = pyos::join(&dir, "brief.md");
    if class.is_some() && src.is_none() && !pyos::isfile(&dst) {
        die!("--class is written to the brief's class: line and {} has no brief; pass --brief <path> or write the brief first", brief_rel(name));
    }
    pyos::makedirs(&dir)?;
    if let Some(p) = src {
        if pyos::abspath(&p) != pyos::abspath(&dst) {
            std::fs::copy(&p, &dst).map_err(|e| pyos::os_error(&e, &dst))?;
        }
    }
    if let Some(c) = class {
        write_brief_class(&dst, c)?;
    }
    let now = life::now_min();
    let row = Row {
        name: name.to_string(),
        role: role_of(name).to_string(),
        session_id: session,
        model: if model.is_empty() { "-".to_string() } else { model },
        started: now.clone(),
        last_seen: now,
        status: "live".to_string(),
        handoff: handoff_rel(name),
    };
    let mut reg = Registry::load(ctx)?;
    reg.upsert(&row);
    reg.save()?;
    outln!("{}", row.line());
    Ok(0)
}

pub fn cmd_list(ctx: &Ctx, as_json: bool) -> R<i32> {
    let mut reg = Registry::load(ctx)?;
    let now = life::now();
    let mut changed = false;
    for mut r in reg.rows() {
        let eff = effective_status(&r, now);
        if eff != r.status {
            r.status = eff;
            reg.upsert(&r);
            changed = true;
        }
    }
    if changed {
        reg.save()?;
    }
    let rows = reg.rows();
    let roster = claude_roster();
    let states = roster.as_ref().map(|r| join_roster(&rows, r));
    let missing = "claude agents roster unavailable (claude agents --json --all); registry only";
    if as_json {
        let arr: Vec<Value> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut v = r.to_json();
                v["class"] = json!(class_of(ctx, &r.name).name());
                if let Some(s) = &states {
                    v["live_state"] = json!(s[i]);
                }
                v
            })
            .collect();
        out::line(&serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_default());
        if states.is_none() {
            out::err_line(&format!("af session list: {missing}"));
        }
        return Ok(0);
    }
    let class = |r: &Row| class_of(ctx, &r.name).name();
    match &states {
        Some(s) => {
            outln!("{HEADER} class | live_state |");
            outln!("{SEPARATOR}---|---|");
            for (r, st) in rows.iter().zip(s) {
                outln!("{} {} | {st} |", r.line(), class(r));
            }
        }
        None => {
            outln!("{HEADER} class |");
            outln!("{SEPARATOR}---|");
            for r in &rows {
                outln!("{} {} |", r.line(), class(r));
            }
            outln!("af session list: {missing}");
        }
    }
    Ok(0)
}

fn pick(reg: &Registry, name: Option<&str>, session: Option<&str>) -> R<Option<Row>> {
    match (name, session) {
        (Some(n), _) if !n.is_empty() => Ok(reg.find_name(n)),
        (_, Some(s)) if !s.is_empty() => Ok(reg.find_session(s)),
        _ => die!("give a session name or --session <id>"),
    }
}

/// The stop mechanics shared by the verb and the Stop hook: stamp, then check.
/// Some((handoff rel, why)) when a handoff is required. With `exempt_main`, a `main` row is
/// only stamped: its state is life.md and open.md, which it writes all day.
pub fn stop_row(ctx: &Ctx, name: Option<&str>, session: Option<&str>, exempt_main: bool) -> R<Option<(String, HandoffNeed)>> {
    let mut reg = Registry::load(ctx)?;
    let mut row = match pick(&reg, name, session)? {
        Some(r) => r,
        None => return Ok(None),
    };
    row.touch(false);
    reg.upsert(&row);
    reg.save()?;
    if exempt_main && row.role == "main" {
        return Ok(None);
    }
    Ok(handoff_need(ctx, &row)?.map(|need| (handoff_rel(&row.name), need)))
}

pub fn cmd_stop(ctx: &Ctx, name: Option<&str>, session: Option<&str>) -> R<i32> {
    match stop_row(ctx, name, session, false)? {
        Some((rel, need)) => {
            outln!("HANDOFF_REQUIRED {rel} ({})", need.short());
            Ok(3)
        }
        None => Ok(0),
    }
}

/// The end mechanics shared by the verb and the SessionEnd hook; the ended row.
pub fn end_row(ctx: &Ctx, name: Option<&str>, session: Option<&str>, reason: &str) -> R<Option<Row>> {
    let mut reg = Registry::load(ctx)?;
    let mut row = match pick(&reg, name, session)? {
        Some(r) => r,
        None => return Ok(None),
    };
    row.status = format!("ended {} {reason}", life::now_min());
    reg.upsert(&row);
    reg.save()?;
    if row.role == "worker" || row.role == "builder" {
        life::append_log(ctx, "session_ended", &row.name)?;
    }
    Ok(Some(row))
}

pub fn cmd_end(ctx: &Ctx, name: Option<&str>, session: Option<&str>, reason: &str) -> R<i32> {
    match end_row(ctx, name, session, reason)? {
        Some(row) => {
            outln!("{}", row.line());
            Ok(0)
        }
        None => die!("no registry row for {}", name.filter(|n| !n.is_empty()).or(session).unwrap_or("?")),
    }
}

// ------------------------------------------------------------------ the bundle

fn file_block(ctx: &Ctx, label: &str, rel: &str) -> R<Option<String>> {
    Ok(TextFile::load(&vpath(ctx, rel))?
        .map(|tf| tf.text.trim_end().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| format!("{label} ({rel}):\n{t}")))
}

/// The "last handoff" block, cut at the page's word cap with a line saying so, and, when the page has any
/// issue, the block that asks the chat to tidy it first.
fn handoff_blocks(ctx: &Ctx, rel: &str) -> R<Vec<String>> {
    let Some(tf) = TextFile::load(&vpath(ctx, rel))? else { return Ok(Vec::new()) };
    let text = tf.text.trim_end();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let pg = page::Page::parse(text);
    let words = pg.words();
    let shown = if words > page::PAGE_WORD_CAP {
        let (cut, shown) = pg.cut(page::PAGE_WORD_CAP);
        format!("{cut}\naf: this page was cut: {shown} of its {words} words are shown. Read {rel} for the rest, and shorten it.")
    } else {
        text.to_string()
    };
    let mut out = vec![format!("last handoff ({rel}):
{shown}")];
    let issues = pg.issues(crate::core::today_date());
    if !issues.is_empty() {
        out.push(page::tidy_block(&issues));
    }
    Ok(out)
}

/// The SessionStart injection for a session id, and its registry row.
pub fn bundle(ctx: &Ctx, session_id: &str) -> R<(String, Option<Row>)> {
    bundle_with(ctx, session_id, None)
}

/// The main chat's row when it is live, found by role whatever its name.
pub fn live_main(reg: &Registry, now: NaiveDateTime) -> Option<Row> {
    reg.rows().into_iter().find(|r| is_main_chat(r) && effective_status(r, now) == "live")
}

/// What an unregistered chat does in its first reply: ask whether it is main, a side
/// chat, or a one-off. With a live `main`, main is not on offer.
pub fn off_system_directive(session_id: &str, main: Option<&Row>) -> String {
    let [l, p, s] = SIDE_NAMES;
    let side = format!(
        "make it a side chat (name it {l}, {p}, or {s}; register it with \"af session register <name> --session {session_id}\", \
write system/sessions/<name>/brief.md, and tell him to send main the name)"
    );
    let one_off = "keep it a one-off (nothing is registered; a loop, a life fact, or a handoff it turns up goes to af-main)";
    let ask = "Before anything else, ask the operator in one line, lettered:";
    let tail = "If he answers with a task instead of a letter, treat it as (C) and say so in your first sentence.";
    match main {
        None => format!(
            "af: this chat is not registered and main is not running. {ask} (A) make this chat main (rename it {MAIN_NAME} with /rename {MAIN_NAME} and register it with \"af session register {MAIN_NAME} --session {session_id}\"), (B) {side}, or (C) {one_off}. {tail}"
        ),
        Some(m) => {
            let seen = parse_ts(&m.last_seen).map(|t| t.format("%H:%M").to_string()).unwrap_or_else(|| m.last_seen.clone());
            format!(
                "af: this chat is not registered. main is live ({}, last seen {seen}); this chat cannot be main. {ask} (B) {side}, or (C) {one_off}. {tail}",
                m.name
            )
        }
    }
}

/// After a compaction or a resume the question was already asked once in this chat.
pub const OFF_SYSTEM_REMINDER: &str = "af: this chat is still unregistered: a one-off unless he said otherwise earlier in this chat. Do not ask again; a loop, a life fact, or a handoff it turns up goes to af-main.";

/// The bundle, led by the index-refresh line when the session-start hook spawned a
/// refresh (which then stands in for the index age line).
pub fn bundle_with(ctx: &Ctx, session_id: &str, refresh: Option<&str>) -> R<(String, Option<Row>)> {
    bundle_with_source(ctx, session_id, refresh, None)
}

/// `bundle_with`, knowing the SessionStart `source`: an unregistered chat gets the A/B/C
/// question on `startup` and `clear`, and only the reminder on `compact` and `resume`.
pub fn bundle_with_source(ctx: &Ctx, session_id: &str, refresh: Option<&str>, source: Option<&str>) -> R<(String, Option<Row>)> {
    bundle_for_start(ctx, session_id, refresh, source, None, None)
}

/// `bundle_with_source`, knowing what the hook's input said about the chat's model and transcript, which
/// choose the model card a build chat is given (see `model_card`).
pub fn bundle_for_start(
    ctx: &Ctx,
    session_id: &str,
    refresh: Option<&str>,
    source: Option<&str>,
    hook_model: Option<&str>,
    transcript: Option<&str>,
) -> R<(String, Option<Row>)> {
    let reg = Registry::load(ctx)?;
    let row = reg.find_session(session_id);
    let mut parts: Vec<String> = Vec::new();
    if let Some(line) = refresh {
        parts.push(line.to_string());
    }
    match &row {
        Some(r) => {
            parts.push(format!("af: you are {} ({})", r.name, r.role));
            if is_main_chat(r) && r.name == LEGACY_MAIN_NAME {
                parts.push(main_rename_line(session_id));
            }
        }
        None if matches!(source, Some("compact") | Some("resume")) => parts.push(OFF_SYSTEM_REMINDER.to_string()),
        None => parts.push(off_system_directive(session_id, live_main(&reg, life::now()).as_ref())),
    }
    let is_main = row.as_ref().map(|r| r.role == "main").unwrap_or(false);
    if is_main {
        if let Some((body, lines, cap)) = life::life_body(ctx)? {
            if lines > cap {
                parts.push(format!("af: life.md is {} lines over its cap of {cap}; cut it before anything else", lines - cap));
            }
            let b = body.trim().to_string();
            if !b.is_empty() {
                parts.push(b);
            }
        }
    }
    if row.as_ref().is_some_and(|r| r.role != "worker") {
        if let Some(doc) = open::OpenDoc::load(ctx)? {
            let due = doc.due(crate::core::today_date());
            let mut s = format!("open loops: {} open, {} due", doc.loops.len(), due.len());
            for (l, age) in &due {
                s.push('\n');
                s.push_str(&open::due_line(l, *age));
            }
            parts.push(s);
            if is_main {
                parts.push(crate::needs::bundle_block(ctx, &doc));
            }
        }
    }
    if is_main {
        parts.extend(crate::loop_watch::bundle_line(ctx));
        parts.extend(crate::watch::bundle_line(ctx));
    }
    if let Some(r) = &row {
        if let Some(b) = file_block(ctx, "brief", &brief_rel(&r.name))? {
            parts.push(b);
        }
        parts.extend(handoff_blocks(ctx, &handoff_rel(&r.name))?);
        parts.extend(crate::model_card::card_block(ctx, r, hook_model, transcript));
    }
    if is_main {
        let now = life::now();
        let roster: Vec<String> =
            reg.rows().iter().filter(|r| !r.is_ended()).map(|r| format!("{} {}", r.name, effective_status(r, now))).collect();
        let mut tail = Vec::new();
        if !roster.is_empty() {
            tail.push(format!("sessions: {}", roster.join(", ")));
        }
        if let Some(ids) = life::backlog_open_ids(ctx) {
            tail.push(format!("backlog: {} open rows", ids.len()));
        }
        if !tail.is_empty() {
            parts.push(tail.join("\n"));
        }
    }
    if refresh.is_none() {
        if let Some(ix) = life::index_age_line(ctx) {
            parts.push(ix);
        }
    }
    Ok((parts.join("\n\n"), row))
}

// ------------------------------------------------------------------ hook handlers

/// What a hook handler hands the dispatcher: stdout text and the process exit code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookOutput {
    pub stdout: String,
    pub exit: i32,
}

impl HookOutput {
    pub fn quiet() -> HookOutput {
        HookOutput::default()
    }
}

fn payload(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn field<'a>(p: &'a Value, key: &str) -> &'a str {
    p.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

/// SessionStart (startup, resume, clear, compact, fork): the bundle as additionalContext,
/// the registered name as sessionTitle; stamps the row live.
pub fn hook_session_start(ctx: &Ctx, payload_text: &str) -> HookOutput {
    hook_session_start_with(ctx, payload_text, None)
}

/// SessionStart with the index-refresh line `af hook session-start` hands it. Without a
/// session id (or on a broken registry) the refresh line alone goes out as plain text.
pub fn hook_session_start_with(ctx: &Ctx, payload_text: &str, refresh: Option<&str>) -> HookOutput {
    let alone = || HookOutput { stdout: refresh.unwrap_or("").to_string(), exit: 0 };
    let p = payload(payload_text);
    let id = field(&p, "session_id");
    if id.is_empty() {
        return alone();
    }
    let source = field(&p, "source");
    let source = if source.is_empty() { None } else { Some(source) };
    let model = Some(field(&p, "model")).filter(|m| !m.is_empty());
    let transcript = crate::context::transcript_for(&p, id);
    let (text, row) = match bundle_for_start(ctx, id, refresh, source, model, transcript.as_deref()) {
        Ok(b) => b,
        Err(_) => return alone(),
    };
    if let Some(r) = &row {
        if let Ok(mut reg) = Registry::load(ctx) {
            let mut r = r.clone();
            r.touch(true);
            reg.upsert(&r);
            let _ = reg.save();
        }
    }
    let mut hso = json!({"hookEventName": "SessionStart", "additionalContext": text});
    if let Some(r) = &row {
        hso["sessionTitle"] = json!(r.name);
    }
    HookOutput { stdout: json!({"hookSpecificOutput": hso}).to_string(), exit: 0 }
}

/// Stop: block once with the handoff instruction when `af session stop` would exit 3;
/// a `main` row is stamped and never blocked.
pub fn hook_stop(ctx: &Ctx, payload_text: &str) -> HookOutput {
    let p = payload(payload_text);
    if p.get("stop_hook_active").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HookOutput::quiet();
    }
    let id = field(&p, "session_id");
    if id.is_empty() {
        return HookOutput::quiet();
    }
    match stop_row(ctx, None, Some(id), true) {
        Ok(Some((rel, need))) => {
            let reason = need.hook_reason(&rel);
            HookOutput { stdout: json!({"decision": "block", "reason": reason}).to_string(), exit: 0 }
        }
        _ => HookOutput::quiet(),
    }
}

/// The harness's SessionEnd reason in the registry's vocabulary.
pub fn end_reason(raw: &str) -> &'static str {
    match raw {
        "clear" => "clear",
        "resume" => "resume",
        "logout" => "logout",
        "exit" | "prompt_input_exit" => "exit",
        _ => "other",
    }
}

/// SessionEnd: stamp the row ended; never blocks.
pub fn hook_session_end(ctx: &Ctx, payload_text: &str) -> HookOutput {
    let p = payload(payload_text);
    let id = field(&p, "session_id");
    if !id.is_empty() {
        let _ = end_row(ctx, None, Some(id), end_reason(field(&p, "reason")));
    }
    HookOutput::quiet()
}

/// Notification: `agent_completed` / `agent_needs_input` for a registered session stamps
/// its `last_seen`; prints nothing.
pub fn hook_notification(ctx: &Ctx, payload_text: &str) -> HookOutput {
    let p = payload(payload_text);
    let kind = field(&p, "notification_type");
    if kind != "agent_completed" && kind != "agent_needs_input" {
        return HookOutput::quiet();
    }
    if let Ok(mut reg) = Registry::load(ctx) {
        let row = reg
            .find_session(field(&p, "session_id"))
            .or_else(|| ["session_name", "name", "session_title"].iter().find_map(|k| reg.find_name(field(&p, k))));
        if let Some(mut r) = row {
            r.touch(false);
            reg.upsert(&r);
            let _ = reg.save();
        }
    }
    HookOutput::quiet()
}
