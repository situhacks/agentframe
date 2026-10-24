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
pub const NAME_GRAMMAR: &str = "main, life/<topic>, project/<slug>, system/<topic>, worker/<slug>";

// ------------------------------------------------------------------ names

pub fn valid_name(name: &str) -> bool {
    rx(r"\A(?:main|(?:life|project|system|worker)/[A-Za-z0-9][A-Za-z0-9_.-]*)\z").is_match(name)
}

pub fn role_of(name: &str) -> &'static str {
    match name.split('/').next().unwrap_or("") {
        "project" => "operator",
        "system" => "builder",
        "worker" => "worker",
        _ => "main",
    }
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

/// True when the chat must write (or refresh) its handoff before it stops.
pub fn handoff_required(ctx: &Ctx, row: &Row) -> R<bool> {
    if row.is_ended() {
        return Ok(false);
    }
    let hpath = vpath(ctx, &handoff_rel(&row.name));
    let tf = match TextFile::load(&hpath)? {
        Some(t) => t,
        None => return Ok(true),
    };
    let (hfm, body) = fm::split_fm_optional(&tf.text).unwrap_or_default();
    let written = fm::get_scalar(&hfm, "written").and_then(|w| parse_ts(&w));
    let touched = files_touched(&body);
    let mut newest: Option<NaiveDateTime> = None;
    let mut bump = |t: Option<NaiveDateTime>| {
        if let Some(t) = t {
            newest = Some(newest.map_or(t, |n: NaiveDateTime| n.max(t)));
        }
    };
    if !touched.is_empty() {
        for p in &touched {
            bump(life::newest_mtime(&abs_of(ctx, p), Some(&hpath)));
        }
    } else if let Some(btf) = TextFile::load(&vpath(ctx, &brief_rel(&row.name)))? {
        if let Some((bfm, _)) = fm::split_fm_optional(&btf.text) {
            for p in fm::fm_list_values(&bfm, "write_surface") {
                bump(life::newest_mtime(&abs_of(ctx, &p), Some(&hpath)));
            }
        }
    }
    Ok(match (written, newest) {
        (None, _) => true,
        (Some(w), Some(n)) => n > w,
        (Some(_), None) => false,
    })
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
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false) && e.file_name() != "handoff.md")
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
    use std::io::Read;
    let mut child = std::process::Command::new("claude")
        .args(["agents", "--json", "--all"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let text = reader.join().ok()?;
                if !status.success() {
                    return None;
                }
                return serde_json::from_str::<Value>(&text).ok().filter(|v| v.is_array());
            }
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

/// The roster `status` for each row's session id; `absent` when the roster lacks it.
pub fn join_roster(rows: &[Row], roster: &Value) -> Vec<String> {
    let entries = roster.as_array().cloned().unwrap_or_default();
    rows.iter()
        .map(|r| {
            entries
                .iter()
                .find(|e| e.get("sessionId").and_then(|v| v.as_str()) == Some(r.session_id.as_str()))
                .and_then(|e| e.get("status").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| "absent".to_string())
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
    /// Mark a row ended
    End {
        name: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(END_REASONS))]
        reason: String,
    },
    /// Print the SessionStart injection for a session id
    Bundle {
        #[arg(long)]
        session: String,
    },
}

pub fn run(ctx: &Ctx, cmd: SessionCmd) -> R<i32> {
    match cmd {
        SessionCmd::Register { name, session, model, brief } => cmd_register(ctx, &name, &session, model.as_deref(), brief.as_deref()),
        SessionCmd::List { json } => cmd_list(ctx, json),
        SessionCmd::Stop { name, session } => cmd_stop(ctx, name.as_deref(), session.as_deref()),
        SessionCmd::End { name, session, reason } => cmd_end(ctx, name.as_deref(), session.as_deref(), &reason),
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

pub fn cmd_register(ctx: &Ctx, name: &str, session: &str, model: Option<&str>, brief: Option<&str>) -> R<i32> {
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
    let dir = vpath(ctx, &session_rel(name));
    pyos::makedirs(&dir)?;
    if let Some(p) = src {
        let dst = pyos::join(&dir, "brief.md");
        if pyos::abspath(&p) != pyos::abspath(&dst) {
            std::fs::copy(&p, &dst).map_err(|e| pyos::os_error(&e, &dst))?;
        }
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
    match &states {
        Some(s) => {
            outln!("{} live_state |", HEADER);
            outln!("{}---|", SEPARATOR);
            for (r, st) in rows.iter().zip(s) {
                outln!("{} {st} |", r.line());
            }
        }
        None => {
            outln!("{HEADER}");
            outln!("{SEPARATOR}");
            for r in &rows {
                outln!("{}", r.line());
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
/// Some(handoff rel) when a handoff is required. With `exempt_main`, a `main` row is
/// only stamped: its state is life.md and open.md, which it writes all day.
pub fn stop_row(ctx: &Ctx, name: Option<&str>, session: Option<&str>, exempt_main: bool) -> R<Option<String>> {
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
    Ok(if handoff_required(ctx, &row)? { Some(handoff_rel(&row.name)) } else { None })
}

pub fn cmd_stop(ctx: &Ctx, name: Option<&str>, session: Option<&str>) -> R<i32> {
    match stop_row(ctx, name, session, false)? {
        Some(rel) => {
            outln!("HANDOFF_REQUIRED {rel}");
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

/// The SessionStart injection for a session id, and its registry row.
pub fn bundle(ctx: &Ctx, session_id: &str) -> R<(String, Option<Row>)> {
    bundle_with(ctx, session_id, None)
}

/// The bundle, led by the index-refresh line when the session-start hook spawned a
/// refresh (which then stands in for the index age line).
pub fn bundle_with(ctx: &Ctx, session_id: &str, refresh: Option<&str>) -> R<(String, Option<Row>)> {
    let reg = Registry::load(ctx)?;
    let row = reg.find_session(session_id);
    let mut parts: Vec<String> = Vec::new();
    if let Some(line) = refresh {
        parts.push(line.to_string());
    }
    match &row {
        Some(r) => parts.push(format!("af: you are {} ({})", r.name, r.role)),
        None => parts.push(format!(
            "af: unregistered session; register with \"af session register <name> --session {session_id}\" or stay ad hoc\n\
af: names are {NAME_GRAMMAR}; classify the task first (AGENTS.md: project work -> AGENTS.operator.md, system work -> AGENTS.builder.md, anything else -> main)"
        )),
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
        if let Some(doc) = open::OpenDoc::load(ctx)? {
            let due = doc.due(crate::core::today_date());
            let mut s = format!("open loops: {} open, {} due", doc.loops.len(), due.len());
            for (l, age) in &due {
                s.push('\n');
                s.push_str(&open::due_line(l, *age));
            }
            parts.push(s);
        }
    }
    if let Some(r) = &row {
        if let Some(b) = file_block(ctx, "brief", &brief_rel(&r.name))? {
            parts.push(b);
        }
        if let Some(h) = file_block(ctx, "last handoff", &handoff_rel(&r.name))? {
            parts.push(h);
        }
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
    let (text, row) = match bundle_with(ctx, id, refresh) {
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
        Ok(Some(rel)) => {
            let reason = format!("Write {rel} (State, Open questions, Next step, Files touched) before stopping.");
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
