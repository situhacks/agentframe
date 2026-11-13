//! Running an outside program for `af`: no shell, stdin closed, no console window on Windows,
//! output captured, and a deadline past which the process is killed and abandoned.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The Claude Code program: `claude`, or the program `AF_CLAUDE_BIN` names (tests use it).
pub fn claude_bin() -> String {
    match std::env::var("AF_CLAUDE_BIN") {
        Ok(b) if !b.trim().is_empty() => b,
        _ => "claude".to_string(),
    }
}

/// One run: the exit code (None when it timed out or would not start) and its output.
pub struct Out {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }

    pub fn timed_out(&self) -> bool {
        self.code.is_none() && self.stderr == "timed out"
    }

    /// The first message line, for a one-line report.
    pub fn why(&self) -> String {
        let text = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        text.lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with("hint:"))
            .unwrap_or("no message given")
            .trim_start_matches("fatal: ")
            .trim_start_matches("error: ")
            .to_string()
    }
}

/// Run `cmd` with stdin closed and both output streams captured. Past the deadline the process
/// is killed and the run reports `timed out`; with `kill_tree` its child processes die with it
/// (Windows only: elsewhere only the process itself is killed). `name` is what a spawn failure
/// calls the program.
pub fn run_command(mut cmd: Command, name: &str, deadline: Option<Instant>, kill_tree: bool) -> Out {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Out { code: None, stdout: String::new(), stderr: format!("{name} would not start: {e}") },
    };
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        if let Some(mut r) = r {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = r.read_to_end(&mut buf);
                let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
            });
        }
        rx
    };
    let out_rx = pipe(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err_rx = pipe(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().or(Some(1)),
            Ok(None) => {}
            Err(_) => break None,
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            if kill_tree {
                kill_process_tree(child.id());
            }
            let _ = child.kill();
            let _ = child.wait();
            return Out { code: None, stdout: String::new(), stderr: "timed out".into() };
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let wait = |rx: std::sync::mpsc::Receiver<String>| {
        let left = deadline.map(|d| d.saturating_duration_since(Instant::now())).unwrap_or(Duration::from_secs(600));
        rx.recv_timeout(left.max(Duration::from_millis(200))).unwrap_or_default()
    };
    Out { code, stdout: wait(out_rx), stderr: wait(err_rx) }
}

/// `taskkill /T /F`: the process and every process it started.
#[cfg(windows)]
fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(not(windows))]
fn kill_process_tree(_pid: u32) {}
