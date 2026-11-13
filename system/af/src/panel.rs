//! The Claude panel from `af`: `system/tools/claude_panel.ps1` manages chats in the VS Code panel
//! (sends text to a chat, starts a new one, lists the chats) through UI Automation and the AgentFrame
//! bridge extension, with no key or mouse input, and these verbs build its calls. Text travels in a
//! UTF-8 file, never as an argument, and reaches the driver exactly as given: a text that starts with
//! `/` is not changed. The driver's exit code says why it did not act: 3 the operator was at the
//! machine or the chat was running for the whole wait, 4 a draft sits in the chat's input, 5 a safety
//! check failed before anything was sent, 2 the bridge, window, chat or control was not found, 6 the
//! global model key or the settings file did not match after a model pick.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::outln;
use crate::proc::{run_command, Out};
use crate::reach;
use crate::sessions::{self, Registry};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The driver program: `AF_PANEL_DRIVER` names another program (tests use a stub), else
/// `system/tools/claude_panel.ps1` under Windows PowerShell.
pub const DRIVER_ENV: &str = "AF_PANEL_DRIVER";
/// How long the driver waits for the operator and the chat when asked for no other time.
pub const DRIVER_WAIT_MINUTES: f64 = 20.0;
/// The time the driver may take beyond its wait: the panel is slow to read and the typing is guarded.
const SLACK: Duration = Duration::from_secs(4 * 60);
const LIST_LIMIT: Duration = Duration::from_secs(90);
const WAITER_LOG: &str = "system/logs/panel-waiters.log";

pub struct Driver {
    program: String,
    lead: Vec<String>,
}

pub fn driver(ctx: &Ctx) -> R<Driver> {
    if let Ok(p) = std::env::var(DRIVER_ENV) {
        if !p.trim().is_empty() {
            return Ok(Driver { program: p, lead: Vec::new() });
        }
    }
    let script = PathBuf::from(&ctx.root).join("system").join("tools").join("claude_panel.ps1");
    if !script.is_file() {
        die!("the panel driver is missing: {}", script.display());
    }
    Ok(Driver {
        program: "powershell.exe".to_string(),
        lead: ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"].iter().map(|s| s.to_string()).chain([script.to_string_lossy().into_owned()]).collect(),
    })
}

/// One driver call.
#[derive(Default)]
pub struct Call<'a> {
    pub action: &'a str,
    pub session: Option<&'a str>,
    pub text_file: Option<&'a Path>,
    pub model: Option<&'a str>,
    pub group_id: Option<&'a str>,
    pub wait_minutes: Option<f64>,
}

impl Driver {
    pub fn args(&self, c: &Call) -> Vec<String> {
        let mut a = self.lead.clone();
        a.push(c.action.to_string());
        if let Some(s) = c.session {
            a.extend(["-Session".to_string(), s.to_string()]);
        }
        if let Some(t) = c.text_file {
            a.extend(["-TextFile".to_string(), t.to_string_lossy().into_owned()]);
        }
        if let Some(m) = c.model {
            a.extend(["-Model".to_string(), m.to_string()]);
        }
        if let Some(g) = c.group_id {
            a.extend(["-GroupId".to_string(), g.to_string()]);
        }
        if let Some(w) = c.wait_minutes {
            a.extend(["-WaitMinutes".to_string(), format!("{w}")]);
        }
        a
    }

    /// The call as one line, for a dry run.
    pub fn shown(&self, c: &Call) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args(c));
        parts.iter().map(|p| if p.contains(' ') { format!("\"{p}\"") } else { p.clone() }).collect::<Vec<_>>().join(" ")
    }

    fn run(&self, ctx: &Ctx, c: &Call, limit: Duration) -> Out {
        let mut cmd = Command::new(&self.program);
        cmd.args(self.args(c)).current_dir(&ctx.root);
        run_command(cmd, "the panel driver", Some(Instant::now() + limit), true)
    }
}

/// A file holding the text to send, exactly as given, under `system/logs/claude-panel/`; removed when dropped unless kept.
pub struct TextFile {
    path: PathBuf,
}

