//! `af sync`: two machines kept in step through a hub folder. The hub holds two bare git
//! repositories, `agentframe.git` (this public repository) and `workspace.git` (the private
//! layer), and a `files/` tree for the private files neither repository carries. Both
//! checkouts reach it through a remote named `hub`; `origin` is never touched. Everything
//! shells out to `git`, so the crate needs no git library.
//!
//! The private layer is committed, merged and pushed on every run; the public repository is
//! never committed, merged only on a clean `master`, and otherwise pushed only as a
//! fast-forward. A private merge conflict stays in progress and prints one `SYNC_CONFLICT`
//! line per file with exit 3, the crate's "the chat must act" signal. The hooks run a
//! bounded private-only half that skips, never blocks, and logs why.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::life::{vpath, workspace_remotes, TextFile};
use crate::outln;
use clap::Subcommand;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

pub const REMOTE: &str = "hub";
pub const PUBLIC_REPO: &str = "agentframe.git";
pub const PRIVATE_REPO: &str = "workspace.git";
pub const PUBLIC_BRANCH: &str = "master";
/// Under the hub folder: the copy list's mirror of the checkout.
pub const FILES_DIR: &str = "files";
/// The exit code of a signal the chat must act on (as `af session stop` exits 3 with
/// HANDOFF_REQUIRED).
pub const SIGNAL_EXIT: i32 = 3;
pub const CONFLICT_TAG: &str = "SYNC_CONFLICT";
pub const PROCEDURE: &str = "library/process/sync.md";

/// Private files outside both repositories, copied newer-wins between the checkout and
/// `<hub>/files/<same path>`; nothing is ever deleted. `*` matches within one folder, `**`
/// any depth; a `.gitkeep` and a file git tracks never copy.
pub const SYNC_COPY_LIST: [&str; 8] = [
    "system/*backlog*.md",
    ".env",
    "system/tools/.env",
    "workspace/**/.env",
    "library/assets/design-languages/**",
    "library/assets/logos/**",
    "library/assets/media/**",
    "library/assets/audio/**",
];

/// Merge rules for the private repository, relative to `workspace/`. `union` keeps both
/// sides' lines: only for files that are append-only by their schema. `ours` keeps this
/// machine's copy of a file the sync re-renders after the merge. Every other file merges
/// line by line and conflicts on a same-line edit.
pub const MERGE_RULES: [(&str, &str); 6] = [
    ("life/sources/INDEX.md", "union"),
    ("projects/**/sources/INDEX.md", "union"),
    ("life/log/*.md", "union"),
    ("life/tags-log.md", "union"),
    ("life/needs.md", "ours"),
    ("life/tags.md", "ours"),
];
pub const ATTR_BEGIN: &str = "# BEGIN af sync merge rules (af sync rewrites this block; edit outside it)";
pub const ATTR_END: &str = "# END af sync merge rules";

/// The keep-ours driver lives on the command line, so no machine needs persistent git config.
const OURS_DRIVER: &str = "merge.ours.driver=true";
/// No git step of a sync starts git's own cleanup: a hook killed mid-cleanup could leave it
/// half done. The verb runs the cleanup once, at the end of its round.
const NO_AUTO_CLEANUP: [&str; 4] = ["-c", "gc.auto=0", "-c", "maintenance.auto=false"];
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
/// Remote settings that make git run a program on fetch or push; the hub remote has none.
const COMMAND_KEYS: [&str; 3] = ["receivepack", "uploadpack", "vcs"];
const LOCK_NAME: &str = "af-sync.lock";
const LOCK_STALE: Duration = Duration::from_secs(10 * 60);
pub const HOOK_PROBE: Duration = Duration::from_secs(2);
/// The hooks' network steps (fetch, push) are killed past this; local steps always finish.
pub const HOOK_BUDGET: Duration = Duration::from_secs(15);
/// A hook starts a merge only with at least this much of its budget left.
pub const MERGE_RESERVE: Duration = Duration::from_secs(3);
/// This many skipped hook syncs in a row, and session start tells the chat.
pub const SKIP_ALERT: u32 = 3;
/// The per-message pull: its hub probe and fetch together end within this.
pub const PROMPT_BUDGET: Duration = Duration::from_secs(2);
/// The per-message pull starts a merge only with this much of its budget left.
const PROMPT_RESERVE: Duration = Duration::from_millis(500);
/// The per-message list of changed paths stops here and says how many more.
pub const PROMPT_LIST_CAP: usize = 10;
/// The verb waits longer than a hook for a share that is slow to wake.
const VERB_PROBE: Duration = Duration::from_secs(10);
/// Two copies whose times differ by less than this are the same age (network shares round).
const MTIME_SLACK: Duration = Duration::from_secs(2);
const TMP_SUFFIX: &str = ".af-sync-tmp";
/// Per machine, ignored by git: the old copy of every file the copy list replaced.
pub const BACKUP_DIR: &str = "system/logs/sync-backups";
/// In the private repository's .git folder (per machine): consecutive skipped hook syncs,
/// and the hub commit whose merge conflicted.
const SKIPS_FILE: &str = "af-sync-skips";
const CONFLICT_FILE: &str = "af-sync-conflict";
/// Also there: every path a merge from the hub brought in, one per line, appended; and per
/// chat, how far into that file the per-message pull has told it, and which conflict.
const BROUGHT_FILE: &str = "af-sync-brought";
const SEEN_DIR: &str = "af-sync-seen";
/// Also there: the hub commit this machine last merged or pushed, so a prompt with nothing
/// new on the hub runs no git at all.
const PULLED_FILE: &str = "af-sync-pulled";
/// What `af sync` says on a machine not set up yet; the laptop kit's watchdog matches it.
pub const NO_HUB: &str = "no hub remote";

#[derive(Subcommand, Debug)]
pub enum SyncCmd {
    /// Create the hub's two bare repositories, point both checkouts at them, push both
    Init {
        /// A local folder or a network share; never a web or SSH address
        hub: String,
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(ctx: &Ctx, cmd: Option<SyncCmd>, dry_run: bool) -> R<i32> {
    match cmd {
        Some(SyncCmd::Init { hub, dry_run: d }) => cmd_init(ctx, &hub, d || dry_run),
        None => cmd_sync(ctx, dry_run),
    }
}

// ------------------------------------------------------------------ addresses

/// None when git would read `addr` as a folder path; otherwise what kind of address it is.
pub fn not_a_folder(addr: &str) -> Option<&'static str> {
    let t = addr.trim();
    if t.is_empty() {
        return Some("empty");
    }
    if t.contains("://") {
        return Some("a web address");
    }
    let b = t.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return if b.len() > 2 && (b[2] == b'/' || b[2] == b'\\') {
            None
        } else {
            Some("a drive-relative path (it depends on the drive's current folder; put a backslash after the colon)")
        };
    }
    if t.starts_with("\\\\") || t.starts_with("//") {
        // `\\machine@SSL\...` and `\\machine@port\...` are Windows' WebDAV form: HTTP, not a share.
        let host = t[2..].split(['/', '\\']).next().unwrap_or("");
        return host.contains('@').then_some("a WebDAV address");
    }
    // Git for Windows reads `/name` (and so `\name`) as a folder under its own install folder.
    if t.starts_with('\\') || (cfg!(windows) && t.starts_with('/')) {
        return Some(r"a path with one leading slash, which Git for Windows reads as a folder inside its own install folder (write D:\folder for a local folder or \\machine\share for a shared one)");
    }
    let head = t.split(['/', '\\']).next().unwrap_or("");
    if head.contains(':') || head.contains('@') {
        return Some("an SSH address");
    }
    None
}

/// The hub folder as given, made absolute (a share or drive path is already absolute).
fn absolute(hub: &str) -> String {
    let t = hub.trim().trim_end_matches(['/', '\\']);
    let b = t.as_bytes();
    let rooted = t.starts_with('/') || t.starts_with('\\') || (b.len() >= 2 && b[1] == b':');
    if rooted {
        t.to_string()
    } else {
        crate::pyos::abspath(t)
    }
}

/// The remote address for one of the hub's repositories: forward slashes, which git reads
/// on every platform (`//machine/share/...` for a network share).
pub fn remote_url(hub_abs: &str, repo: &str) -> String {
    format!("{}/{repo}", hub_abs.replace('\\', "/").trim_end_matches('/'))
}

/// The hub folder: the parent of the `hub` remote's path, as a filesystem path.
pub fn hub_dir_from_url(url: &str) -> PathBuf {
    let u = url.trim().replace('\\', "/");
    let u = u.trim_end_matches('/');
    let parent = match u.rfind('/') {
        Some(i) if i > 0 => &u[..i],
        _ => u,
    };
    PathBuf::from(if cfg!(windows) { parent.replace('/', "\\") } else { parent.to_string() })
}

