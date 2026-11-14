//! The Claude panel from `af`: `system/tools/claude_panel.ps1` manages chats in the VS Code panel
//! (sends text to a chat, starts a new one, lists the chats) through UI Automation and the AgentFrame
//! bridge extension, with no key or mouse input, and these verbs build its calls. Text travels in a
//! UTF-8 file, never as an argument, and reaches the driver exactly as given: a text that starts with
//! `/` is not changed. The driver's exit code says why it did not act: 3 the operator was at the
//! machine, the chat was busy or another driver run held the panel for the whole wait, 4 the chat's
//! input holds text that is not the driver's, 5 a safety check failed; on all three nothing was sent
//! (nothing was archived, for `archive`), and the driver's message says what it changed and where the
//! chat's tab is now. 2 the bridge, window, chat or control was not found, 6 the global model key or the
//! settings file did not match after a model pick, 7 the run was cancelled through its cancel file before it
//! changed anything. Its notes (lines `note: ...` on stderr) are printed on success too.

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The driver program: `AF_PANEL_DRIVER` names another program (tests use a stub), else
/// `system/tools/claude_panel.ps1` under Windows PowerShell.
pub const DRIVER_ENV: &str = "AF_PANEL_DRIVER";
/// How long the driver waits for the operator and the chat when asked for no other time
/// (the driver's own `-WaitMinutes` default).
pub const DRIVER_WAIT_MINUTES: f64 = 20.0;
/// The time a driver run may take beyond its wait: the driver's `$WorkSeconds`, the same number. It
/// bounds every step by the wait plus this, and puts the chat back inside it.
pub const DRIVER_WORK_SECS: u64 = 1200;
/// What af allows on top of the driver's own deadline, for PowerShell to start and end.
pub const DRIVER_MARGIN_SECS: u64 = 60;
/// A list waits at most this long (for a minimized window it must restore, or for the lock).
pub const LIST_WAIT_MINUTES: f64 = 0.5;
/// The driver's work time for a read-only run (`list`): its `$ListWorkSeconds`, the same number.
pub const DRIVER_LIST_WORK_SECS: u64 = 45;
/// af's whole limit for a read-only run: its wait, its work and a margin. A list never holds af for
/// the 22 minutes a send may take.
pub const READ_LIMIT_SECS: u64 = 90;
/// Test hook: seconds that replace the computed limit, to prove what happens when af has to kill the driver.
const LIMIT_ENV: &str = "AF_PANEL_LIMIT_SECS";
const WAITER_LOG: &str = "system/logs/panel-waiters.log";
const DRIVER_LOG: &str = "system/logs/claude-panel.log";
/// The lock every driver run that changes anything holds, under the vault root.
const LOCK_FILE: &str = "system/logs/panel-bridge/driver.lock";

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
    /// `-CancelFile`: the run stops with exit 7 when this file exists while it has changed nothing yet.
    pub cancel_file: Option<&'a Path>,
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
        if let Some(f) = c.cancel_file {
            a.extend(["-CancelFile".to_string(), f.to_string_lossy().into_owned()]);
        }
        a
    }

    /// The call as one line, for a dry run.
    pub fn shown(&self, c: &Call) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args(c));
        parts.iter().map(|p| if p.contains(' ') { format!("\"{p}\"") } else { p.clone() }).collect::<Vec<_>>().join(" ")
    }

    /// Runs the driver within its own deadline plus a margin. If af has to kill it, the driver could
    /// neither put the chat back nor log, so af logs the kill itself and the answer says so.
    fn run(&self, ctx: &Ctx, c: &Call) -> Out {
        let limit = action_limit(c.action, c.wait_minutes);
        let mut cmd = Command::new(&self.program);
        cmd.args(self.args(c)).current_dir(&ctx.root);
        let out = run_command(cmd, "the panel driver", Some(Instant::now() + limit), true);
        if out.timed_out() {
            driver_log(
                ctx,
                &format!("{} {} exit=killed-by-af secs={} [af stopped the driver at its limit; nothing was put back]", c.action, c.session.unwrap_or("-"), limit.as_secs()),
            );
            return Out {
                code: None,
                stdout: String::new(),
                stderr: format!(
                    "af killed the panel driver: it ran past its limit of {} seconds (its wait, {} seconds of work and a {} second margin). A killed driver cannot put the chat back or log, so look at the chat now: its tab may be closed, or open with the text unsent",
                    limit.as_secs(),
                    DRIVER_WORK_SECS,
                    DRIVER_MARGIN_SECS
                ),
            };
        }
        out
    }
}

