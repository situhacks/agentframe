//! The Claude panel from `af`: `system/tools/claude_panel.ps1` drives the VS Code panel (types into
//! a chat, opens a new one, lists the chats), and these verbs build its calls. Text travels in a
//! UTF-8 file, never as an argument. A leading `/` gets exactly one leading space, or the slash
//! palette takes the keystrokes. The driver's exit code says why it did not act: 3 the operator was
//! at the machine or the chat was running for the whole wait, 4 a draft sits in the chat's input,
//! 5 a safety check failed before typing, 2 the window, chat or control was not found.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::outln;
use crate::proc::{run_command, Out};
use crate::reach;
use crate::sessions::{self, Registry};
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

/// The text as typed: a leading `/` becomes ` /`, so the slash palette does not open. A text that
/// already starts with a space is left alone, so the space is there exactly once.
pub fn typed(text: &str) -> String {
    if text.starts_with('/') {
        format!(" {text}")
    } else {
        text.to_string()
    }
}

/// A file holding the text to type, under `system/logs/claude-panel/`; removed when dropped unless kept.
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
        if let Err(e) = std::fs::write(&path, typed(text).as_bytes()) {
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
        Some(2) => Err(format!("the panel driver could not find what it needs: {why}. A chat running in a terminal has no panel row")),
        Some(3) => Err(format!("refused (exit 3): the operator was using the computer, or the chat was running, for the whole wait; nothing was typed. {why}")),
        Some(4) => Err(format!("refused (exit 4): the chat has a draft typed in its input box; nothing was sent. Ask him to send or clear it. {why}")),
        Some(5) => Err(format!("refused (exit 5): a safety check failed before typing, so nothing was typed. {why}")),
        Some(n) => Err(format!("the panel driver failed (exit {n}): {why}")),
        None => Err(format!("the panel driver did not run to the end: {why}")),
    }
}

/// Open the chat, type the text, press Enter. Err carries the refusal sentence.
pub fn send(ctx: &Ctx, session: &str, text: &str, wait_minutes: Option<f64>) -> Result<(), String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let call = Call { action: "send", session: Some(session), text_file: Some(file.path()), wait_minutes, ..Call::default() };
    answer(&d.run(ctx, &call, limit_for(wait_minutes, Duration::ZERO))).map(|_| ())
}

/// A new chat with this first message; its session id.
pub fn new_chat(ctx: &Ctx, text: &str, model: Option<&str>, wait_minutes: Option<f64>) -> Result<String, String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let call = Call { action: "new", text_file: Some(file.path()), model, wait_minutes, ..Call::default() };
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

/// `af session rename <name> [--to <text>]`: send ` /rename <text>` to the chat, then read the panel's row back.
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

/// `af session start <name> --brief-prompt-file <path> --model <m>`: a new chat whose first message is the file.
/// The model is required: a new chat inherits the default model otherwise.
pub fn cmd_start(ctx: &Ctx, name: &str, prompt_file: &str, model: Option<&str>, wait_minutes: Option<f64>) -> R<i32> {
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
            "{name}: --model is needed: a new chat would inherit the default model, which is the expensive one.              The model picker shows: {}. (`claude_panel.ps1 model -Session <id>` lists the live options.)",
            MODEL_OPTIONS.join(", ")
        )
    };
    match new_chat(ctx, &text, Some(model), wait_minutes) {
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