/// The private repository's `hub` address; None without one.
pub fn private_hub_url(ctx: &Ctx) -> Option<String> {
    workspace_remotes(ctx).into_iter().find(|(n, _)| n == REMOTE).map(|(_, u)| u)
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
    /// git's first message line, for a one-line report; git's ownership refusal becomes
    /// the one command that fixes it.
    pub fn why(&self) -> String {
        if let Some(fix) = ownership_fix(&self.stderr) {
            return fix;
        }
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

/// git's "dubious ownership" refusal, as the command that lifts it: git's own suggestion
/// when it printed one, else ours.
pub fn ownership_fix(stderr: &str) -> Option<String> {
    if !stderr.contains("dubious ownership") {
        return None;
    }
    let cmd = stderr
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("git config") && l.contains("safe.directory"))
        .map(str::to_string)
        .or_else(|| {
            let at = stderr.split("repository at '").nth(1)?.split('\'').next()?;
            Some(format!("git config --global --add safe.directory '{}'", safe_directory_value(at)))
        })
        .unwrap_or_else(|| "git config --global --add safe.directory '<the path git names>'".into());
    Some(format!(
        "git refuses the hub's repository because its files belong to the other machine's account; run this once on this machine, then rerun af sync: {cmd}"
    ))
}

/// A repository path as `safe.directory` must spell it. A share path (`//machine/share/...`)
/// takes the `%(prefix)/` escape, which git strips, leaving `//machine/share/...`.
pub fn safe_directory_value(path: &str) -> String {
    let p = path.replace('\\', "/");
    if p.starts_with("//") {
        format!("%(prefix)/{p}")
    } else {
        p
    }
}

/// git in one folder, with this crate's fixed settings. Only the steps that talk to the hub
/// (`net`) carry the deadline; a local step always runs to its end, so it never leaves a
/// lock or a half-written tree behind.
pub struct Git {
    pub dir: PathBuf,
    pre: Vec<String>,
    net_deadline: Option<Instant>,
}

impl Git {
    pub fn new(dir: &Path, net_deadline: Option<Instant>) -> Git {
        let pre = ["-c", "core.quotepath=off", "-c", OURS_DRIVER].iter().chain(NO_AUTO_CLEANUP.iter()).map(|s| s.to_string()).collect();
        let mut g = Git { dir: dir.to_path_buf(), pre, net_deadline };
        // A machine with no git identity still commits its own private layer.
        let has = |g: &Git, k: &str| !g.run(&["config", "--get", k]).stdout.trim().is_empty();
        if !(has(&g, "user.name") && has(&g, "user.email")) {
            g.pre.extend(["-c", "user.name=AgentFrame", "-c", "user.email=af-sync@localhost"].iter().map(|s| s.to_string()));
        }
        g
    }

    /// Trust the hub's two repositories for this git's runs only (command scope; no config
    /// file is written): on a share their files belong to the other machine's account.
    pub fn trusting(mut self, hub: &str) -> Git {
        for repo in [PUBLIC_REPO, PRIVATE_REPO] {
            self.pre.push("-c".into());
            self.pre.push(format!("safe.directory={}", safe_directory_value(&remote_url(hub, repo))));
        }
        self
    }

    fn full(&self, args: &[&str]) -> Vec<String> {
        let mut full: Vec<String> = vec!["-C".into(), self.dir.to_string_lossy().into_owned()];
        full.extend(self.pre.iter().cloned());
        full.extend(args.iter().map(|s| s.to_string()));
        full
    }

    /// A local step: runs to its end.
    pub fn run(&self, args: &[&str]) -> Out {
        run_git_in(&self.full(args), None, self.dir.parent())
    }

    /// A step that talks to the hub: killed at the deadline.
    pub fn net(&self, args: &[&str]) -> Out {
        run_git_in(&self.full(args), self.net_deadline, self.dir.parent())
    }

    pub fn ok(&self, args: &[&str]) -> bool {
        self.run(args).ok()
    }

    pub fn text(&self, args: &[&str]) -> Option<String> {
        let o = self.run(args);
        o.ok().then(|| o.stdout.trim().to_string())
    }

    pub fn rev(&self, r: &str) -> Option<String> {
        self.text(&["rev-parse", "--verify", "-q", &format!("{r}^{{commit}}")]).filter(|s| !s.is_empty())
    }

    pub fn branch(&self) -> Option<String> {
        self.text(&["symbolic-ref", "--short", "-q", "HEAD"]).filter(|s| !s.is_empty())
    }

    /// Paths `-z` output lists.
    pub fn paths(&self, args: &[&str]) -> Vec<String> {
        self.run(args).stdout.split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect()
    }

    pub fn unmerged(&self) -> Vec<String> {
        let mut v = self.paths(&["diff", "--name-only", "-z", "--diff-filter=U"]);
        v.dedup();
        v
    }

    pub fn is_ancestor(&self, a: &str, b: &str) -> bool {
        self.ok(&["merge-base", "--is-ancestor", a, b])
    }

    /// git's own cleanup, when git judges it due, in the foreground; only the verb runs it.
    fn cleanup(&self) {
        let mut args: Vec<String> = vec!["-C".into(), self.dir.to_string_lossy().into_owned()];
        // This git's settings (the hub's trust among them), less the ones that block cleanup.
        for pair in self.pre.chunks(2) {
            if !NO_AUTO_CLEANUP.contains(&pair[pair.len() - 1].as_str()) {
                args.extend(pair.iter().cloned());
            }
        }
        args.extend(["-c", "gc.autoDetach=false", "gc", "--auto", "--quiet"].iter().map(|s| s.to_string()));
        let _ = run_git_in(&args, None, self.dir.parent());
    }
}

/// Run git without a shell, stdin closed, never prompting; past the deadline the process is
/// killed and abandoned, so no caller ever waits on a hung share.
pub fn run_git(args: &[String], deadline: Option<Instant>) -> Out {
    run_git_in(args, deadline, None)
}

/// `run_git` that never looks for a repository above `ceiling`, so a broken `.git` in
/// `workspace/` can never make git act on the public repository around it.
fn run_git_in(args: &[String], deadline: Option<Instant>, ceiling: Option<&Path>) -> Out {
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
        .env("GIT_MERGE_AUTOEDIT", "no")
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

fn machine() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| "this machine".into())
}

fn commit_message(what: &str) -> String {
    format!("af sync: {what} on {} at {}", machine(), chrono::Local::now().format("%Y-%m-%d %H:%M"))
}

/// True when the folder is a git checkout (`.git` is a folder, or a file in a linked one).
fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn git_dir(ws: &Path) -> PathBuf {
    ws.join(".git")
}

/// The hooks' check, before any git process starts (they require `.git` to be a folder).
fn merge_in_progress(ws: &Path) -> bool {
    git_dir(ws).join("MERGE_HEAD").is_file()
}

/// The verb's check, right in a linked checkout too.
fn merging(g: &Git) -> bool {
    g.rev("MERGE_HEAD").is_some()
}

/// A text file still carrying conflict markers, or a binary file, is unresolved; only an
/// edited text file is staged on the chat's behalf. A file the chat deleted is resolved as
/// a deletion (in a modify/delete conflict git leaves the edited copy in place, so a
/// missing file is the chat's choice).
fn unresolved_on_disk(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return path.exists() };
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return true;
    }
    String::from_utf8_lossy(&bytes)
        .lines()
        .any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>") || l.starts_with("|||||||"))
}

fn age(path: &Path) -> Option<Duration> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(SystemTime::now().duration_since(t).unwrap_or_default())
}

// ------------------------------------------------------------------ real addresses

