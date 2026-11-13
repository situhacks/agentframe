//! `af save`: commit the private layer. `workspace/` is its own git repository on this
//! machine; the save stages everything in it and commits. It is local only: it never
//! fetches, pulls, merges or pushes, and never touches the public repository around it.
//! The stop hook runs the same save, bounded by a deadline, and logs what it skips.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::life::vpath;
use crate::outln;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

/// How long the stop hook lets git run before it gives up and logs a skip.
pub const HOOK_BUDGET: Duration = Duration::from_secs(20);
/// The run-log verb of the stop hook's save.
pub const HOOK_VERB: &str = "hook stop save";

/// Inherited from an outer git (af started by a git hook), these would point every git step
/// at that repository instead of the one named with `-C`.
const OUTER_GIT_VARS: [&str; 12] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_QUARANTINE_PATH",
    "GIT_SHALLOW_FILE",
    "GIT_GRAFT_FILE",
    "GIT_PREFIX",
    "GIT_IMPLICIT_WORK_TREE",
];

/// What one save did.
#[derive(Debug, PartialEq, Eq)]
pub enum Saved {
    /// `workspace/` is not a git repository: nothing to save.
    NotRepo,
    /// Left alone for a reason that clears by itself or is his to look at.
    Skipped(String),
    /// The repository has no change to commit.
    Nothing,
    /// A commit of this many files.
    Committed(usize),
    /// A dry run: this many files would be committed.
    Would(usize),
    /// git refused a step.
    Failed(String),
}

/// `af save [--dry-run]`.
pub fn run(ctx: &Ctx, dry_run: bool) -> R<i32> {
    match save(ctx, dry_run, None) {
        Saved::NotRepo => outln!("workspace/ is not a git repo, so there is nothing to save."),
        Saved::Skipped(why) => outln!("Skipped: {why}."),
        Saved::Nothing | Saved::Would(0) => outln!("Nothing to save."),
        Saved::Committed(n) => outln!("Saved {n} {} in the private layer.", files(n)),
        Saved::Would(n) => outln!("Would commit {n} {}.", files(n)),
        Saved::Failed(why) => die!("save failed: {why}"),
    }
    Ok(0)
}

/// The stop hook's save: bounded, silent, and a skip or failure becomes one run-log note.
pub fn hook(ctx: &Ctx) {
    if let Saved::Skipped(why) | Saved::Failed(why) = save(ctx, false, Some(HOOK_BUDGET)) {
        crate::runlog::log_note(ctx, HOOK_VERB, &format!("skipped: {why}"));
    }
}

fn files(n: usize) -> &'static str {
    if n == 1 {
        "file"
    } else {
        "files"
    }
}

/// Stage and commit everything in `workspace/`. With `budget`, every git step is killed at
/// the deadline and the save is skipped.
pub fn save(ctx: &Ctx, dry: bool, budget: Option<Duration>) -> Saved {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    if !ws.join(".git").exists() {
        return Saved::NotRepo;
    }
    let started = SystemTime::now();
    let g = Git::new(&ws, budget.map(|b| Instant::now() + b));
    let dotgit = git_dir(&g);
    if dotgit.join("index.lock").exists() {
        return Saved::Skipped("a git lock is held (workspace/.git/index.lock)".into());
    }
    if dotgit.join("MERGE_HEAD").is_file() {
        return Saved::Skipped("a merge is in progress in workspace/".into());
    }
    let failed = |what: &str, o: &Out| -> Saved {
        if o.timed_out() {
            // The killed git may have left its own lock behind; one made since this save
            // began is ours, and left in place it would block every later save.
            let lock = dotgit.join("index.lock");
            if std::fs::metadata(&lock).and_then(|m| m.modified()).is_ok_and(|t| t >= started) {
                let _ = std::fs::remove_file(&lock);
            }
            Saved::Skipped(format!("git took too long to {what}"))
        } else {
            Saved::Failed(format!("{what}: {}", o.why()))
        }
    };
    if dry {
        let s = g.run(&["status", "--porcelain", "-z", "--untracked-files=all"]);
        return if s.ok() { Saved::Would(count_status(&s.stdout)) } else { failed("read the status", &s) };
    }
    let a = g.run(&["add", "-A"]);
    if !a.ok() {
        return failed("stage the changes", &a);
    }
    let staged = g.run(&["diff", "--cached", "--name-only", "-z"]);
    if !staged.ok() {
        return failed("list the staged files", &staged);
    }
    let n = staged.stdout.split('\0').filter(|s| !s.is_empty()).count();
    if n == 0 {
        return Saved::Nothing;
    }
    let c = g.run(&["commit", "-q", "--no-verify", "-m", &commit_message()]);
    if !c.ok() {
        return failed("commit", &c);
    }
    g.cleanup();
    Saved::Committed(n)
}

