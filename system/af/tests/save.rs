//! `af save`: the private layer (workspace/, its own git repo) is committed locally. A dirty
//! repo gets one commit, a clean one says so, a git lock or a merge in progress skips, a
//! workspace that is not a repo is left alone, nothing ever creates or contacts a remote, the
//! stop hook saves only when the chat really stops, and a dry run commits nothing.

mod common;
mod life_fx;
use common::Fx;
use life_fx::*;
use std::sync::Once;

static ENV: Once = Once::new();

/// git in every test (and in the af processes they start) sees no user or system config, so
/// the machine's own settings never leak in and the no-identity fallback is exercised.
fn isolate_git() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("af-save-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("gitconfig");
        std::fs::write(&cfg, "").unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", &cfg);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    });
}

fn git(fx: &Fx, rel: &str, args: &[&str]) -> String {
    let o = std::process::Command::new("git")
        .arg("-C")
        .arg(fx.path(rel))
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid"])
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?} in {rel:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn commits(fx: &Fx, rel: &str) -> usize {
    git(fx, rel, &["rev-list", "--count", "HEAD"]).parse().unwrap()
}

/// A root whose workspace/ is a git repo with one commit and two life files.
fn fixture() -> Fx {
    isolate_git();
    let fx = Fx::new();
    fx.write("workspace/life/open.md", "# Open loops\n");
    fx.write("workspace/life/life.md", "# Life\n");
    git(&fx, "", &["init", "-q", "-b", "master", "workspace"]);
    git(&fx, "workspace", &["add", "-A"]);
    git(&fx, "workspace", &["commit", "-q", "-m", "private start"]);
    fx
}

fn dirty(fx: &Fx) {
    fx.write("workspace/life/open.md", "# Open loops\n\n- changed\n");
    fx.write("workspace/life/knowledge/new.md", "# New\n");
}

#[test]
fn a_dirty_workspace_repo_is_committed() {
    let fx = fixture();
    dirty(&fx);
    let out = fx.ok(&["save"]);
    assert!(out.contains("Saved 2 files"), "{out}");
    assert_eq!(commits(&fx, "workspace"), 2);
    assert_eq!(git(&fx, "workspace", &["status", "--porcelain"]), "", "everything is committed");
    let msg = git(&fx, "workspace", &["log", "-1", "--format=%s"]);
    assert!(msg.starts_with("af save: private layer on "), "{msg}");
    let stamp = msg.rsplit(" at ").next().unwrap();
    assert!(chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M").is_ok(), "{msg}");
}

#[test]
fn a_clean_repo_has_nothing_to_save() {
    let fx = fixture();
    let out = fx.ok(&["save"]);
    assert!(out.contains("Nothing to save."), "{out}");
    assert_eq!(commits(&fx, "workspace"), 1);
}

#[test]
fn a_git_lock_skips_the_save() {
    let fx = fixture();
    dirty(&fx);
    fx.write("workspace/.git/index.lock", "");
    let out = fx.ok(&["save"]);
    assert!(out.starts_with("Skipped: ") && out.contains("index.lock"), "{out}");
    assert_eq!(commits(&fx, "workspace"), 1);
    assert!(fx.isfile("workspace/.git/index.lock"), "a lock that is not ours stays");
}

#[test]
fn a_merge_in_progress_skips_the_save() {
    let fx = fixture();
    dirty(&fx);
    let head = git(&fx, "workspace", &["rev-parse", "HEAD"]);
    fx.write("workspace/.git/MERGE_HEAD", &format!("{head}\n"));
    let out = fx.ok(&["save"]);
    assert!(out.starts_with("Skipped: ") && out.contains("merge"), "{out}");
    assert_eq!(commits(&fx, "workspace"), 1);
}

#[test]
fn a_workspace_that_is_not_a_repo_is_left_alone() {
    isolate_git();
    let fx = Fx::new();
    fx.write("workspace/life/open.md", "# Open loops\n");
    // The public repository around it must never be committed in its place.
    fx.write("AGENTS.md", "# root\n");
    git(&fx, "", &["init", "-q", "-b", "master", "."]);
    git(&fx, "", &["add", "AGENTS.md"]);
    git(&fx, "", &["commit", "-q", "-m", "public start"]);
    fx.write("AGENTS.md", "# root, edited\n");

    let out = fx.ok(&["save"]);
    assert!(out.contains("not a git repo"), "{out}");
    assert!(!fx.exists("workspace/.git"), "no repo is created");
    assert_eq!(commits(&fx, ""), 1, "the public repo is untouched");
    assert_eq!(git(&fx, "", &["status", "--porcelain", "--untracked-files=no"]), "M AGENTS.md");

    fx.rmtree("workspace");
    assert!(fx.ok(&["save"]).contains("not a git repo"), "no workspace folder at all");
}

#[test]
fn a_save_never_creates_or_contacts_a_remote() {
    let fx = fixture();
    dirty(&fx);
    fx.ok(&["save"]);
    assert_eq!(git(&fx, "workspace", &["remote"]), "");
    assert!(!fx.read("workspace/.git/config").contains("[remote"), "no remote section");

    // A remote someone added is left as it is, and the save neither fails on it nor uses it.
    let nowhere = fx.p("nowhere.git");
    git(&fx, "workspace", &["remote", "add", "origin", &nowhere]);
    dirty(&fx);
    fx.write("workspace/life/life.md", "# Life\n\n- again\n");
    let out = fx.ok(&["save"]);
    assert!(out.contains("Saved "), "{out}");
    assert_eq!(commits(&fx, "workspace"), 3);
    assert_eq!(git(&fx, "workspace", &["remote"]), "origin");
    assert!(!fx.exists("nowhere.git"), "nothing was pushed");
    assert_eq!(git(&fx, "workspace", &["branch", "-r"]), "", "nothing was fetched");
}

#[test]
fn a_dry_run_counts_and_commits_nothing() {
    let fx = fixture();
    let out = fx.ok(&["save", "--dry-run"]);
    assert!(out.contains("Nothing to save."), "{out}");

    dirty(&fx);
    let out = fx.ok(&["save", "--dry-run"]);
    assert!(out.contains("Would commit 2 files."), "{out}");
    assert_eq!(commits(&fx, "workspace"), 1);
    assert_eq!(git(&fx, "workspace", &["diff", "--cached", "--name-only"]), "", "nothing is staged");
    assert!(git(&fx, "workspace", &["status", "--porcelain"]).contains("life/knowledge"), "the changes stay uncommitted");
}

// ------------------------------------------------------------------ the stop hook

fn stop(fx: &Fx, payload: &str) -> (i32, String) {
    let o = fx.af().args(["hook", "stop"]).write_stdin(payload.to_string()).output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn the_stop_hook_saves_when_the_chat_really_stops() {
    let fx = fixture();
    dirty(&fx);
    let (code, out) = stop(&fx, r#"{"session_id":"s-new"}"#);
    assert_eq!((code, out.as_str()), (0, ""), "the hook prints nothing");
    assert_eq!(commits(&fx, "workspace"), 2);
    assert_eq!(git(&fx, "workspace", &["status", "--porcelain"]), "");
    assert!(fx.run_log_rows().is_empty(), "a save is no run-log row");
}

#[test]
fn the_stop_hook_does_not_save_while_it_blocks_for_a_handoff() {
    let fx = fixture();
    write_registry(&fx, &[row("project/acme", "operator", "s-acme", &ts(-300), "live")]);
    dirty(&fx);
    let (code, out) = stop(&fx, r#"{"session_id":"s-acme","stop_hook_active":false}"#);
    assert_eq!(code, 0);
    assert!(out.contains(r#""decision": "block""#) || out.contains(r#""decision":"block""#), "{out}");
    assert_eq!(commits(&fx, "workspace"), 1, "the chat is not stopping yet");

    let (_, out) = stop(&fx, r#"{"session_id":"s-acme","stop_hook_active":true}"#);
    assert_eq!(out, "");
    assert_eq!(commits(&fx, "workspace"), 2, "it saves once the chat stops");
}

#[test]
fn the_stop_hook_logs_a_skip_and_stays_quiet() {
    let fx = fixture();
    dirty(&fx);
    fx.write("workspace/.git/index.lock", "");
    let (code, out) = stop(&fx, r#"{"session_id":"s-new"}"#);
    assert_eq!((code, out.as_str()), (0, ""));
    assert_eq!(commits(&fx, "workspace"), 1);
    let rows = fx.run_log_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["verb"], "hook stop save");
    assert!(rows[0]["note"].as_str().unwrap().starts_with("skipped: a git lock is held"), "{rows:?}");
    assert_eq!(rows[0]["exit"], 0, "a skip is not a refusal");
}

#[test]
fn the_stop_hook_is_silent_without_a_private_repo() {
    isolate_git();
    let fx = Fx::new();
    fx.mkdir("workspace");
    let (code, out) = stop(&fx, r#"{"session_id":"s-new"}"#);
    assert_eq!((code, out.as_str()), (0, ""));
    assert!(fx.run_log_rows().is_empty(), "nothing to save is not a skip");
}
