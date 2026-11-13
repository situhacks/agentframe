//! `af save`: commit the private layer. `workspace/` is its own git repository on this
//! machine; the save stages everything in it and commits. It is local only: it never
//! fetches, pulls, merges or pushes, and never touches the public repository around it.
//! The stop hook runs the same save, bounded by a deadline, and logs what it skips.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::life::vpath;
use crate::outln;
use crate::proc::{run_command, Out};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    match save(ctx, false, Some(HOOK_BUDGET)) {
        Saved::Skipped(why) => crate::runlog::log_note(ctx, HOOK_VERB, &format!("skipped: {why}")),
        Saved::Failed(why) => crate::runlog::log_note(ctx, HOOK_VERB, &format!("failed: {why}")),
        _ => {}
    }
}

/// Git's own cleanup runs only for the manual save. A hook never starts it: a cleanup cut off
/// by the hook's deadline is the one git step that could leave work half done.
pub fn cleanup_runs(budget: Option<Duration>) -> bool {
    budget.is_none()
}

/// What a git operation in progress in the repository is, in plain words; the save leaves
/// the repository alone while one runs. A lock file is an operation too.
fn operation_in_progress(dotgit: &Path) -> Option<&'static str> {
    if dotgit.join("index.lock").exists() {
        return Some("a git lock is held (workspace/.git/index.lock)");
    }
    let marks: [(&str, &str, bool); 6] = [
        ("MERGE_HEAD", "a merge is in progress in workspace/", false),
        ("rebase-merge", "a rebase is in progress in workspace/", true),
        ("rebase-apply", "a rebase is in progress in workspace/", true),
        ("CHERRY_PICK_HEAD", "a cherry-pick is in progress in workspace/", false),
        ("REVERT_HEAD", "a revert is in progress in workspace/", false),
        ("BISECT_LOG", "a bisect is in progress in workspace/", false),
    ];
    marks.iter().find(|(name, _, dir)| if *dir { dotgit.join(name).is_dir() } else { dotgit.join(name).is_file() }).map(|m| m.1)
}

fn files(n: usize) -> &'static str {
    if n == 1 {
        "file"
    } else {
        "files"
    }
}

/// Stage and commit everything in `workspace/`. With `budget`, every git step is killed at
/// the deadline and the save is skipped; a killed step leaves its lock file alone.
pub fn save(ctx: &Ctx, dry: bool, budget: Option<Duration>) -> Saved {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    if !ws.join(".git").exists() {
        return Saved::NotRepo;
    }
    let g = Git::new(&ws, budget.map(|b| Instant::now() + b));
    let dotgit = git_dir(&g);
    if let Some(why) = operation_in_progress(&dotgit) {
        return Saved::Skipped(why.into());
    }
    let failed = |what: &str, o: &Out| -> Saved {
        if o.timed_out() {
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
    if cleanup_runs(budget) {
        g.cleanup();
    }
    Saved::Committed(n)
}

/// A git lock this old was left by a git that died.
const STALE_LOCK: Duration = Duration::from_secs(10 * 60);
/// A private layer with changes and no commit for this long is not being saved.
const UNSAVED_AFTER: Duration = Duration::from_secs(24 * 3600);
/// What the doctor lets git take to answer; past it, it reports nothing.
const DOCTOR_BUDGET: Duration = Duration::from_secs(3);

/// The doctor's check of the private layer: a stale git lock that blocks the save, and
/// changes left uncommitted for a day. Cheap, and silent when git cannot answer in time.
pub fn doctor_issues(ctx: &Ctx) -> Vec<String> {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    let mut issues = Vec::new();
    if !ws.join(".git").exists() {
        return issues;
    }
    let g = Git::plain(&ws, Some(Instant::now() + DOCTOR_BUDGET));
    let dotgit = git_dir(&g);
    let mut locks: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dotgit) {
        locks.extend(rd.flatten().map(|e| e.path()).filter(|p| {
            p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".lock") || n == "gc.pid")
        }));
    }
    locks.extend(
        walkdir::WalkDir::new(dotgit.join("refs").join("heads"))
            .into_iter()
            .flatten()
            .map(|e| e.into_path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "lock")),
    );
    locks.sort();
    for lock in locks {
        let age = std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
        if let Some(age) = age.filter(|a| *a > STALE_LOCK) {
            let rel = lock.strip_prefix(&dotgit).map(|r| r.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/")).unwrap_or_default();
            issues.push(format!(
                "workspace/.git/{rel} is {} minutes old and can stop the private layer from saving; confirm no git is running, then delete that file",
                age.as_secs() / 60
            ));
        }
    }
    let status = g.run(&["status", "--porcelain"]);
    if status.ok() && !status.stdout.trim().is_empty() {
        let last = g.text(&["log", "-1", "--format=%ct"]).and_then(|s| s.parse::<u64>().ok());
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Some(age) = last.map(|c| now.saturating_sub(c)).filter(|a| *a > UNSAVED_AFTER.as_secs()) {
            issues.push(format!(
                "workspace/ has uncommitted changes and its last commit is {} hours old, so the private layer is not being saved; run af save to see why",
                age / 3600
            ));
        }
    }
    issues
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

/// git in the private layer's folder, with this crate's fixed settings.
struct Git {
    dir: PathBuf,
    pre: Vec<String>,
    deadline: Option<Instant>,
}

impl Git {
    /// git with the fixed settings and no identity probe, for a read.
    fn plain(dir: &Path, deadline: Option<Instant>) -> Git {
        let pre = ["--no-optional-locks", "-c", "core.quotepath=off"];
        Git { dir: dir.to_path_buf(), pre: pre.iter().map(|s| s.to_string()).collect(), deadline }
    }

    fn new(dir: &Path, deadline: Option<Instant>) -> Git {
        let mut g = Git::plain(dir, deadline);
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
    cmd.args(args).env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never");
    run_command(cmd, "git", deadline)
}