/// Every address git will really use for the `hub` remote: fetch and push, after
/// `pushurl`, `insteadOf` and `pushInsteadOf`. None when the remote does not resolve.
pub fn hub_addresses(g: &Git) -> Option<Vec<String>> {
    let fetch = g.run(&["remote", "get-url", "--all", REMOTE]);
    if !fetch.ok() {
        return None;
    }
    let push = g.run(&["remote", "get-url", "--push", "--all", REMOTE]);
    let mut out: Vec<String> = Vec::new();
    for line in fetch.stdout.lines().chain(push.stdout.lines()) {
        let l = line.trim();
        if !l.is_empty() && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    Some(out)
}

/// The first address that is not a folder, with its kind.
pub fn first_non_folder(addrs: &[String]) -> Option<(String, &'static str)> {
    addrs.iter().find_map(|a| not_a_folder(a).map(|k| (a.clone(), k)))
}

/// The refusal for a hub remote that would reach beyond a folder; `dir` is how the
/// repository is named in the fix (`workspace` or `.`).
fn address_refusal(dir: &str, addr: &str, kind: &str) -> String {
    format!(
        "the hub remote of {dir} resolves to {addr}, {kind}, not a folder path, so nothing was fetched or pushed. Remove what points it there: git -C {dir} config --unset-all remote.hub.pushurl, and any url.<address>.insteadOf or pushInsteadOf rule (list them with git config --show-origin --get-regexp \"^url\\.\"). If its own address is wrong, or this machine should not sync: git -C {dir} remote remove hub (or git -C {dir} config --unset-all remote.hub.url). Then af sync init <hub-folder>"
    )
}

/// The first hub-remote setting, in any config git reads, that makes git run a program.
fn command_key(g: &Git) -> Option<String> {
    COMMAND_KEYS.iter().map(|k| format!("remote.{REMOTE}.{k}")).find(|key| g.ok(&["config", "--get-all", key]))
}

fn command_refusal(dir: &str, key: &str) -> String {
    format!(
        "the hub remote of {dir} sets {key}, which runs a command on every fetch or push, so nothing was fetched or pushed. Remove it: git -C {dir} config --unset-all {key} (git -C {dir} config --show-origin --get-all {key} names the file when it is set elsewhere), then rerun af sync"
    )
}

/// What the hub remote is, before anything talks to it.
enum Hub {
    Missing,
    /// The refusal, and a short reason for the run log.
    Refused(String, String),
    Safe,
}

fn inspect_hub(g: &Git, dir: &str) -> Hub {
    if let Some(key) = command_key(g) {
        return Hub::Refused(command_refusal(dir, &key), format!("the hub remote sets {key}, which runs a command (af doctor names the fix)"));
    }
    match hub_addresses(g) {
        None => Hub::Missing,
        Some(a) => match first_non_folder(&a) {
            Some((addr, kind)) => Hub::Refused(address_refusal(dir, &addr, kind), format!("the hub remote resolves to {kind}, not a folder (af doctor names the fix)")),
            None => Hub::Safe,
        },
    }
}

/// Ok when the hub remote exists, runs no command, and every address git will use is a
/// folder; otherwise the refusal.
fn check_addresses(g: &Git, dir: &str) -> Result<(), String> {
    match inspect_hub(g, dir) {
        Hub::Missing => Err(format!("{dir} has {NO_HUB}; run af sync init <hub-folder> first")),
        Hub::Refused(why, _) => Err(why),
        Hub::Safe => Ok(()),
    }
}

/// For `af doctor`: the private layer's hub addresses as git resolves them, else as its
/// config file spells them (`url`, `pushurl`), and the first that is not a folder.
pub fn doctor_hub_problem(ctx: &Ctx, parsed: &[String]) -> Option<(String, &'static str)> {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    let resolved = if git_dir(&ws).join("HEAD").is_file() { hub_addresses(&Git::new(&ws, None)) } else { None };
    first_non_folder(&resolved.unwrap_or_else(|| parsed.to_vec()))
}

/// For `af doctor`: a hub-remote setting that runs a command, as git reads the config,
/// else as the config file spells it.
pub fn doctor_hub_command(ctx: &Ctx, parsed: &[String]) -> Option<String> {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    if git_dir(&ws).join("HEAD").is_file() {
        command_key(&Git::new(&ws, None))
    } else {
        parsed.first().cloned()
    }
}

// ------------------------------------------------------------------ per-machine state

/// Consecutive skipped hook syncs: the count after this one.
fn record_skip(ws: &Path, why: &str) -> u32 {
    let p = git_dir(ws).join(SKIPS_FILE);
    let n = std::fs::read_to_string(&p).ok().and_then(|t| t.split('\t').next()?.trim().parse::<u32>().ok()).unwrap_or(0) + 1;
    let _ = std::fs::write(&p, format!("{n}\t{why}\n"));
    n
}

/// A sync that ran clears the skip count and the recorded conflict.
fn clear_state(ws: &Path) {
    for f in [SKIPS_FILE, CONFLICT_FILE] {
        let _ = std::fs::remove_file(git_dir(ws).join(f));
    }
}

fn recorded_conflict(ws: &Path) -> Option<String> {
    std::fs::read_to_string(git_dir(ws).join(CONFLICT_FILE)).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// The paths a merge from the hub brought in, for the per-message pull to tell each chat.
fn record_brought(ws: &Path, paths: &[String]) {
    let text: String = paths.iter().filter(|p| !p.contains('\n')).map(|p| format!("{p}\n")).collect();
    if text.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(git_dir(ws).join(BROUGHT_FILE)) {
        let _ = f.write_all(text.as_bytes());
    }
}

// ------------------------------------------------------------------ the lock

/// One af-driven sync at a time per private repository: the verb and both hooks take it.
struct SyncLock(PathBuf);

enum Lock {
    Taken(SyncLock),
    Busy,
    /// Older than `LOCK_STALE`: left by an interrupted sync. The hooks report it; the verb
    /// clears it.
    Stale,
}

impl SyncLock {
    fn path(ws: &Path) -> PathBuf {
        git_dir(ws).join(LOCK_NAME)
    }

    fn take(ws: &Path, clear_stale: bool) -> Lock {
        let p = SyncLock::path(ws);
        if let Some(a) = age(&p) {
            if a < LOCK_STALE {
                return Lock::Busy;
            }
            if !clear_stale {
                return Lock::Stale;
            }
            let _ = std::fs::remove_file(&p);
        }
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                let _ = write!(f, "{}", std::process::id());
                Lock::Taken(SyncLock(p))
            }
            Err(_) => Lock::Busy,
        }
    }

    /// Move the lock's time to now, so a long round is never mistaken for an interrupted one.
    fn touch(&self) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&self.0) {
            let _ = f.set_modified(SystemTime::now());
        }
    }
}

impl Drop for SyncLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ------------------------------------------------------------------ merge attributes

pub fn attr_block() -> String {
    let mut s = format!("{ATTR_BEGIN}\n");
    for (pattern, driver) in MERGE_RULES {
        s.push_str(&format!("{pattern} merge={driver}\n"));
    }
    s.push_str(ATTR_END);
    s.push('\n');
    s
}

/// `.gitattributes` text with the managed block current: replaced in place, else appended.
pub fn with_attr_block(text: &str) -> String {
    let block = attr_block();
    if let (Some(a), Some(b)) = (text.find(ATTR_BEGIN), text.find(ATTR_END)) {
        if a < b {
            let mut end = b + ATTR_END.len();
            if text[end..].starts_with('\n') {
                end += 1;
            }
            return format!("{}{block}{}", &text[..a], &text[end..]);
        }
    }
    if text.is_empty() {
        block
    } else if text.ends_with('\n') {
        format!("{text}\n{block}")
    } else {
        format!("{text}\n\n{block}")
    }
}

/// Write the block when it is missing or stale; true when the file changed.
fn ensure_attributes(ws: &Path) -> R<bool> {
    let path = ws.join(".gitattributes").to_string_lossy().into_owned();
    let mut tf = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
    let next = with_attr_block(&tf.text);
    if next == tf.text {
        return Ok(false);
    }
    tf.text = next;
    tf.save(&path)?;
    Ok(true)
}

// ------------------------------------------------------------------ re-render

/// Regenerate every view the merge rules keep as this machine's copy; never fails the sync.
pub fn rerender(ctx: &Ctx) {
    crate::needs::refresh(ctx);
    if let Ok(Some(reg)) = crate::tags::Registry::load(ctx) {
        let _ = crate::tags::render_view(ctx, &reg);
    }
}

// ------------------------------------------------------------------ init