/// The entries `git status --porcelain -z` lists: a rename or copy carries its source as a
/// second path, which is the same file.
fn count_status(z: &str) -> usize {
    let mut n = 0;
    let mut parts = z.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        n += 1;
        if matches!(entry.chars().next(), Some('R' | 'C')) {
            parts.next();
        }
    }
    n
}

fn machine() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| "this machine".into())
}

fn commit_message() -> String {
    format!("af save: private layer on {} at {}", machine(), chrono::Local::now().format("%Y-%m-%d %H:%M"))
}

/// The repository's git folder: `.git`, or the folder a linked checkout's `.git` file names.
fn git_dir(g: &Git) -> PathBuf {
    let dot = g.dir.join(".git");
    if dot.is_dir() {
        return dot;
    }
    g.text(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from).unwrap_or(dot)
}

// ------------------------------------------------------------------ git

/// One git run: the exit code (None when it timed out or would not start) and its output.
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

    /// git's first message line, for a one-line report.
    pub fn why(&self) -> String {
        let text = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        text.lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with("hint:"))
            .unwrap_or("git gave no message")
            .trim_start_matches("fatal: ")
            .trim_start_matches("error: ")
            .to_string()
    }
}

/// git in the private layer's folder, with this crate's fixed settings.
struct Git {
    dir: PathBuf,
    pre: Vec<String>,
    deadline: Option<Instant>,
}

impl Git {
    fn new(dir: &Path, deadline: Option<Instant>) -> Git {
        let pre = ["--no-optional-locks", "-c", "core.quotepath=off"];
        let mut g = Git { dir: dir.to_path_buf(), pre: pre.iter().map(|s| s.to_string()).collect(), deadline };
        // A machine with no git identity still commits its own private layer.
        let has = |g: &Git, k: &str| !g.run(&["config", "--get", k]).stdout.trim().is_empty();
        if !(has(&g, "user.name") && has(&g, "user.email")) {
            g.pre.extend(["-c", "user.name=AgentFrame", "-c", "user.email=af-save@localhost"].iter().map(|s| s.to_string()));
        }
        g
    }

    fn full(&self, args: &[&str]) -> Vec<String> {
        let mut full: Vec<String> = vec!["-C".into(), self.dir.to_string_lossy().into_owned()];
        full.extend(self.pre.iter().cloned());
        full.extend(args.iter().map(|s| s.to_string()));
        full
    }

    /// No step starts git's own cleanup detached: the save runs it once, in the foreground,
    /// after its commit.
    fn run(&self, args: &[&str]) -> Out {
        let mut full = self.full(&["-c", "gc.auto=0", "-c", "maintenance.auto=false"]);
        full.extend(args.iter().map(|s| s.to_string()));
        run_git(&full, self.deadline, self.dir.parent())
    }

    fn text(&self, args: &[&str]) -> Option<String> {
        let o = self.run(args);
        o.ok().then(|| o.stdout.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// git's own cleanup, when git judges it due, in the foreground.
    fn cleanup(&self) {
        let _ = run_git(&self.full(&["-c", "gc.autoDetach=false", "gc", "--auto", "--quiet"]), self.deadline, self.dir.parent());
    }
}

/// Run git without a shell, stdin closed, never prompting; past the deadline the process is
/// killed and abandoned. Git never looks for a repository above `ceiling`, so a broken
/// `.git` in `workspace/` can never make it act on the public repository around it.
fn run_git(args: &[String], deadline: Option<Instant>, ceiling: Option<&Path>) -> Out {
    let mut cmd = Command::new("git");
    for v in OUTER_GIT_VARS {
        cmd.env_remove(v);
    }
    if let Some(c) = ceiling {
        cmd.env("GIT_CEILING_DIRECTORIES", c);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Out { code: None, stdout: String::new(), stderr: format!("git would not start: {e}") },
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