impl TextFile {
    pub fn new(ctx: &Ctx, text: &str) -> R<TextFile> {
        let dir = PathBuf::from(&ctx.root).join("system").join("logs").join("claude-panel");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            die!("cannot create {}: {e}", dir.display());
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = dir.join(format!("text-{}-{nanos}.txt", std::process::id()));
        if let Err(e) = std::fs::write(&path, text.as_bytes()) {
            die!("cannot write {}: {e}", path.display());
        }
        Ok(TextFile { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TextFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn limit_for(wait_minutes: Option<f64>, extra: Duration) -> Duration {
    Duration::from_secs_f64(wait_minutes.unwrap_or(DRIVER_WAIT_MINUTES).max(0.0) * 60.0) + SLACK + extra
}

/// The driver's answer: its output on exit 0, else the sentence that says what it refused and why.
fn answer(out: &Out) -> Result<String, String> {
    let why = out.why();
    match out.code {
        Some(0) => Ok(out.stdout.trim().to_string()),
        Some(2) => Err(format!("the panel driver could not find what it needs (the bridge extension, the VS Code window, the chat or a control): {why}. A chat running in a terminal has no panel row")),
        Some(3) => Err(format!("refused (exit 3): the operator was using the computer, or the chat was running, for the whole wait; nothing was sent. {why}")),
        Some(4) => Err(format!("refused (exit 4): the chat has a draft typed in its input box; nothing was sent. Ask him to send or clear it. {why}")),
        Some(5) => Err(format!("refused (exit 5): a safety check failed before anything was sent, so nothing was sent. {why}")),
        Some(6) => Err(format!("MODEL KEY MISMATCH (exit 6): after a model pick the global model key or ~/.claude/settings.json did not match what the driver saved, and the driver could not put it back. Fix settings.json by hand before anything else runs. {why}")),
        Some(n) => Err(format!("the panel driver failed (exit {n}): {why}")),
        None => Err(format!("the panel driver did not run to the end: {why}")),
    }
}

/// Send the text to the chat. Err carries the refusal sentence.
pub fn send(ctx: &Ctx, session: &str, text: &str, wait_minutes: Option<f64>) -> Result<(), String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let call = Call { action: "send", session: Some(session), text_file: Some(file.path()), wait_minutes, ..Call::default() };
    answer(&d.run(ctx, &call, limit_for(wait_minutes, Duration::ZERO))).map(|_| ())
}

/// A new chat with this first message; its session id.
pub fn new_chat(ctx: &Ctx, text: &str, model: Option<&str>, group_id: Option<&str>, wait_minutes: Option<f64>) -> Result<String, String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let call = Call { action: "new", text_file: Some(file.path()), model, group_id, wait_minutes, ..Call::default() };
    let id = answer(&d.run(ctx, &call, limit_for(wait_minutes, Duration::from_secs(60))))?;
    let id = id.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string();
    if id.len() < 8 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(format!("the panel driver sent the message but printed no session id (it printed '{id}')"));
    }
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelChat {
    pub session_id: String,
    pub name: String,
    pub status: String,
    pub active: bool,
}

/// The chats the panel lists.
pub fn list(ctx: &Ctx) -> Result<Vec<PanelChat>, String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let out = d.run(ctx, &Call { action: "list", ..Call::default() }, LIST_LIMIT);
    let text = answer(&out)?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("the panel driver's list is not JSON: {e}"))?;
    let rows = v.as_array().ok_or("the panel driver's list is not a JSON array")?;
    Ok(rows
        .iter()
        .map(|r| PanelChat {
            session_id: r.get("session_id").and_then(Value::as_str).unwrap_or("").to_string(),
            name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            status: r.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
            active: r.get("active").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect())
}

// ------------------------------------------------------------------ the extension's session groups

/// Test hook: the path of a `state.vscdb` that replaces VS Code's own.
pub const STATE_DB_ENV: &str = "AF_VSCODE_STATE_DB";

fn state_db_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var(STATE_DB_ENV) {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let appdata = std::env::var("APPDATA").ok().filter(|a| !a.trim().is_empty())?;
    Some(PathBuf::from(appdata).join("Code").join("User").join("globalStorage").join("state.vscdb"))
}

/// A folder path as the extension keys its groups: the worktree suffix is cut, slashes and case do not matter.
fn norm_root(path: &str) -> String {
    let mut p = path.replace('/', "\\");
    if let Some(i) = p.to_lowercase().find("\\.claude\\worktrees\\") {
        p.truncate(i);
    }
    p.trim_end_matches('\\').to_lowercase()
}

/// The groups the Claude extension stored for this vault: (id, name). Read only, from a copy of the
/// file: table `ItemTable`, key `Anthropic.claude-code`, inner key `sessionGroups:<vault root>`.
pub fn session_groups(ctx: &Ctx) -> Result<Vec<(String, String)>, String> {
    let Some(src) = state_db_path() else { return Err("cannot find VS Code's state.vscdb (APPDATA is not set)".to_string()) };
    session_groups_at(&src, &ctx.root)
}

/// The same, from a named `state.vscdb` and vault root.
pub fn session_groups_at(src: &Path, vault_root: &str) -> Result<Vec<(String, String)>, String> {
    if !src.is_file() {
        return Err(format!("cannot find VS Code's state file {}", src.display()));
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("af-state-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let copy = dir.join("state.vscdb");
    let result = (|| -> Result<Vec<(String, String)>, String> {
        std::fs::copy(src, &copy).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        let wal = PathBuf::from(format!("{}-wal", src.display()));
        if wal.is_file() {
            let _ = std::fs::copy(&wal, dir.join("state.vscdb-wal"));
        }
        let text: String = {
            let conn = Connection::open(&copy).map_err(|e| format!("cannot open the copy of state.vscdb: {e}"))?;
            let value: Option<rusqlite::types::Value> = conn
                .query_row("SELECT value FROM ItemTable WHERE key = 'Anthropic.claude-code'", [], |r| r.get(0))
                .map(Some)
                .or_else(|e| if matches!(e, rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(e) })
                .map_err(|e| format!("cannot read ItemTable in state.vscdb: {e}"))?;
            match value {
                Some(rusqlite::types::Value::Text(t)) => t,
                Some(rusqlite::types::Value::Blob(b)) => String::from_utf8_lossy(&b).into_owned(),
                _ => return Err("the Claude extension has stored no state yet (no Anthropic.claude-code key)".to_string()),
            }
        };
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("the extension's stored state is not JSON: {e}"))?;
        let obj = v.as_object().ok_or("the extension's stored state is not an object")?;
        let want = norm_root(vault_root);
        let mut out = Vec::new();
        for (k, val) in obj {
            let Some(root) = k.strip_prefix("sessionGroups:") else { continue };
            if norm_root(root) != want {
                continue;
            }
            let list = match val {
                Value::String(s) => serde_json::from_str::<Value>(s).unwrap_or(Value::Null),
                other => other.clone(),
            };
            for g in list.as_array().into_iter().flatten() {
                let id = g.get("id").and_then(Value::as_str).unwrap_or("");
                let name = g.get("name").and_then(Value::as_str).unwrap_or("");
                if !id.is_empty() && !name.is_empty() {
                    out.push((id.to_string(), name.to_string()));
                }
            }
        }
        Ok(out)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// A group name to its id. An unknown name is a refusal that lists the names there are.
pub fn resolve_group(ctx: &Ctx, name: &str) -> Result<String, String> {
    let groups = session_groups(ctx)?;
    let want = name.trim();
    if let Some((id, _)) = groups.iter().find(|(_, n)| n == want) {
        return Ok(id.clone());
    }
    let folded: Vec<&(String, String)> = groups.iter().filter(|(_, n)| n.eq_ignore_ascii_case(want)).collect();
    if folded.len() == 1 {
        return Ok(folded[0].0.clone());
    }
    let names: Vec<String> = groups.iter().map(|(_, n)| format!("'{n}'")).collect();
    Err(format!(
        "no session group named '{want}' in the panel (a group is made by hand in the panel's list; nothing here creates one). The groups are: {}",
        if names.is_empty() { "none".to_string() } else { names.join(", ") }
    ))
}

// ------------------------------------------------------------------ the verbs

/// `af session panel [--json]`: the chats in the panel, with their registry names.
pub fn cmd_panel(ctx: &Ctx, as_json: bool) -> R<i32> {
    let chats = match list(ctx) {
        Ok(c) => c,
        Err(m) => die!("af session panel: {m}"),
    };
    let reg = Registry::load(ctx)?;
    let registry_name = |id: &str| reg.find_session(id).map(|r| r.name).unwrap_or_default();
    if as_json {
        let arr: Vec<Value> = chats
            .iter()
            .map(|c| json!({"session_id": c.session_id, "panel_name": c.name, "registry_name": registry_name(&c.session_id), "status": c.status, "active": c.active}))
            .collect();
        outln!("{}", serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_default());
        return Ok(0);
    }
    outln!("| session id | panel name | registry name | status | active |");
    outln!("|---|---|---|---|---|");
    for c in &chats {
        let reg_name = registry_name(&c.session_id);
        outln!("| {} | {} | {} | {} | {} |", c.session_id, c.name, if reg_name.is_empty() { "-" } else { &reg_name }, c.status, if c.active { "yes" } else { "no" });
    }
    Ok(0)
}

/// `af session rename <name> [--to <text>]`: send `/rename <text>` to the chat, then read the panel's row back.
pub fn cmd_rename(ctx: &Ctx, name: &str, to: Option<&str>, wait_minutes: Option<f64>) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let row = reach::row_or_refuse(&reg, name)?;
    let to = to.unwrap_or(name).trim();
    if to.is_empty() || to.contains('\n') || to.contains('\r') {
        die!("--to is one non-empty line of text, got {to:?}");
    }
    if let Err(m) = send(ctx, &row.session_id, &format!("/rename {to}"), wait_minutes) {
        die!("{name}: {m}");
    }
    let mut shown = String::new();
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(readback_pause_ms()));
        }
        match list(ctx) {
            Ok(chats) => {
                if let Some(c) = chats.iter().find(|c| c.session_id == row.session_id) {
                    if c.name == to {
                        outln!("{name}: renamed to {to}; the panel row shows it.");
                        return Ok(0);
                    }
                    shown = c.name.clone();
                }
            }
            Err(m) => die!("{name}: /rename was sent, but the panel could not be read back to check it: {m}"),
        }
    }
    die!("{name}: /rename {to} was sent, but the panel row shows '{shown}', not the new name; check the chat")
}

fn readback_pause_ms() -> u64 {
    std::env::var("AF_PANEL_READBACK_MS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(1000)
}

/// The options the panel's model picker showed on 2026-11-13 (each is `<name> <description>` there; the driver
/// matches the name). Only for the refusal text; the picker is the truth.
const MODEL_OPTIONS: [&str; 14] = [
    "Default (recommended)", "Opus 5.5", "Fable 5.2", "Sonnet 5.5", "Haiku 5.5", "Fable 5.1", "Haiku 4.5",
    "Sonnet 5", "Opus 5", "Fable 5", "Opus 4.8", "Opus 4.7", "Opus 4.6", "Sonnet 4.6",
];

/// `af session start <name> --brief-prompt-file <path> --model <m> [--group <name>]`: a new chat whose first
/// message is the file. The model is required: a new chat inherits the default model otherwise.
pub fn cmd_start(ctx: &Ctx, name: &str, prompt_file: &str, model: Option<&str>, group: Option<&str>, wait_minutes: Option<f64>) -> R<i32> {
    if !sessions::valid_name(name) {
        die!("'{name}' is not a chat name: af-main, life/<topic>, project/<slug>, system/<topic> or worker/<slug>");
    }
    let text = match std::fs::read_to_string(prompt_file) {
        Ok(t) => t,
        Err(e) => die!("cannot read --brief-prompt-file {prompt_file}: {e}"),
    };
    if text.trim().is_empty() {
        die!("--brief-prompt-file {prompt_file} is empty");
    }
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    let Some(model) = model else {
        die!(
            "{name}: --model is needed: a new chat would inherit the default model, which is the expensive one. \
             The model picker shows: {}. (`claude_panel.ps1 model -Session <id>` lists the live options.)",
            MODEL_OPTIONS.join(", ")
        )
    };
    let group_id = match group.map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) => match resolve_group(ctx, g) {
            Ok(id) => Some(id),
            Err(m) => die!("{name}: {m}"),
        },
        None => None,
    };
    match new_chat(ctx, &text, Some(model), group_id.as_deref(), wait_minutes) {
        Ok(id) => {
            outln!("{name}: started in the panel as session {id}. Its first act is af session register {name} --session {id}.");
            Ok(0)
        }
        Err(m) => die!("{name}: {m}"),
    }
}

// ------------------------------------------------------------------ the detached waiter

/// This process's own standard handles stop being inheritable. Windows hands every inheritable handle
/// to a child process, and a detached child holding the pipe that carries this process's output would
/// keep whoever reads it waiting until the child ends.
#[cfg(windows)]
fn stop_inheriting_std_handles() {
    extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetHandleInformation(handle: isize, mask: u32, flags: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 1;
    for which in [-10i32 as u32, -11i32 as u32, -12i32 as u32] {
        // SAFETY: both functions take a handle value and no pointers; an invalid handle only makes the call fail.
        unsafe {
            let h = GetStdHandle(which);
            if h != 0 && h != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Start `program args` detached: it outlives this process and its parent's console, and holds none
/// of this process's handles. Returns its pid.
pub fn spawn_detached(program: &Path, args: &[String], root: &str) -> std::io::Result<u32> {
    #[cfg(windows)]
    stop_inheriting_std_handles();
    let build = |extra: u32| {
        let mut cmd = Command::new(program);
        cmd.args(args).current_dir(root).env("AF_ROOT", root).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | extra);
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::process::CommandExt;
            let _ = extra;
            cmd.process_group(0);
        }
        cmd
    };
    // A job object that allows breakaway lets the waiter leave it; one that does not refuses, and
    // the waiter then starts inside it without.
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    match build(CREATE_BREAKAWAY_FROM_JOB).spawn() {
        Ok(c) => Ok(c.id()),
        Err(_) => build(0).spawn().map(|c| c.id()),
    }
}

/// One line in `system/logs/panel-waiters.log`: the time, then what the waiter did.
pub fn waiter_log(ctx: &Ctx, line: &str) {
    use std::io::Write;
    let path = PathBuf::from(&ctx.root).join(WAITER_LOG);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let flat = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {flat}", crate::core::now_iso());
    }
}

pub fn waiter_log_path() -> &'static str {
    WAITER_LOG
}