pub fn cmd_init(ctx: &Ctx, hub: &str, dry: bool) -> R<i32> {
    if let Some(kind) = not_a_folder(hub) {
        die!(
            "'{hub}' is {kind}; the hub is a folder on your home network. Give a local folder (D:\\agentframe-hub) or a shared folder (\\\\<machine>\\<share>), e.g. af sync init \\\\<machine>\\agentframe-hub"
        );
    }
    let root = PathBuf::from(&ctx.root);
    if !is_repo(&root) {
        die!("{} is not a git checkout; af sync needs this repository cloned with git", ctx.root);
    }
    let hub_abs = absolute(hub);
    let hub_dir = PathBuf::from(&hub_abs);
    let (pub_url, priv_url) = (remote_url(&hub_abs, PUBLIC_REPO), remote_url(&hub_abs, PRIVATE_REPO));
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    let joining = !is_repo(&ws);
    if joining && ws.is_dir() && std::fs::read_dir(&ws).map(|mut d| d.next().is_some()).unwrap_or(false) {
        die!(
            "workspace/ holds files but is not a git repository, so af sync cannot tell them from the hub's copy. Rename workspace/ (for example to workspace-old/), rerun af sync init {hub} to clone the hub's private layer, then copy back anything missing"
        );
    }
    if dry {
        outln!("af sync init (dry run): nothing changes.");
        outln!("Hub folder: {hub_abs}");
        for (name, url) in [("public code", &pub_url), ("private layer", &priv_url)] {
            let state = if hub_dir_exists(&url_path(url)) { "already exists" } else { "would be created" };
            outln!("  {name}: {url} {state}; remote 'hub' would point at it");
        }
        if joining {
            outln!("  workspace/ has no repository: it would be cloned from the hub's private layer");
        }
        outln!("  both would then be pushed to the hub (public: master only)");
        return Ok(0);
    }
    if let Err(e) = std::fs::create_dir_all(&hub_dir) {
        die!("cannot create the hub folder {hub_abs}: {e}; check the share is connected and you can write to it");
    }
    let public = Git::new(&root, None).trusting(&hub_abs);
    let priv_branch = if joining { None } else { Some(Git::new(&ws, None).branch().unwrap_or_else(|| PUBLIC_BRANCH.into())) };
    for (url, branch) in [(&pub_url, Some(PUBLIC_BRANCH.to_string())), (&priv_url, priv_branch.clone())] {
        let bare = url_path(url);
        if !bare.join("HEAD").is_file() {
            let at = Git::new(&hub_dir, None).trusting(&hub_abs);
            let o = at.net(&["init", "--bare", "-q", &bare.to_string_lossy()]);
            if !o.ok() {
                die!("cannot create {}: {}", bare.display(), o.why());
            }
            if let Some(b) = branch {
                let _ = Git::new(&bare, None).trusting(&hub_abs).net(&["symbolic-ref", "HEAD", &format!("refs/heads/{b}")]);
            }
            outln!("Hub: created {}", url);
        }
        // A push never starts the hub's own cleanup: a stop hook would wait on a repack over
        // the share. The verb runs it at the end of its round instead.
        let hub_git = Git::new(&bare, None).trusting(&hub_abs);
        if hub_git.text(&["config", "--get", "receive.autogc"]).as_deref() != Some("false") {
            let o = hub_git.net(&["config", "receive.autogc", "false"]);
            if !o.ok() {
                outln!("Hub: could not turn off git's cleanup on push in {url}: {}; pushes may pause while the hub tidies itself", o.why());
            }
        }
    }
    set_remote(&public, &pub_url)?;
    if let Err(why) = check_addresses(&public, ".") {
        die!("{why}");
    }
    outln!("Public code: remote 'hub' points at {pub_url}");
    if joining {
        let bare = url_path(&priv_url);
        let has_commits =
            !Git::new(&bare, None).trusting(&hub_abs).net(&["for-each-ref", "--count=1", "refs/heads"]).stdout.trim().is_empty();
        if !has_commits {
            die!(
                "workspace/ has no repository and the hub's private layer is empty, so there is nothing to clone; run af sync init on the machine that holds workspace/ first"
            );
        }
        // A clone reads no repository's own config, so the address it will really use, after
        // any insteadOf rule, is resolved outside every repository too (from the hub folder,
        // which is none), and the clone runs from there.
        let outside = Git::new(&hub_dir, None).trusting(&hub_abs);
        if let Some(key) = command_key(&outside) {
            die!("{}", command_refusal("workspace", &key));
        }
        let real = outside.run(&["ls-remote", "--get-url", &priv_url]).stdout.trim().to_string();
        if let Some(kind) = not_a_folder(&real) {
            die!("{}", address_refusal("workspace", &real, kind));
        }
        let o = outside.net(&["clone", "-q", "--no-hardlinks", "-o", REMOTE, &priv_url, &ws.to_string_lossy()]);
        if !o.ok() {
            die!("cloning the private layer from the hub failed: {}", o.why());
        }
        outln!("Private layer: cloned from the hub into workspace/");
    }
    let private = Git::new(&ws, None).trusting(&hub_abs);
    set_remote(&private, &priv_url)?;
    if let Err(why) = check_addresses(&private, "workspace") {
        die!("{why}");
    }
    outln!("Private layer: remote 'hub' points at {priv_url}");
    if ensure_attributes(&ws)? {
        let o = private.run(&["add", "--", ".gitattributes"]);
        let c = private.run(&["commit", "-q", "-m", &commit_message("merge rules"), "--", ".gitattributes"]);
        if !o.ok() || !c.ok() {
            die!("committing workspace/.gitattributes failed: {}", if o.ok() { c.why() } else { o.why() });
        }
        outln!("Private layer: merge rules written to workspace/.gitattributes and committed");
    }
    let mut problems = Vec::new();
    for (g, branch, name) in [(&public, PUBLIC_BRANCH.to_string(), "Public code"), (&private, private.branch().unwrap_or_default(), "Private layer")] {
        if branch.is_empty() || g.rev(&format!("refs/heads/{branch}")).is_none() {
            outln!("{name}: no '{}' branch with commits yet; nothing pushed", if branch.is_empty() { PUBLIC_BRANCH } else { &branch });
            continue;
        }
        match push_if_fast_forward(g, &branch) {
            Ok(line) => outln!("{name}: {line}"),
            Err(why) => problems.push(format!("{name}: {why}")),
        }
    }
    for p in &problems {
        outln!("{p}");
    }
    if !problems.is_empty() {
        die!("af sync init finished with the problems above");
    }
    outln!("Done. From now on, af sync keeps this machine and the hub in step.");
    Ok(0)
}

fn url_path(url: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { url.replace('/', "\\") } else { url.to_string() })
}

fn hub_dir_exists(p: &Path) -> bool {
    p.join("HEAD").is_file()
}

fn set_remote(g: &Git, url: &str) -> R<()> {
    let o = match g.text(&["remote", "get-url", REMOTE]) {
        Some(cur) if cur == url => return Ok(()),
        Some(_) => g.run(&["remote", "set-url", REMOTE, url]),
        None => g.run(&["remote", "add", REMOTE, url]),
    };
    if !o.ok() {
        die!("setting remote 'hub' in {} failed: {}", g.dir.display(), o.why());
    }
    Ok(())
}

/// Fetch the hub, then push `branch` when the hub lacks it or this is a fast-forward.
fn push_if_fast_forward(g: &Git, branch: &str) -> Result<String, String> {
    let f = g.net(&["fetch", "-q", REMOTE]);
    if !f.ok() {
        return Err(format!("fetching from the hub failed: {}", f.why()));
    }
    let theirs = format!("refs/remotes/{REMOTE}/{branch}");
    let ours = format!("refs/heads/{branch}");
    match (g.rev(&theirs), g.rev(&ours)) {
        (Some(t), Some(o)) if t == o => return Ok("the hub already has this machine's state".into()),
        (Some(_), Some(_)) if !g.is_ancestor(&theirs, &ours) => {
            return Ok("not pushed: the hub has commits this machine lacks; af sync merges them".into())
        }
        _ => {}
    }
    let p = g.net(&["push", "-q", REMOTE, &format!("{ours}:{ours}")]);
    if p.ok() {
        Ok(format!("pushed {branch} to the hub"))
    } else {
        Err(format!("pushing to the hub failed: {}", p.why()))
    }
}

// ------------------------------------------------------------------ sync

enum Private {
    Done { brought: usize },
    Conflict(Vec<String>),
}

