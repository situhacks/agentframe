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
    // The doctor renders life/needs.md into the repo; commit it so a clean repo stays clean.
    afcore::life::doctor_checks(&fx.ctx()).unwrap();
    git(&fx, "", &["init", "-q", "-b", "master", "workspace"]);
    git(&fx, "workspace", &["add", "-A"]);
    git(&fx, "workspace", &["commit", "-q", "-m", "private start"]);
    fx
}

fn dirty(fx: &Fx) {
    fx.write("workspace/life/open.md", "# Open loops\n\nProse is fine here.\n");
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

// ------------------------------------------------------------------ what blocks a save

#[test]
fn any_operation_in_progress_skips_the_save() {
    let cases: [(&str, bool, &str); 5] = [
        ("rebase-merge", true, "a rebase is in progress"),
        ("rebase-apply", true, "a rebase is in progress"),
        ("CHERRY_PICK_HEAD", false, "a cherry-pick is in progress"),
        ("REVERT_HEAD", false, "a revert is in progress"),
        ("BISECT_LOG", false, "a bisect is in progress"),
    ];
    for (marker, is_dir, words) in cases {
        let fx = fixture();
        dirty(&fx);
        let rel = format!("workspace/.git/{marker}");
        if is_dir {
            fx.mkdir(&rel);
        } else {
            fx.write(&rel, "abc\n");
        }
        let out = fx.ok(&["save"]);
        assert!(out.starts_with("Skipped: ") && out.contains(words), "{marker}: {out}");
        assert_eq!(commits(&fx, "workspace"), 1, "{marker}");
    }
}

#[test]
fn git_cleanup_runs_for_the_manual_save_and_never_for_the_hook() {
    assert!(afcore::save::cleanup_runs(None), "af save cleans up");
    assert!(!afcore::save::cleanup_runs(Some(afcore::save::HOOK_BUDGET)), "a save with a deadline never does");
}

// ------------------------------------------------------------------ af doctor

fn issues(fx: &Fx) -> Vec<String> {
    afcore::life::doctor_checks(&fx.ctx()).unwrap().0
}

/// The fixture's last commit, backdated by `hours`.
fn backdate(fx: &Fx, hours: i64) {
    let when = (chrono::Local::now() - chrono::Duration::hours(hours)).format("%Y-%m-%dT%H:%M:%S").to_string();
    let o = std::process::Command::new("git")
        .arg("-C")
        .arg(fx.path("workspace"))
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-q", "--amend", "--no-edit", "--allow-empty"])
        .env("GIT_AUTHOR_DATE", &when)
        .env("GIT_COMMITTER_DATE", &when)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn a_stale_git_lock_is_an_issue_naming_the_file_and_the_fix() {
    for (rel, name) in [
        ("workspace/.git/index.lock", "index.lock"),
        ("workspace/.git/HEAD.lock", "HEAD.lock"),
        ("workspace/.git/refs/heads/master.lock", "refs/heads/master.lock"),
        ("workspace/.git/gc.pid", "gc.pid"),
    ] {
        let fx = fixture();
        fx.write(rel, "");
        assert!(issues(&fx).is_empty(), "{name}: a fresh lock is a git at work");
        set_mtime(&fx, rel, -11 * 60);
        let i = issues(&fx);
        assert_eq!(i.len(), 1, "{name}: {i:?}");
        assert!(i[0].starts_with(&format!("workspace/.git/{name} is 11 minutes old")), "{}", i[0]);
        assert!(i[0].contains("confirm no git is running, then delete that file"), "{}", i[0]);
    }
}

#[test]
fn a_lock_a_few_minutes_old_is_not_flagged() {
    let fx = fixture();
    fx.write("workspace/.git/index.lock", "");
    set_mtime(&fx, "workspace/.git/index.lock", -5 * 60);
    assert!(issues(&fx).is_empty(), "{:?}", issues(&fx));
}

#[test]
fn uncommitted_changes_over_an_old_commit_mean_the_layer_is_not_saved() {
    let fx = fixture();
    backdate(&fx, 30);
    dirty(&fx);
    let i = issues(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("uncommitted changes") && i[0].contains("not being saved") && i[0].contains("af save"), "{}", i[0]);
    assert!(i[0].contains("30 hours old"), "{}", i[0]);

    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("not being saved"), "{out}");

    fx.ok(&["save"]);
    assert!(issues(&fx).is_empty(), "a save clears it");
}

#[test]
fn uncommitted_changes_over_a_recent_commit_are_not_flagged() {
    let fx = fixture();
    backdate(&fx, 2);
    dirty(&fx);
    assert!(issues(&fx).is_empty(), "{:?}", issues(&fx));
}

#[test]
fn a_clean_repo_with_an_old_commit_is_not_flagged() {
    let fx = fixture();
    backdate(&fx, 100);
    assert!(issues(&fx).is_empty(), "{:?}", issues(&fx));
}

#[test]
fn a_workspace_git_cannot_read_reports_nothing() {
    isolate_git();
    let fx = Fx::new();
    fx.write("workspace/.git/config", "[core]\n\tbare = false\n");
    fx.write("workspace/life/open.md", "# Open loops\n");
    assert!(issues(&fx).is_empty(), "no false issue when git cannot answer");
}