/// af's limit for one driver run: the driver's own deadline (its wait, default 20 minutes, plus its
/// work time) and a margin. The driver bounds every step by its deadline, so af kills it only when
/// something hangs.
pub fn driver_limit(wait_minutes: Option<f64>) -> Duration {
    if let Some(s) = std::env::var(LIMIT_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|s| *s > 0.0) {
        return Duration::from_secs_f64(s);
    }
    let wait = wait_minutes.unwrap_or(DRIVER_WAIT_MINUTES).max(0.0);
    Duration::from_secs_f64(wait * 60.0) + Duration::from_secs(DRIVER_WORK_SECS + DRIVER_MARGIN_SECS)
}

/// af's limit for one run of this action: a read-only action (`list`) has its own short limit, every
/// other action the driver's deadline plus a margin.
pub fn action_limit(action: &str, wait_minutes: Option<f64>) -> Duration {
    if action == "list" {
        if let Some(s) = std::env::var(LIMIT_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|s| *s > 0.0) {
            return Duration::from_secs_f64(s);
        }
        return Duration::from_secs(READ_LIMIT_SECS);
    }
    driver_limit(wait_minutes)
}

/// The vault root: the main checkout, also when af runs from a worktree under `.claude/worktrees/`.
pub fn vault_root(root: &str) -> PathBuf {
    // ASCII lower case keeps every byte where it was, so the index fits the original.
    let unified = root.replace('\\', "/").to_ascii_lowercase();
    match unified.find("/.claude/worktrees/") {
        Some(i) => PathBuf::from(&root[..i]),
        None => PathBuf::from(root),
    }
}

/// One line in the driver's own log, `system/logs/claude-panel.log` under the vault root.
fn driver_log(ctx: &Ctx, line: &str) {
    use std::io::Write;
    let path = vault_root(&ctx.root).join(DRIVER_LOG);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = write!(f, "{} {line}\r\n", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"));
    }
}

/// Whether a process with this id is running now.
#[cfg(windows)]
pub fn pid_alive(pid: u32) -> bool {
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetExitCodeProcess(handle: isize, code: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    if pid == 0 {
        return false;
    }
    // SAFETY: OpenProcess takes plain values; the handle is checked, read once and closed.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code == STILL_ACTIVE
    }
}

#[cfg(not(windows))]
pub fn pid_alive(pid: u32) -> bool {
    pid != 0 && Path::new(&format!("/proc/{pid}")).exists()
}

/// When a running process started, as a Windows FILETIME (100 ns since 1601, the number the driver
/// and Claude's own session records write); None when it is not running or cannot be read.
#[cfg(windows)]
pub fn pid_start(pid: u32) -> Option<u64> {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetProcessTimes(handle: isize, creation: *mut FileTime, exit: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    if pid == 0 || !pid_alive(pid) {
        return None;
    }
    // SAFETY: plain values in, four out-structs owned here; the handle is checked and closed.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return None;
        }
        let (mut c, mut e, mut k, mut u) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        CloseHandle(h);
        (ok != 0).then(|| ((c.high as u64) << 32) | c.low as u64)
    }
}

#[cfg(not(windows))]
pub fn pid_start(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit(')').next()?.split_whitespace().nth(19)?.parse().ok()
}

/// Whether this process id is the process that was recorded: running, and, when its start time was
/// recorded, started then (a pid reused after a reboot or an exit is another process).
pub fn process_is(pid: u32, start: Option<u64>) -> bool {
    if !pid_alive(pid) {
        return false;
    }
    match start {
        Some(s) => pid_start(pid) == Some(s),
        None => true,
    }
}