pub fn cmd_sync(ctx: &Ctx, dry: bool) -> R<i32> {
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    if !is_repo(&ws) {
        die!("workspace/ is not a git repository yet; on the machine that holds it, commit it (git -C workspace init), then run af sync init <hub-folder>");
    }
    let Some(url) = private_hub_url(ctx) else {
        die!("the private layer has {NO_HUB}, so there is nothing to sync with; run af sync init <hub-folder> first (a local folder or a shared folder on your home network)");
    };
    let hub = hub_dir_from_url(&url);
    let hub_s = hub.to_string_lossy().into_owned();
    let private = Git::new(&ws, None).trusting(&hub_s);
    if let Err(why) = check_addresses(&private, "workspace") {
        die!("{why}");
    }
    if git_dir(&ws).join("index.lock").exists() {
        die!(
            "workspace/.git/index.lock exists: a git program is working in workspace/, or one was interrupted and left the lock behind. If no git program is running, delete workspace/.git/index.lock, then rerun af sync"
        );
    }
    if !probe(&hub.join(PRIVATE_REPO).join("HEAD"), VERB_PROBE) {
        die!("the hub folder {} is not reachable (or has no {PRIVATE_REPO}); check the other machine is on and the shared folder is connected, then rerun af sync", hub.display());
    }
    if dry {
        return dry_run(ctx, &ws, &hub);
    }
    let stale = age(&SyncLock::path(&ws)).is_some_and(|a| a >= LOCK_STALE);
    let lock = match SyncLock::take(&ws, true) {
        Lock::Taken(l) => l,
        _ => die!("another af sync is running on this machine; wait for it, or delete workspace/.git/{LOCK_NAME} if none is"),
    };
    if stale {
        outln!("Private layer: removed a stale sync lock left by an interrupted sync.");
    }
    match sync_private(ctx, &private, &lock)? {
        Private::Conflict(paths) => {
            outln!("Private layer: the other machine changed the same lines; the merge is waiting for you.");
            for p in &paths {
                outln!("{CONFLICT_TAG} {p}");
            }
            outln!("Resolve each file as {PROCEDURE} says, then run af sync again.");
            Ok(SIGNAL_EXIT)
        }
        Private::Done { brought } => {
            clear_state(&ws);
            lock.touch();
            let mut problems = Vec::new();
            let pub_brought = sync_public(ctx, &hub_s, &mut problems);
            lock.touch();
            copy_files(ctx, &hub, false, &mut problems, &|| lock.touch());
            if brought > 0 || pub_brought {
                if let Some(line) = crate::hooks::index_refresh::refresh_after_sync(&ctx.root) {
                    outln!("{line}");
                }
            }
            private.cleanup();
            if problems.is_empty() {
                // The hub's repositories never clean up on a push (init turns it off), so here.
                for repo in [PRIVATE_REPO, PUBLIC_REPO] {
                    let bare = hub.join(repo);
                    if bare.join("HEAD").is_file() {
                        lock.touch();
                        Git::new(&bare, None).trusting(&hub_s).cleanup();
                    }
                }
            }
            for p in &problems {
                outln!("{p}");
            }
            if !problems.is_empty() {
                die!("af sync finished with the problems above");
            }
            outln!("Done.");
            Ok(0)
        }
    }
}

fn sync_private(ctx: &Ctx, g: &Git, lock: &SyncLock) -> R<Private> {
    let ws = g.dir.clone();
    let Some(branch) = g.branch() else {
        die!("workspace/ is not on a branch (detached HEAD); check out its branch (git -C workspace checkout master), then rerun af sync");
    };
    // A merge the chat resolved brought the other machine's changes, so it re-renders too.
    let finishing = merging(g);
    if finishing {
        let still: Vec<String> = g.unmerged().into_iter().filter(|p| unresolved_on_disk(&ws.join(p))).collect();
        if !still.is_empty() {
            return Ok(Private::Conflict(still));
        }
        outln!("Private layer: finishing the merge you resolved.");
    }
    ensure_attributes(&ws)?;
    let a = g.run(&["add", "-A"]);
    if !a.ok() {
        die!("staging the private layer failed: {}", a.why());
    }
    let staged = g.paths(&["diff", "--cached", "--name-only", "-z"]).len();
    if staged > 0 || merging(g) {
        let c = g.run(&["commit", "-q", "--no-verify", "-m", &commit_message("private layer")]);
        if !c.ok() {
            die!("committing the private layer failed: {}", c.why());
        }
        outln!("Private layer: saved {staged} changed file(s) from this machine.");
    } else {
        outln!("Private layer: nothing new on this machine.");
    }
    lock.touch();
    let f = g.net(&["fetch", "-q", REMOTE]);
    if !f.ok() {
        die!("fetching the private layer from the hub failed: {}", f.why());
    }
    lock.touch();
    let theirs = format!("{REMOTE}/{branch}");
    let mut brought = 0;
    if g.rev(&theirs).is_some() {
        let before = g.rev("HEAD");
        let m = g.run(&["merge", "-q", "--no-edit", "--no-verify", &theirs]);
        if !m.ok() {
            let u = g.unmerged();
            if !u.is_empty() {
                return Ok(Private::Conflict(u));
            }
            if merging(g) {
                let _ = g.run(&["merge", "--abort"]);
            }
            die!("merging the hub's private layer failed: {}; nothing was merged", m.why());
        }
        let after = g.rev("HEAD");
        if before != after {
            brought = match &before {
                Some(b) => {
                    let paths = g.paths(&["diff", "--name-only", "-z", b, "HEAD"]);
                    record_brought(&ws, &paths);
                    paths.len().max(1)
                }
                None => 1,
            };
            outln!("Private layer: brought in {brought} changed file(s) from the other machine.");
        } else if !finishing {
            outln!("Private layer: the other machine had nothing new.");
        }
    }
    if brought > 0 || finishing {
        rerender(ctx);
        let _ = g.run(&["add", "-A"]);
        if !g.paths(&["diff", "--cached", "--name-only", "-z"]).is_empty() {
            let _ = g.run(&["commit", "-q", "--no-verify", "-m", &commit_message("re-rendered views")]);
        }
        brought = brought.max(1);
    }
    lock.touch();
    let ours = format!("refs/heads/{branch}");
    let p = g.net(&["push", "-q", REMOTE, &format!("{ours}:{ours}")]);
    if !p.ok() {
        die!("sending the private layer to the hub failed: {}; rerun af sync", p.why());
    }
    outln!("Private layer: the hub has this machine's state.");
    if let Some(head) = g.rev("HEAD") {
        let _ = std::fs::write(git_dir(&ws).join(PULLED_FILE), format!("{head}\n"));
    }
    Ok(Private::Done { brought })
}

/// The public half: true when a merge brought new code. Problems are reported, never fatal.
fn sync_public(ctx: &Ctx, hub: &str, problems: &mut Vec<String>) -> bool {
    let g = Git::new(Path::new(&ctx.root), None).trusting(hub);
    match inspect_hub(&g, ".") {
        Hub::Missing => {
            outln!("Public code: {NO_HUB} on this checkout; af sync init <hub-folder> adds it. Skipped.");
            return false;
        }
        Hub::Refused(why, _) => {
            problems.push(format!("Public code: {why}"));
            return false;
        }
        Hub::Safe => {}
    }
    let f = g.net(&["fetch", "-q", REMOTE]);
    if !f.ok() {
        problems.push(format!("Public code: fetching from the hub failed: {}", f.why()));
        return false;
    }
    let theirs = format!("refs/remotes/{REMOTE}/{PUBLIC_BRANCH}");
    let ours = format!("refs/heads/{PUBLIC_BRANCH}");
    if g.rev(&ours).is_none() {
        outln!("Public code: this checkout has no {PUBLIC_BRANCH} branch; skipped.");
        return false;
    }
    let branch = g.branch().unwrap_or_else(|| "a detached HEAD".into());
    let dirty = !g.run(&["status", "--porcelain", "--untracked-files=no"]).stdout.trim().is_empty();
    let mut brought = false;
    let mut skipped = None;
    if branch == PUBLIC_BRANCH && !dirty {
        if g.rev(&theirs).is_some() {
            let before = g.rev("HEAD");
            let m = g.run(&["merge", "-q", "--no-edit", "--no-verify", &theirs]);
            if !m.ok() {
                let u = g.unmerged();
                if merging(&g) {
                    let _ = g.run(&["merge", "--abort"]);
                }
                problems.push(if u.is_empty() {
                    format!("Public code: merging the hub's master failed: {}; nothing was merged", m.why())
                } else {
                    format!(
                        "Public code: the hub's master conflicts with this checkout in {}; the merge was undone. A builder chat merges hub/master by hand.",
                        u.join(", ")
                    )
                });
                return false;
            }
            brought = before != g.rev("HEAD");
            if brought {
                outln!("Public code: brought in new code from the other machine.");
            }
        }
    } else {
        skipped = Some(if branch != PUBLIC_BRANCH {
            format!("this checkout is on {branch}, not {PUBLIC_BRANCH}")
        } else {
            "tracked files have uncommitted changes".to_string()
        });
    }
    let (t, o) = (g.rev(&theirs), g.rev(&ours));
    let line = if t.is_some() && t == o {
        "up to date with the hub".to_string()
    } else if t.is_none() || g.is_ancestor(&theirs, &ours) {
        let p = g.net(&["push", "-q", REMOTE, &format!("{ours}:{ours}")]);
        if !p.ok() {
            problems.push(format!("Public code: pushing master to the hub failed: {}", p.why()));
            return brought;
        }
        "sent master to the hub".to_string()
    } else {
        "master not sent: the hub has code this checkout lacks".to_string()
    };
    match skipped {
        Some(why) => outln!("Public code: merge skipped because {why}; {line}."),
        None => outln!("Public code: {line}."),
    }
    brought
}

fn dry_run(ctx: &Ctx, ws: &Path, hub: &Path) -> R<i32> {
    outln!("af sync (dry run): nothing changes.");
    let hub_s = hub.to_string_lossy().into_owned();
    let private = Git::new(ws, None).trusting(&hub_s);
    let changed = private.run(&["status", "--porcelain"]).stdout.lines().count();
    outln!("Private layer: {changed} changed file(s) would be saved.");
    let branch = private.branch().unwrap_or_else(|| PUBLIC_BRANCH.into());
    outln!("Private layer: {}", compare_with_hub(&private, &branch));
    let public = Git::new(Path::new(&ctx.root), None).trusting(&hub_s);
    match inspect_hub(&public, ".") {
        Hub::Missing => outln!("Public code: {NO_HUB}; would be skipped."),
        Hub::Refused(why, _) => outln!("Public code: {why}"),
        Hub::Safe => {
            let b = public.branch().unwrap_or_default();
            let dirty = !public.run(&["status", "--porcelain", "--untracked-files=no"]).stdout.trim().is_empty();
            if b != PUBLIC_BRANCH || dirty {
                outln!("Public code: the merge would be skipped ({}); master is sent only as a fast-forward.", if dirty { "uncommitted changes" } else { "not on master" });
            }
            outln!("Public code: {}", compare_with_hub(&public, PUBLIC_BRANCH));
        }
    }
    let mut problems = Vec::new();
    copy_files(ctx, hub, true, &mut problems, &|| {});
    for p in &problems {
        outln!("{p}");
    }
    Ok(0)
}

/// Read-only: what the hub's branch is against this machine's, without fetching.
fn compare_with_hub(g: &Git, branch: &str) -> String {
    let o = g.net(&["ls-remote", REMOTE, &format!("refs/heads/{branch}")]);
    let theirs = if o.ok() { o.stdout } else { String::new() };
    let theirs = theirs.split_whitespace().next().unwrap_or("");
    let ours = g.rev(&format!("refs/heads/{branch}")).unwrap_or_default();
    if theirs.is_empty() {
        "the hub has no copy yet; this machine's would be sent".into()
    } else if theirs == ours {
        "already the same as the hub".into()
    } else if g.rev(theirs).is_some() && g.is_ancestor(theirs, &ours) {
        "this machine has newer commits; they would be sent".into()
    } else {
        "the hub has commits this machine lacks; they would be fetched and merged".into()
    }
}

// ------------------------------------------------------------------ the copy list

/// `*` within one path segment.
fn wild(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if pi < p.len() && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// A slash path against a pattern of segments where `**` spans any number of them.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    fn go(p: &[&str], s: &[&str]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(&"**") => (0..=s.len()).any(|i| go(&p[1..], &s[i..])),
            Some(seg) => !s.is_empty() && wild(seg, s[0]) && go(&p[1..], &s[1..]),
        }
    }
    let p: Vec<&str> = pattern.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    go(&p, &s)
}

/// The leading folders a pattern names literally.
fn literal_base(pattern: &str) -> String {
    let segs: Vec<&str> = pattern.split('/').collect();
    let n = segs.iter().position(|s| s.contains('*')).unwrap_or(segs.len().saturating_sub(1));
    segs[..n].join("/")
}

fn walk_matches(base_dir: &Path, base_rel: &str, pattern: &str, out: &mut BTreeSet<String>) {
    if !pattern.contains('*') {
        if base_dir.join(pattern.rsplit('/').next().unwrap_or(pattern)).is_file() {
            out.insert(pattern.to_string());
        }
        return;
    }
    if !base_dir.is_dir() {
        return;
    }
    let depth = if pattern.contains("**") { usize::MAX } else { pattern.split('/').count() - base_rel.split('/').filter(|s| !s.is_empty()).count() };
    let walker = walkdir::WalkDir::new(base_dir).max_depth(depth).follow_links(false).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !(e.file_type().is_dir() && (n == ".git" || n == "node_modules"))
    });
    for e in walker.flatten() {
        if !e.file_type().is_file() || e.file_name() == ".gitkeep" || e.file_name().to_string_lossy().ends_with(TMP_SUFFIX) {
            continue;
        }
        let Ok(r) = e.path().strip_prefix(base_dir) else { continue };
        let r = r.to_string_lossy().replace('\\', "/");
        let rel = if base_rel.is_empty() { r } else { format!("{base_rel}/{r}") };
        if glob_match(pattern, &rel) {
            out.insert(rel);
        }
    }
}

fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|s| !s.is_empty()).fold(base.to_path_buf(), |p, s| p.join(s))
}

/// What one file in the copy list needs.
#[derive(Debug, PartialEq, Eq)]
enum Move {
    /// Copy; true is to the hub.
    Copy(bool),
    /// Same content, different times: give the older copy the newer one's time, copy nothing.
    Align(bool),
    Stay,
}

/// Which way one file goes. Times decide, read from the directory listing; contents are
/// read only when the times differ and the sizes match, to skip copying an identical file.
/// Two copies of the same age stay as they are, whatever they hold.
fn direction(local: &Path, remote: &Path) -> Move {
    match (std::fs::metadata(local), std::fs::metadata(remote)) {
        (Ok(_), Err(_)) => Move::Copy(true),
        (Err(_), Ok(_)) => Move::Copy(false),
        (Ok(a), Ok(b)) => {
            let (Ok(ta), Ok(tb)) = (a.modified(), b.modified()) else { return Move::Stay };
            let to_hub = if ta > tb + MTIME_SLACK {
                true
            } else if tb > ta + MTIME_SLACK {
                false
            } else {
                return Move::Stay;
            };
            if a.len() == b.len() && std::fs::read(local).ok() == std::fs::read(remote).ok() {
                Move::Align(to_hub)
            } else {
                Move::Copy(to_hub)
            }
        }
        _ => Move::Stay,
    }
}

/// The old copy of a file about to be replaced, saved under this machine's backup folder
/// (never the hub); its path relative to the checkout.
fn back_up(root: &Path, stamp: &str, rel: &str, old: &Path) -> std::io::Result<String> {
    let backup_rel = format!("{BACKUP_DIR}/{stamp}/{rel}");
    let dest = join_rel(root, &backup_rel);
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(old, &dest)?;
    Ok(backup_rel)
}

/// Copy through a temp name, then carry the source's time so the next run sees them equal.
fn copy_one(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = to.with_file_name(format!(".{name}{TMP_SUFFIX}"));
    std::fs::copy(from, &tmp)?;
    if let Ok(t) = std::fs::metadata(from).and_then(|m| m.modified()) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&tmp) {
            let _ = f.set_modified(t);
        }
    }
    // rename replaces an existing file on every platform.
    std::fs::rename(&tmp, to)
}

/// The copy list, newer wins, nothing deleted: a replaced file's old copy goes to this
/// machine's backup folder first. With `dry` only prints what would move. `copied` runs after
/// each file copied (the verb refreshes its lock there).
pub fn copy_files(ctx: &Ctx, hub: &Path, dry: bool, problems: &mut Vec<String>, copied: &dyn Fn()) {
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    let root = PathBuf::from(&ctx.root);
    let mirror = hub.join(FILES_DIR);
    let mut rels = BTreeSet::new();
    for pattern in SYNC_COPY_LIST {
        let base = literal_base(pattern);
        walk_matches(&join_rel(&root, &base), &base, pattern, &mut rels);
        walk_matches(&join_rel(&mirror, &base), &base, pattern, &mut rels);
    }
    // A file a repository tracks travels by git, never by copy.
    let tracked: BTreeSet<String> = Git::new(&root, None).paths(&["ls-files", "-z"]).into_iter().collect();
    let ws = root.join("workspace");
    let ws_tracked: BTreeSet<String> = if is_repo(&ws) {
        Git::new(&ws, None).paths(&["ls-files", "-z"]).into_iter().map(|p| format!("workspace/{p}")).collect()
    } else {
        BTreeSet::new()
    };
    let (mut up, mut down) = (0, 0);
    for rel in rels.iter().filter(|r| !tracked.contains(*r) && !ws_tracked.contains(*r)) {
        let (local, remote) = (join_rel(&root, rel), join_rel(&mirror, rel));
        let to_hub = match direction(&local, &remote) {
            Move::Stay => continue,
            Move::Align(to_hub) => {
                if !dry {
                    let (from, to) = if to_hub { (&local, &remote) } else { (&remote, &local) };
                    if let (Ok(t), Ok(f)) = (std::fs::metadata(from).and_then(|m| m.modified()), std::fs::OpenOptions::new().write(true).open(to)) {
                        let _ = f.set_modified(t);
                    }
                }
                continue;
            }
            Move::Copy(to_hub) => to_hub,
        };
        let (from, to) = if to_hub { (&local, &remote) } else { (&remote, &local) };
        let side = if to_hub { "on the hub" } else { "here" };
        if dry {
            let backup = if to.exists() { format!("; the old copy {side} would be saved under {BACKUP_DIR}/") } else { String::new() };
            outln!("Files: would copy {rel} {}{backup}", if to_hub { "to the hub (newer here)" } else { "here (newer on the hub)" });
            continue;
        }
        if to.exists() {
            match back_up(&root, &stamp, rel, to) {
                Ok(b) => outln!("Files: replacing {rel} {side} with the newer copy; the old one is saved at {b}"),
                Err(e) => {
                    problems.push(format!("Files: {rel} not replaced: saving its old copy failed: {e}"));
                    continue;
                }
            }
        }
        match copy_one(from, to) {
            Ok(()) if to_hub => up += 1,
            Ok(()) => down += 1,
            Err(e) => problems.push(format!("Files: copying {rel} failed: {e}")),
        }
        copied();
    }
    if !dry {
        if up + down == 0 {
            outln!("Files: the copy list is the same on both sides.");
        } else {
            outln!("Files: {up} copied to the hub, {down} copied here (newer file wins; nothing deleted).");
        }
    }
}