/// A recorded start time from JSON: a number or a string of digits.
pub fn start_of(v: &Value, key: &str) -> Option<u64> {
    match v.get(key)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Who holds the panel lock now, from its file: pid, session id, action. A lock whose process is gone
/// (its pid is dead, or now belongs to a process that started at another time) is left over, and
/// does not count; nor does this process's own lock.
pub fn lock_info(ctx: &Ctx) -> Option<(u32, String, String)> {
    let text = std::fs::read_to_string(vault_root(&ctx.root).join(LOCK_FILE)).ok()?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let pid = v.get("pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok())?;
    if pid == std::process::id() || !process_is(pid, start_of(&v, "pid_start")) {
        return None;
    }
    let sid = v.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
    let action = v.get("action").and_then(Value::as_str).unwrap_or("").to_string();
    Some((pid, sid, action))
}

/// The pid of a live run that holds the panel lock for this session, if any.
pub fn lock_holder(ctx: &Ctx, session_id: &str) -> Option<u32> {
    lock_info(ctx).filter(|(_, sid, _)| sid == session_id).map(|(pid, _, _)| pid)
}

/// The panel lock held by af itself (the closed-route compaction): the same file the driver holds,
/// opened for writing and shared for reading only, so a driver run cannot take it meanwhile and waits.
/// Released (and the file removed) when dropped.
pub struct PanelLock {
    file: Option<std::fs::File>,
    path: PathBuf,
}

impl Drop for PanelLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Who holds the panel lock, in words: its action and session, from the lock file.
pub fn lock_holder_text(ctx: &Ctx) -> String {
    match lock_info(ctx) {
        Some((pid, sid, act)) => format!(
            "the panel lock is held by a {} run for session {} (pid {pid})",
            if act.is_empty() { "panel".to_string() } else { format!("'{act}'") },
            if sid.is_empty() { "-".to_string() } else { sid }
        ),
        None => "the panel lock is held, and its holder could not be read".to_string(),
    }
}

/// Take the panel lock, waiting for it up to `budget`; then refuse, naming the holder.
pub fn acquire_lock_within(ctx: &Ctx, session_id: &str, action: &str, budget: Duration) -> Result<PanelLock, String> {
    let until = Instant::now() + budget;
    loop {
        match acquire_lock(ctx, session_id, action) {
            Ok(l) => return Ok(l),
            Err(m) if Instant::now() >= until => return Err(format!("{m}, for the whole wait of {} seconds", budget.as_secs())),
            Err(_) if cancel_requested() => {
                note_cancelled();
                return Err("cancelled while waiting for the panel lock; nothing was changed".to_string());
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// Take the panel lock for this session and action, or say who holds it.
pub fn acquire_lock(ctx: &Ctx, session_id: &str, action: &str) -> Result<PanelLock, String> {
    use std::io::Write;
    let path = vault_root(&ctx.root).join(LOCK_FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ only: the driver's open for writing fails while af holds it.
        opts.share_mode(1);
    }
    let mut file = match opts.open(&path) {
        Ok(f) => f,
        Err(e) => return Err(format!("{} ({e})", lock_holder_text(ctx))),
    };
    let me = std::process::id();
    let body = json!({"pid": me, "pid_start": pid_start(me), "session_id": session_id, "action": action, "started": crate::core::now_iso()}).to_string();
    file.set_len(0).and_then(|_| file.write_all(body.as_bytes())).and_then(|_| file.flush()).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(PanelLock { file: Some(file), path })
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

// ------------------------------------------------------------------ cancelling a queued run

/// The cancel marker of a detached waiter: its job file with the extension `.cancel`.
pub fn cancel_marker(job: &Path) -> PathBuf {
    job.with_extension("cancel")
}

/// The file the driver writes when it first changes the panel (and the closed route writes when it starts
/// the compaction): from then on a cancel no longer stops the run.
pub fn sending_marker(job: &Path) -> PathBuf {
    job.with_extension("sending")
}

/// In a waiter process: the marker its waits and its driver runs look for.
static CANCEL_FILE: OnceLock<PathBuf> = OnceLock::new();
/// Cleared once the waiter has begun changing things: a cancel is no longer looked for.
static CANCEL_ON: AtomicBool = AtomicBool::new(false);
/// Set when a wait or a driver run ended because of the marker.
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// In a waiter: from now on its waits and driver runs stop for this marker.
pub fn set_cancel_file(marker: PathBuf) {
    let _ = CANCEL_FILE.set(marker);
    CANCEL_ON.store(true, Ordering::SeqCst);
}

/// The marker to pass to the driver, while cancelling is still possible.
fn cancel_file() -> Option<PathBuf> {
    if CANCEL_ON.load(Ordering::SeqCst) {
        CANCEL_FILE.get().cloned()
    } else {
        None
    }
}

/// The waiter has begun changing things (or the driver did): stop looking for a cancel.
pub fn cancel_off() {
    CANCEL_ON.store(false, Ordering::SeqCst);
}

/// A cancel marker exists and cancelling is still possible.
pub fn cancel_requested() -> bool {
    cancel_file().is_some_and(|f| f.exists())
}

pub fn note_cancelled() {
    CANCELLED.store(true, Ordering::SeqCst);
}

pub fn was_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

/// The closed route is about to change something: refuse when a cancel is waiting, else leave the
/// `.sending` marker and stop looking for a cancel.
pub fn begin_changes() -> Result<(), String> {
    if cancel_requested() {
        note_cancelled();
        return Err("cancelled before anything was changed".to_string());
    }
    if let Some(f) = CANCEL_FILE.get().filter(|_| CANCEL_ON.load(Ordering::SeqCst)) {
        let _ = std::fs::write(f.with_extension("sending"), crate::core::now_iso());
    }
    cancel_off();
    Ok(())
}

/// What the driver printed on success: its output and its notes.
#[derive(Debug, Default)]
pub struct Answer {
    pub value: String,
    pub notes: Vec<String>,
}

impl Answer {
    /// The notes as a sentence to append to a report, empty when there are none.
    pub fn notes_sentence(&self) -> String {
        if self.notes.is_empty() {
            String::new()
        } else {
            format!(" Driver notes: {}.", self.notes.iter().map(|n| n.trim_end_matches('.')).collect::<Vec<_>>().join("; "))
        }
    }
}

fn notes_of(stderr: &str) -> Vec<String> {
    stderr.lines().filter_map(|l| l.trim().strip_prefix("note: ")).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()
}

/// Every message line the driver printed (its reason, then its notes), as one line.
fn whole(out: &Out) -> String {
    let lines: Vec<&str> = out.stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        out.why()
    } else {
        lines.join(" ")
    }
}

/// The driver's answer: its output and notes on exit 0, else the sentence that says what it refused
/// and why, with everything the driver said (what it changed, where the tab is now, its notes).
fn answer(out: &Out) -> Result<Answer, String> {
    answer_as(out, "sent")
}

/// The same, for a verb whose refusals say what was not done: `sent`, or `archived`.
fn answer_as(out: &Out, nothing: &str) -> Result<Answer, String> {
    let why = whole(out);
    match out.code {
        Some(0) => Ok(Answer { value: out.stdout.trim().to_string(), notes: notes_of(&out.stderr) }),
        Some(2) => Err(format!("the panel driver could not find what it needs (the bridge extension, the VS Code window, the chat or a control): {why}. A chat running in a terminal has no panel row")),
        Some(3) => Err(format!("refused (exit 3): nothing was {nothing}. The operator was using the computer, the chat was busy or running, the chat is the one he has in front (archive), a full-screen window stayed in front of its row (archive), or another driver run held the panel, for the whole wait. {why}")),
        Some(4) => Err(format!("refused (exit 4): nothing was {nothing}. The chat's input holds a draft typed in its input box or other text that is not the driver's; ask him to send or clear it. {why}")),
        Some(5) => Err(format!("refused (exit 5): a safety check failed, so nothing was {nothing}. {why}")),
        Some(7) => {
            note_cancelled();
            Err(format!("cancelled (exit 7): the run was cancelled while it was only waiting, and nothing was changed. {why}"))
        }
        Some(6) => Err(format!("MODEL KEY MISMATCH (exit 6): after a model pick the global model key or ~/.claude/settings.json did not match what the driver saved, and the driver could not put it back. Fix settings.json by hand before anything else runs. {why}")),
        Some(n) => Err(format!("the panel driver failed (exit {n}): {why}")),
        None => Err(format!("the panel driver did not run to the end: {why}")),
    }
}

/// Send the text to the chat; the driver's notes. Err carries the refusal sentence.
pub fn send(ctx: &Ctx, session: &str, text: &str, wait_minutes: Option<f64>) -> Result<Answer, String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let cancel = cancel_file();
    let call = Call { action: "send", session: Some(session), text_file: Some(file.path()), wait_minutes, cancel_file: cancel.as_deref(), ..Call::default() };
    answer(&d.run(ctx, &call))
}

/// Archive the chat with this session id through the panel; the driver's message (what it archived, or
/// that the list has no row for it) and notes. Err carries the refusal sentence.
pub fn archive(ctx: &Ctx, session: &str, wait_minutes: Option<f64>) -> Result<Answer, String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let call = Call { action: "archive", session: Some(session), wait_minutes, ..Call::default() };
    answer_as(&d.run(ctx, &call), "archived")
}

/// A new chat with this first message; its session id and the driver's notes.
pub fn new_chat(ctx: &Ctx, text: &str, model: Option<&str>, group_id: Option<&str>, wait_minutes: Option<f64>) -> Result<(String, Answer), String> {
    let d = driver(ctx).map_err(|e| e.to_string())?;
    let file = TextFile::new(ctx, text).map_err(|e| e.to_string())?;
    let call = Call { action: "new", text_file: Some(file.path()), model, group_id, wait_minutes, ..Call::default() };
    let a = answer(&d.run(ctx, &call))?;
    let id = a.value.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string();
    if id.len() < 8 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(format!("the panel driver sent the message but printed no session id (it printed '{id}')"));
    }
    Ok((id, a))
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
    let out = d.run(ctx, &Call { action: "list", wait_minutes: Some(LIST_WAIT_MINUTES), ..Call::default() });
    let text = answer(&out)?.value;
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
    let sent = match send(ctx, &row.session_id, &format!("/rename {to}"), wait_minutes) {
        Ok(a) => a,
        Err(m) => die!("{name}: {m}"),
    };
    for n in &sent.notes {
        outln!("note: {n}");
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

/// A session id as the driver takes it: hex digits and dashes.
fn valid_session_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// `af session archive [<name>] [--session-id <id>] [--wait-minutes N] [--dry-run]`: archive a chat in the
/// panel's list. The driver closes the chat's open tab first (when it has one and it holds no draft), rests
/// the pointer on the row and invokes the row's Archive button. The registry row is not changed.
pub fn cmd_archive(ctx: &Ctx, name: Option<&str>, session_id: Option<&str>, wait_minutes: Option<f64>, dry_run: bool) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let session_id = session_id.map(str::trim).filter(|s| !s.is_empty());
    let (label, id, registered) = match (name, session_id) {
        (None, None) => die!("af session archive needs a chat name, or --session-id <id> for a chat with no registry row"),
        (Some(n), sid) => {
            let row = reach::row_or_refuse(&reg, n)?;
            if let Some(s) = sid {
                if s != row.session_id {
                    die!("{n}: --session-id {s} is not the session id of that registry row ({}); give the name or the id, not two different chats", row.session_id);
                }
            }
            (n.to_string(), row.session_id, true)
        }
        (None, Some(s)) => {
            if !valid_session_id(s) {
                die!("--session-id is a session id (8 to 64 hex digits and dashes), got '{s}'; nothing was archived");
            }
            let known = reg.find_session(s).map(|r| r.name);
            (known.clone().unwrap_or_else(|| s.to_string()), s.to_string(), known.is_some())
        }
    };
    if let Some(me) = crate::compact::caller_session() {
        if me == id {
            die!("{label}: this is the chat that is running this command; af session archive does not archive the calling chat. Ask him, or another chat, to archive it from outside; nothing was archived");
        }
    }
    let d = driver(ctx)?;
    let call = Call { action: "archive", session: Some(&id), wait_minutes, ..Call::default() };
    if dry_run {
        outln!("{label}: would archive session {id} through the panel driver:");
        outln!("{}", d.shown(&call));
        outln!("The driver closes the chat's open tab first (a draft in it refuses, exit 4), rests the pointer on its row in the panel's list, invokes the row's Archive button and puts the pointer back. It never takes the foreground. The registry row is not changed. Nothing was run.");
        return Ok(0);
    }
    match archive(ctx, &id, wait_minutes) {
        Ok(a) => {
            outln!("{label}: {}{}", a.value, a.notes_sentence());
            if registered {
                outln!("The registry row for {label} is unchanged. To mark it ended: af session end {label} --reason other.");
            }
            Ok(0)
        }
        Err(m) => die!("{label}: {m}"),
    }
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
        Ok((id, a)) => {
            for n in &a.notes {
                outln!("note: {n}");
            }
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