// ------------------------------------------------------------------ hooks

/// The line session start shows for a conflict the hooks cannot merge.
fn conflict_line() -> String {
    format!(
        "AgentFrame sync: this machine's private layer conflicts with the other machine's, so nothing merges until it is resolved. Run af sync and resolve the files it names ({PROCEDURE})."
    )
}

/// The hook half: commit, fetch and merge the private layer; with `push`, send it to the
/// hub. Inert without a `hub` remote. Never fails: an unreachable hub, a git lock, a merge
/// in progress, or any git failure skips with one run-log line. Only fetch and push are
/// killed at the budget; the local steps always finish. Returns the line for the chat:
/// session start says so when a merge waits, a lock is stale, a conflict is recorded, or
/// `SKIP_ALERT` hook syncs in a row were skipped for a reason that needs him; the stop hook
/// never prints. No git step here starts git's own cleanup.
pub fn hook_half(ctx: &Ctx, push: bool) -> Option<String> {
    hook_half_with(ctx, push, HOOK_BUDGET)
}

/// `hook_half` with its network budget given (tests drive the merge reserve through it).
pub fn hook_half_with(ctx: &Ctx, push: bool, budget: Duration) -> Option<String> {
    half(ctx, if push { Half::Stop } else { Half::Start }, budget)
}

/// The hook a private half runs in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Half {
    Start,
    Prompt,
    Stop,
}

/// The per-message pull: the session-start half within `PROMPT_BUDGET` (probe and fetch
/// together), then what this chat has not been told: the paths any merge from the hub brought
/// in since its last prompt, capped at `PROMPT_LIST_CAP`, and the conflict line once per
/// recorded conflict. None when there is nothing to tell. `session` keys the chat's marker; a
/// chat with no marker yet read the files and heard any conflict at its session start.
pub fn prompt_half(ctx: &Ctx, session: Option<&str>) -> Option<String> {
    prompt_half_with(ctx, session, PROMPT_BUDGET)
}

/// A chat's marker file; None for a missing session id or one that is not a plain name.
fn seen_marker(ws: &Path, session: Option<&str>) -> Option<PathBuf> {
    session
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|s| git_dir(ws).join(SEEN_DIR).join(s))
}

fn brought_len(ws: &Path) -> u64 {
    std::fs::metadata(git_dir(ws).join(BROUGHT_FILE)).map(|m| m.len()).unwrap_or(0)
}

fn write_marker(ws: &Path, marker: &Path, at: u64, told: Option<&str>) {
    let _ = std::fs::create_dir_all(git_dir(ws).join(SEEN_DIR));
    let _ = std::fs::write(marker, format!("{at}\t{}\n", told.unwrap_or("")));
}

/// After session start's pull: a new chat has read every file and heard any conflict, so its
/// first prompt reports only what arrives after this. A chat that already has a marker (a
/// resumed one) keeps it and hears what came in while it was away.
pub fn mark_session_start(ctx: &Ctx, session: Option<&str>) {
    if private_hub_url(ctx).is_none() {
        return;
    }
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    if let Some(m) = seen_marker(&ws, session).filter(|m| !m.exists() && git_dir(&ws).is_dir()) {
        write_marker(&ws, &m, brought_len(&ws), recorded_conflict(&ws).as_deref());
    }
}

/// The complete lines of the brought-paths log between `from` and `end` (bytes past `end`,
/// appended meanwhile, wait for the next prompt; so does a line still being written), and
/// the offset just past the last complete line.
pub fn complete_lines(bytes: &[u8], from: u64, end: u64) -> (u64, Vec<String>) {
    let end = (end as usize).min(bytes.len());
    let from = (from as usize).min(end);
    let span = &bytes[from..end];
    let Some(last) = span.iter().rposition(|b| *b == b'\n') else { return (from as u64, Vec::new()) };
    let lines = String::from_utf8_lossy(&span[..=last]).lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
    ((from + last + 1) as u64, lines)
}

/// A path as the chat's context may carry it: control, separator and formatting characters
/// (a line or paragraph separator, a direction override) become `?`.
pub fn printable(path: &str) -> String {
    path.chars()
        .map(|c| {
            let hidden = c.is_control()
                || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}');
            if hidden {
                '?'
            } else {
                c
            }
        })
        .collect()
}

/// `prompt_half` with its budget given.
pub fn prompt_half_with(ctx: &Ctx, session: Option<&str>, budget: Duration) -> Option<String> {
    private_hub_url(ctx)?;
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    let marker = seen_marker(&ws, session);
    let saved = marker.as_ref().and_then(|m| std::fs::read_to_string(m).ok()).and_then(|t| {
        let (at, told) = t.trim_end_matches(['\r', '\n']).split_once('\t')?;
        Some((at.parse::<u64>().ok()?, Some(told.to_string()).filter(|t| !t.is_empty())))
    });
    let (from, told) = saved.unwrap_or_else(|| (brought_len(&ws), recorded_conflict(&ws)));
    let _ = half(ctx, Half::Prompt, budget);
    if !git_dir(&ws).is_dir() {
        return None;
    }
    let end = brought_len(&ws);
    let mut paths = BTreeSet::new();
    let mut seen = end;
    if end > from {
        if let Ok(bytes) = std::fs::read(git_dir(&ws).join(BROUGHT_FILE)) {
            let (at, lines) = complete_lines(&bytes, from, end);
            seen = at;
            paths.extend(lines);
        }
    }
    let conflict = recorded_conflict(&ws);
    if let Some(m) = &marker {
        write_marker(&ws, m, seen, conflict.as_deref());
    }
    let mut out = Vec::new();
    if conflict.is_some() && conflict != told {
        out.push(conflict_line());
    }
    if !paths.is_empty() {
        out.push("AgentFrame sync: the other machine changed these files since this chat last looked; reread any you answer from:".to_string());
        out.extend(paths.iter().take(PROMPT_LIST_CAP).map(|p| format!("- workspace/{}", printable(p))));
        if paths.len() > PROMPT_LIST_CAP {
            out.push(format!("and {} more", paths.len() - PROMPT_LIST_CAP));
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

fn half(ctx: &Ctx, kind: Half, budget: Duration) -> Option<String> {
    let started = Instant::now();
    private_hub_url(ctx)?;
    let ws = PathBuf::from(vpath(ctx, "workspace"));
    let push = kind == Half::Stop;
    let verb = match kind {
        Half::Start => "hook session-start sync",
        Half::Prompt => "hook user-prompt-submit sync",
        Half::Stop => "hook stop sync",
    };
    // Every skip is logged. One that clears by itself (the hub is off or slow, another sync
    // is running) is not counted; one that needs him is, unless a prompt met it (session
    // start and stop count their own), and the line reaches the chat only at session start.
    let pass = |why: &str| -> Option<String> {
        crate::runlog::log_note(ctx, verb, &format!("skipped: {why}"));
        None
    };
    let skip = |why: &str, line: Option<String>| -> Option<String> {
        crate::runlog::log_note(ctx, verb, &format!("skipped: {why}"));
        if kind == Half::Prompt {
            return None;
        }
        let n = if git_dir(&ws).is_dir() { record_skip(&ws, why) } else { 0 };
        if push {
            return None;
        }
        line.or_else(|| {
            (n >= SKIP_ALERT).then(|| {
                format!("AgentFrame sync: the last {n} automatic syncs were skipped (latest: {why}). Run af sync; it says what is wrong.")
            })
        })
    };
    if !git_dir(&ws).is_dir() {
        return skip("workspace/.git is not a folder", None);
    }
    let index_lock = git_dir(&ws).join("index.lock");
    if let Some(a) = age(&index_lock) {
        return if a >= LOCK_STALE {
            skip(
                "a stale git lock (workspace/.git/index.lock)",
                Some("AgentFrame sync: a git lock left by an interrupted git step (workspace/.git/index.lock) is stopping the sync. Run af sync; it says what to do.".into()),
            )
        } else {
            skip("a git lock is held (workspace/.git/index.lock)", None)
        };
    }
    if merge_in_progress(&ws) {
        return skip(
            "a merge is in progress; af sync finishes it",
            Some(format!(
                "AgentFrame sync: a merge with the other machine is waiting to be resolved, so nothing syncs until it is. Run af sync; it names the files ({PROCEDURE})."
            )),
        );
    }
    if kind == Half::Prompt {
        // Read on the hub without git: no hub answers is the probe failing, and a hub tip
        // this machine already merged or conflicted on is nothing new.
        let url = private_hub_url(ctx)?;
        if let (None, Some(branch)) = (not_a_folder(&url), head_branch(&ws)) {
            let limit = (started + budget).saturating_duration_since(Instant::now());
            match hub_tip(&hub_dir_from_url(&url).join(PRIVATE_REPO), &branch, limit) {
                None => return pass("the hub is unreachable"),
                Some(Some(tip)) => {
                    let pulled = std::fs::read_to_string(git_dir(&ws).join(PULLED_FILE)).ok().map(|t| t.trim().to_string());
                    if pulled.as_deref() == Some(tip.as_str()) || recorded_conflict(&ws).as_deref() == Some(tip.as_str()) {
                        return None;
                    }
                }
                Some(None) => {}
            }
        }
    }
    let _lock = match SyncLock::take(&ws, false) {
        Lock::Taken(l) => l,
        Lock::Busy => return pass("another af sync is running"),
        Lock::Stale => {
            return skip(
                "a stale sync lock (workspace/.git/af-sync.lock)",
                Some(format!(
                    "AgentFrame sync: a sync lock left by an interrupted sync (workspace/.git/{LOCK_NAME}) is stopping the sync. Run af sync; it clears it."
                )),
            )
        }
    };
    // A prompt's budget runs from its start and covers the probe; the others' cover the
    // network steps after a probe of their own.
    let (cap, reserve) = match kind {
        Half::Prompt => (started + budget, PROMPT_RESERVE),
        _ => (Instant::now() + budget, MERGE_RESERVE),
    };
    let hub = hub_dir_from_url(&private_hub_url(ctx)?);
    let g = Git::new(&ws, Some(cap)).trusting(&hub.to_string_lossy());
    match inspect_hub(&g, "workspace") {
        Hub::Missing => return skip("the hub remote does not resolve", None),
        Hub::Refused(_, why) => return skip(&why, None),
        Hub::Safe => {}
    }
    // A recorded conflict he has since merged by hand is over.
    if recorded_conflict(&ws).is_some_and(|rec| g.is_ancestor(&rec, "HEAD")) {
        let _ = std::fs::remove_file(git_dir(&ws).join(CONFLICT_FILE));
    }
    let probe_limit = if kind == Half::Prompt { cap.saturating_duration_since(Instant::now()) } else { HOOK_PROBE };
    if !probe(&hub.join(PRIVATE_REPO).join("HEAD"), probe_limit) {
        return pass("the hub is unreachable");
    }
    let Some(branch) = g.branch() else {
        return skip("workspace/ is not on a branch", None);
    };
    let a = g.run(&["add", "-A"]);
    if !a.ok() {
        return skip(&format!("staging failed: {}", a.why()), None);
    }
    if !g.paths(&["diff", "--cached", "--name-only", "-z"]).is_empty() {
        let c = g.run(&["commit", "-q", "--no-verify", "-m", &commit_message("private layer")]);
        if !c.ok() {
            return skip(&format!("commit failed: {}", c.why()), None);
        }
    }
    // A network step cut off at the deadline is the hub being slow; a git error is not.
    let net_skip = |what: &str, o: &Out| -> Option<String> {
        if o.code.is_none() {
            pass(&format!("{what} timed out"))
        } else {
            skip(&format!("{what} failed: {}", o.why()), None)
        }
    };
    let f = g.net(&["fetch", "-q", REMOTE]);
    if !f.ok() {
        return net_skip("fetch", &f);
    }
    let theirs = format!("{REMOTE}/{branch}");
    let mut line = None;
    let mut pulled = None;
    if let Some(their_head) = g.rev(&theirs) {
        if recorded_conflict(&ws).as_deref() == Some(their_head.as_str()) {
            return skip("the hub still has the commit that conflicted; af sync resolves it", Some(conflict_line()));
        }
        if cap.saturating_duration_since(Instant::now()) < reserve {
            return pass("too little time left to start the merge");
        }
        let before = g.rev("HEAD");
        let m = g.run(&["merge", "-q", "--no-edit", "--no-verify", &theirs]);
        if !m.ok() {
            let conflict = !g.unmerged().is_empty();
            if merge_in_progress(&ws) {
                let _ = g.run(&["merge", "--abort"]);
            }
            if conflict {
                // Remembered per machine, so the next hooks do not merge and undo it again.
                let _ = std::fs::write(git_dir(&ws).join(CONFLICT_FILE), format!("{their_head}\n"));
                return skip("the merge conflicted and was undone; af sync resolves it", Some(conflict_line()));
            }
            return skip(&format!("merge failed: {}", m.why()), None);
        }
        if before != g.rev("HEAD") {
            if let Some(b) = &before {
                record_brought(&ws, &g.paths(&["diff", "--name-only", "-z", b, "HEAD"]));
            }
            rerender(ctx);
            let _ = g.run(&["add", "-A"]);
            if !g.paths(&["diff", "--cached", "--name-only", "-z"]).is_empty() {
                let _ = g.run(&["commit", "-q", "--no-verify", "-m", &commit_message("re-rendered views")]);
            }
            let _ = crate::hooks::index_refresh::refresh_after_sync(&ctx.root);
            line = Some("AgentFrame sync: brought in the other machine's latest private changes.".to_string());
        }
        pulled = Some(their_head);
    }
    if push {
        let ours = format!("refs/heads/{branch}");
        let p = g.net(&["push", "-q", REMOTE, &format!("{ours}:{ours}")]);
        if !p.ok() {
            return net_skip("push", &p);
        }
        pulled = g.rev("HEAD");
    }
    if let Some(head) = pulled {
        let _ = std::fs::write(git_dir(&ws).join(PULLED_FILE), format!("{head}\n"));
    }
    clear_state(&ws);
    if push {
        None
    } else {
        line
    }
}

/// The branch `workspace/.git/HEAD` names, read without git.
fn head_branch(ws: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir(ws).join("HEAD")).ok()?;
    head.trim().strip_prefix("ref: refs/heads/").map(str::to_string)
}

/// The tip of `branch` in a hub repository, read from its ref files within `limit`: None when
/// the hub does not answer (no repository there, or a hung share, abandoned), Some(None) when
/// no ref file names the branch.
fn hub_tip(repo: &Path, branch: &str, limit: Duration) -> Option<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (repo, name) = (repo.to_path_buf(), format!("refs/heads/{branch}"));
    std::thread::spawn(move || {
        if !repo.join("HEAD").is_file() {
            let _ = tx.send(None);
            return;
        }
        let loose = std::fs::read_to_string(join_rel(&repo, &name)).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        let tip = loose.or_else(|| {
            let packed = std::fs::read_to_string(repo.join("packed-refs")).ok()?;
            packed.lines().find_map(|l| l.split_once(' ').filter(|(_, r)| r.trim() == name).map(|(sha, _)| sha.to_string()))
        });
        let _ = tx.send(Some(tip));
    });
    rx.recv_timeout(limit).unwrap_or(None)
}

/// Whether a file on the hub answers within `limit`; a hung share is abandoned, not awaited.
pub fn probe(path: &Path, limit: Duration) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    let p = path.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(p.is_file());
    });
    rx.recv_timeout(limit).unwrap_or(false)
}
