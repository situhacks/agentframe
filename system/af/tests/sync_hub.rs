//! `af sync`: two checkouts and a hub folder, all in temp directories. Concurrent edits
//! converge, a same-line edit stops with SYNC_CONFLICT and exit 3, init refuses web and SSH
//! addresses, the copy list moves newer files both ways and deletes nothing, and the hook
//! halves push, pull, abort a conflict, and skip an unreachable hub.

use afcore::core::Ctx;
use afcore::sync::{self, glob_match, not_a_folder, with_attr_block, ATTR_BEGIN, ATTR_END};
use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant, SystemTime};

static ENV: Once = Once::new();

/// git in every test (and in the af processes they start) sees no user or system config,
/// so the machine's own settings never leak in and the no-identity fallback is exercised.
fn isolate_git() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("af-sync-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("gitconfig");
        // Git for Windows ships core.autocrlf=true in its system config; the suite runs that way
        // too, so line-ending conversion on both machines is covered. AF_SYNC_TEST_LF=1 runs without it.
        let lf = std::env::var("AF_SYNC_TEST_LF").is_ok_and(|v| v == "1");
        std::fs::write(&cfg, if lf { "" } else { "[core]\n\tautocrlf = true\n" }).unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", &cfg);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    });
}

const OPEN: &str = "# Open loops\n\n## Waiting on the operator\n\n\
- [ ] 2026-11-01 · first loop · nudge 2099-12-20 · from: main\n\n\
- [ ] 2026-11-01 · second loop · nudge 2099-12-21 · from: main\n\n\
## Waiting on others\n\n\
- [ ] 2026-11-01 · third loop · nudge 2099-12-22 · from: main\n";

const INDEX: &str = "# Sources\n\n| ID | File | Date | Description |\n|---|---|---|---|\n\
| src-2026-11-01-01 | orders/order-2026-11-01-01-start.md | 2026-11-01 | first order |\n";

const PAGE: &str = "---\ntitle: Topic\n---\n# Topic\n\n## Facts\n\n- fact one\n- fact two\n\n## Notes\n\n- note one\n\n## Timeline\n\n- 2026-11-01 · started [main]\n";

struct World {
    dir: tempfile::TempDir,
}

impl World {
    fn new() -> World {
        isolate_git();
        World { dir: tempfile::tempdir().unwrap() }
    }

    fn p(&self, rel: &str) -> PathBuf {
        rel.split('/').filter(|s| !s.is_empty()).fold(self.dir.path().to_path_buf(), |p, s| p.join(s))
    }

    fn hub(&self) -> String {
        self.p("hub").to_string_lossy().into_owned()
    }

    fn write(&self, rel: &str, text: &str) {
        let p = self.p(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// The text with line ends normalized: under autocrlf a checkout writes CRLF where af
    /// wrote LF, and only the committed content (equal HEADs, a clean status) must match.
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.p(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")).replace("\r\n", "\n")
    }

    fn git(&self, rel: &str, args: &[&str]) -> String {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(self.p(rel))
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid"])
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?} in {rel}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    fn af(&self, root: &str, args: &[&str]) -> (i32, String, String) {
        let o = Command::cargo_bin("af").unwrap().env("AF_ROOT", self.p(root)).args(args).output().unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }

    fn ctx(&self, root: &str) -> Ctx {
        Ctx::new(&self.p(root).to_string_lossy())
    }

    /// Checkout A: a public repository on master and a private one with three life files.
    fn checkout_a(&self) {
        self.write("a/AGENTS.md", "# AgentFrame\n");
        self.write("a/system/README.md", "system\n");
        self.write("a/library/assets/logos/.gitkeep", "");
        self.write(
            "a/.gitignore",
            "workspace/\nsystem/logs/\nsystem/*backlog*.md\n.env\nlibrary/assets/logos/*\n!library/assets/logos/.gitkeep\nlibrary/assets/media/*\n",
        );
        self.git("", &["init", "-q", "-b", "master", "a"]);
        self.git("a", &["add", "-A"]);
        self.git("a", &["commit", "-q", "-m", "public start"]);
        self.write("a/workspace/life/open.md", OPEN);
        self.write("a/workspace/life/sources/INDEX.md", INDEX);
        self.write("a/workspace/life/knowledge/topic.md", PAGE);
        // As the real private layer does: .env files travel by the copy list, never by git.
        self.write("a/workspace/.gitignore", "**/.env\n");
        self.git("a", &["init", "-q", "-b", "master", "workspace"]);
        self.git("a/workspace", &["add", "-A"]);
        self.git("a/workspace", &["commit", "-q", "-m", "private start"]);
    }

    /// A on the hub, then B: the public code cloned from A, the private layer from the hub.
    fn two_machines(&self) {
        self.checkout_a();
        let (code, out, err) = self.af("a", &["sync", "init", &self.hub()]);
        assert_eq!(code, 0, "init on A: {out}{err}");
        self.git("", &["clone", "-q", &self.p("a").to_string_lossy(), "b"]);
        let (code, out, err) = self.af("b", &["sync", "init", &self.hub()]);
        assert_eq!(code, 0, "init on B: {out}{err}");
        assert!(out.contains("cloned from the hub"), "{out}");
    }

    fn sync(&self, root: &str) -> (i32, String) {
        let (code, out, err) = self.af(root, &["sync"]);
        (code, format!("{out}{err}"))
    }

    fn sync_ok(&self, root: &str) -> String {
        let (code, out) = self.sync(root);
        assert_eq!(code, 0, "af sync on {root}: {out}");
        out
    }
}

fn edit(w: &World, rel: &str, from: &str, to: &str) {
    let text = w.read(rel);
    assert!(text.contains(from), "{rel} lacks {from:?}");
    w.write(rel, &text.replacen(from, to, 1));
}

fn append(w: &World, rel: &str, line: &str) {
    let mut text = w.read(rel);
    text.push_str(line);
    text.push('\n');
    w.write(rel, &text);
}

fn set_age(path: &Path, secs_ago: u64) {
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(secs_ago)).unwrap();
}

#[test]
fn concurrent_edits_converge_after_a_then_b_then_a() {
    let w = World::new();
    w.two_machines();

    edit(&w, "a/workspace/life/open.md", "first loop", "first loop, edited on A");
    append(&w, "a/workspace/life/sources/INDEX.md", "| src-2026-11-08-01 | note-2026-11-08-a.md | 2026-11-08 | filed on A |");
    edit(&w, "a/workspace/life/knowledge/topic.md", "- fact one", "- fact one, corrected on A");

    edit(&w, "b/workspace/life/open.md", "third loop", "third loop, edited on B");
    append(&w, "b/workspace/life/sources/INDEX.md", "| src-2026-11-08-02 | note-2026-11-08-b.md | 2026-11-08 | filed on B |");
    append(&w, "b/workspace/life/knowledge/topic.md", "- 2026-11-08 · learned on B [main]");

    // Both machines append to the month log (union) and hold a different stale view
    // (keep-ours, then re-rendered): neither may conflict.
    w.write("a/workspace/life/log/2026-11.md", "- 2026-11-08 09:00 · event: on A\n");
    w.write("b/workspace/life/log/2026-11.md", "- 2026-11-08 09:05 · event: on B\n");
    w.write("a/workspace/life/needs.md", "A's stale view\n");
    w.write("b/workspace/life/needs.md", "B's stale view\n");

    let out = w.sync_ok("a");
    assert!(out.contains("Private layer: saved 5 changed file(s)"), "{out}");
    let out = w.sync_ok("b");
    assert!(out.contains("brought in"), "{out}");
    w.sync_ok("a");

    let log = w.read("a/workspace/life/log/2026-11.md");
    assert!(log.contains("on A") && log.contains("on B") && !log.contains("<<<<<<<"), "{log}");
    for rel in ["life/open.md", "life/sources/INDEX.md", "life/knowledge/topic.md", "life/needs.md", "life/log/2026-11.md", ".gitattributes"] {
        assert_eq!(w.read(&format!("a/workspace/{rel}")), w.read(&format!("b/workspace/{rel}")), "{rel} differs");
    }
    let open = w.read("a/workspace/life/open.md");
    assert!(open.contains("first loop, edited on A") && open.contains("third loop, edited on B") && open.contains("second loop"));
    let index = w.read("a/workspace/life/sources/INDEX.md");
    assert!(index.contains("filed on A") && index.contains("filed on B") && index.contains("first order"), "{index}");
    assert!(!index.contains("<<<<<<<"), "union merge leaves no markers");
    let page = w.read("a/workspace/life/knowledge/topic.md");
    assert!(page.contains("fact one, corrected on A") && page.contains("learned on B"), "{page}");
    let needs = w.read("a/workspace/life/needs.md");
    assert!(needs.contains("# What needs you") && !needs.contains("stale view"), "the needs list is re-rendered after the merge: {needs}");

    assert_eq!(w.git("a/workspace", &["rev-parse", "HEAD"]), w.git("b/workspace", &["rev-parse", "HEAD"]));
    assert_eq!(w.git("a/workspace", &["status", "--porcelain"]), "");
    assert_eq!(w.git("b/workspace", &["status", "--porcelain"]), "");
    // origin is never touched; the public repository is never committed.
    assert_eq!(w.git("b", &["remote", "get-url", "origin"]), w.p("a").to_string_lossy());
    assert_eq!(w.git("a", &["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn a_same_line_edit_stops_with_sync_conflict_and_the_signal_exit() {
    let w = World::new();
    w.two_machines();
    edit(&w, "a/workspace/life/open.md", "second loop", "second loop, A's wording");
    edit(&w, "b/workspace/life/open.md", "second loop", "second loop, B's wording");
    w.sync_ok("a");

    let (code, out) = w.sync("b");
    assert_eq!(code, 3, "{out}");
    assert!(out.lines().any(|l| l == "SYNC_CONFLICT life/open.md"), "{out}");
    assert!(out.contains("library/process/sync.md"), "{out}");
    assert!(w.p("b/workspace/.git/MERGE_HEAD").is_file(), "the merge stays in progress for the chat");

    let (code, out) = w.sync("b");
    assert_eq!(code, 3, "an unresolved file still signals: {out}");

    // The chat resolves the file by editing it, then reruns.
    let resolved = OPEN.replace("second loop", "second loop, both wordings merged");
    w.write("b/workspace/life/open.md", &resolved);
    let out = w.sync_ok("b");
    assert!(out.contains("finishing the merge"), "{out}");
    w.sync_ok("a");
    assert_eq!(w.read("a/workspace/life/open.md"), resolved);
    assert_eq!(w.git("a/workspace", &["rev-parse", "HEAD"]), w.git("b/workspace", &["rev-parse", "HEAD"]));

    // The run log does not count the signal as a refusal.
    let log = w.read("b/system/logs/af-runs.jsonl");
    assert!(log.contains("\"verb\": \"sync\"") && log.contains("\"exit\": 3"), "{log}");
    let notes = afcore::runlog::button_notes(&w.ctx("b"));
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn init_refuses_web_and_ssh_addresses() {
    let w = World::new();
    w.checkout_a();
    for addr in [
        "https://github.com/someone/agentframe.git",
        "ssh://laptop/srv/agentframe",
        "file:///D:/agentframe-hub",
        "git@github.com:someone/agentframe.git",
        "someone@laptop",
        "laptop:agentframe-hub",
    ] {
        let (code, _, err) = w.af("a", &["sync", "init", addr]);
        assert_eq!(code, 1, "{addr} must be refused");
        assert!(err.contains("the hub is a folder") && err.contains("af sync init"), "the error names the fix: {err}");
    }
    assert!(!w.p("hub").exists());
    assert_eq!(w.git("a/workspace", &["remote"]), "", "no remote was added");

    for ok in [r"D:\agentframe-hub", "D:/agentframe-hub", r"\\laptop\agentframe-hub", "//laptop/agentframe-hub", "agentframe-hub"] {
        assert_eq!(not_a_folder(ok), None, "{ok} is a folder path");
    }
    if !cfg!(windows) {
        assert_eq!(not_a_folder("/srv/hub"), None, "an absolute path off Windows");
    }
}

#[test]
fn sync_without_a_hub_names_init() {
    let w = World::new();
    w.checkout_a();
    let (code, out) = w.sync("a");
    assert_eq!(code, 1);
    assert!(out.contains("no hub remote") && out.contains("af sync init"), "{out}");
}

#[test]
fn init_refuses_a_workspace_with_files_but_no_repository() {
    let w = World::new();
    w.checkout_a();
    assert_eq!(w.af("a", &["sync", "init", &w.hub()]).0, 0);
    w.git("", &["clone", "-q", &w.p("a").to_string_lossy(), "b"]);
    w.write("b/workspace/life/stray.md", "a file made before joining\n");
    let (code, _, err) = w.af("b", &["sync", "init", &w.hub()]);
    assert_eq!(code, 1);
    assert!(err.contains("holds files but is not a git repository") && err.contains("Rename workspace/"), "{err}");
    assert_eq!(w.read("b/workspace/life/stray.md"), "a file made before joining\n", "nothing is touched");
}

#[test]
fn the_copy_list_moves_newer_files_both_ways_and_deletes_nothing() {
    let w = World::new();
    w.two_machines();
    // Only here: goes to the hub.
    w.write("a/system/backlog.md", "## BB-1 — a row\n");
    w.write("a/workspace/projects/p/.env", "TOKEN=here\n");
    // Only on the hub: comes here.
    w.write("hub/files/library/assets/logos/brand.svg", "<svg/>\n");
    w.write("hub/files/system/backlog-completed.md", "## BB-0 — done\n");
    // On both: the newer copy wins in each direction.
    w.write("a/.env", "KEY=newer-here\n");
    w.write("hub/files/.env", "KEY=older-on-hub\n");
    set_age(&w.p("hub/files/.env"), 3600);
    w.write("a/library/assets/media/clip.txt", "older here\n");
    w.write("hub/files/library/assets/media/clip.txt", "newer on hub\n");
    set_age(&w.p("a/library/assets/media/clip.txt"), 3600);
    w.write("a/workspace/life/pending.md", "not saved yet\n");
    let head = w.git("a/workspace", &["rev-parse", "HEAD"]);
    let (code, out, _) = w.af("a", &["sync", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Files: would copy system/backlog.md to the hub"), "{out}");
    assert!(out.contains("Private layer: 1 changed file(s) would be saved"), "{out}");
    assert!(!w.p("hub/files/system/backlog.md").exists(), "a dry run changes nothing");
    assert_eq!(w.git("a/workspace", &["rev-parse", "HEAD"]), head, "a dry run commits nothing");
    assert!(w.git("a/workspace", &["status", "--porcelain"]).contains("pending.md"));

    let out = w.sync_ok("a");
    assert!(out.contains("3 copied to the hub, 3 copied here"), "{out}");
    assert_eq!(w.read("hub/files/system/backlog.md"), "## BB-1 — a row\n");
    assert_eq!(w.read("hub/files/workspace/projects/p/.env"), "TOKEN=here\n");
    assert_eq!(w.read("hub/files/.env"), "KEY=newer-here\n");
    assert_eq!(w.read("a/library/assets/logos/brand.svg"), "<svg/>\n");
    assert_eq!(w.read("a/system/backlog-completed.md"), "## BB-0 — done\n");
    assert_eq!(w.read("a/library/assets/media/clip.txt"), "newer on hub\n");
    assert!(!w.p("hub/files/library/assets/logos/.gitkeep").exists(), "a tracked .gitkeep never copies");

    let out = w.sync_ok("a");
    assert!(out.contains("the copy list is the same on both sides"), "a second run moves nothing: {out}");

    w.sync_ok("b");
    for rel in ["system/backlog.md", "system/backlog-completed.md", ".env", "workspace/projects/p/.env", "library/assets/logos/brand.svg", "library/assets/media/clip.txt"] {
        assert_eq!(w.read(&format!("b/{rel}")), w.read(&format!("a/{rel}")), "{rel} reached B");
    }
    // B's file only on B stays, and nothing on the hub disappears.
    w.write("b/library/assets/logos/b-only.svg", "<svg id='b'/>\n");
    std::fs::remove_file(w.p("a/system/backlog-completed.md")).unwrap();
    w.sync_ok("a");
    assert!(w.p("hub/files/system/backlog-completed.md").is_file(), "a file gone here is never deleted on the hub");
    assert!(w.p("a/system/backlog-completed.md").is_file(), "it comes back from the hub");
    w.sync_ok("b");
    assert!(w.p("b/library/assets/logos/b-only.svg").is_file());
    assert!(w.p("hub/files/library/assets/logos/b-only.svg").is_file());
}

#[test]
fn the_public_repository_is_never_committed_and_merges_only_on_a_clean_master() {
    let w = World::new();
    w.two_machines();
    w.write("a/library/new-rule.md", "a rule\n");
    w.git("a", &["add", "-A"]);
    w.git("a", &["commit", "-q", "-m", "new rule"]);
    let out = w.sync_ok("a");
    assert!(out.contains("Public code: sent master to the hub"), "{out}");

    // B has a tracked edit in progress: no merge, no push of a non-fast-forward.
    w.write("b/AGENTS.md", "# AgentFrame, edited\n");
    let out = w.sync_ok("b");
    assert!(out.contains("merge skipped because tracked files have uncommitted changes"), "{out}");
    assert!(!w.p("b/library/new-rule.md").exists());
    assert_eq!(w.read("b/AGENTS.md"), "# AgentFrame, edited\n", "the edit is left alone");

    w.git("b", &["checkout", "-q", "--", "AGENTS.md"]);
    let out = w.sync_ok("b");
    assert!(out.contains("brought in new code"), "{out}");
    assert!(w.p("b/library/new-rule.md").is_file());
}

#[test]
fn the_merge_rules_block_is_managed_in_place() {
    let block = with_attr_block("");
    assert!(block.starts_with(ATTR_BEGIN) && block.trim_end().ends_with(ATTR_END));
    assert!(block.contains("life/sources/INDEX.md merge=union") && block.contains("life/needs.md merge=ours"));
    assert!(!block.contains("life/open.md"), "open.md is edited in place and never union-merged");
    let mine = format!("*.md text\n\n{block}# mine\n");
    assert_eq!(with_attr_block(&mine), mine, "a current block is left alone");
    let stale = format!("*.md text\n\n{ATTR_BEGIN}\nold rule\n{ATTR_END}\n# mine\n");
    assert_eq!(with_attr_block(&stale), mine);

    assert!(glob_match("system/*backlog*.md", "system/backlog-completed.md"));
    assert!(!glob_match("system/*backlog*.md", "system/sub/backlog.md"));
    assert!(glob_match("workspace/**/.env", "workspace/.env") && glob_match("workspace/**/.env", "workspace/a/b/.env"));
    assert!(glob_match("library/assets/logos/**", "library/assets/logos/client/x.svg"));
}

// ------------------------------------------------------------------ hooks

fn run_log(w: &World, root: &str) -> String {
    std::fs::read_to_string(w.p(&format!("{root}/system/logs/af-runs.jsonl"))).unwrap_or_default()
}

#[test]
fn the_hooks_are_inert_without_a_hub() {
    let w = World::new();
    w.checkout_a();
    w.write("a/workspace/life/new.md", "uncommitted\n");
    assert_eq!(sync::hook_half(&w.ctx("a"), false), None);
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    assert_eq!(run_log(&w, "a"), "", "nothing logged");
    assert!(w.git("a/workspace", &["status", "--porcelain"]).contains("new.md"), "nothing committed");
}

#[test]
fn an_unreachable_hub_or_a_git_lock_skips_quickly_with_one_log_line() {
    let w = World::new();
    w.checkout_a();
    let gone = w.p("nowhere/workspace.git").to_string_lossy().replace('\\', "/");
    w.git("a/workspace", &["remote", "add", "hub", &gone]);
    let t = Instant::now();
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    let log = run_log(&w, "a");
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(log.contains("hook stop sync") && log.contains("skipped: the hub is unreachable") && log.contains("\"exit\": 0"), "{log}");

    w.write("a/workspace/.git/index.lock", "");
    assert_eq!(sync::hook_half(&w.ctx("a"), false), None);
    assert!(run_log(&w, "a").contains("skipped: a git lock is held"));
    assert!(afcore::runlog::button_notes(&w.ctx("a")).is_empty(), "a skip is not a refusal");
}

#[test]
fn a_git_command_past_its_deadline_is_killed_not_awaited() {
    isolate_git();
    let args: Vec<String> = ["-c", "alias.hang=!sleep 30", "hang"].iter().map(|s| s.to_string()).collect();
    let t = Instant::now();
    let o = sync::run_git(&args, Some(Instant::now() + Duration::from_secs(1)));
    assert_eq!(o.code, None, "a timed-out run has no exit code");
    assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
}

/// A hub that hangs instead of failing: a share on an address nothing answers. The whole
/// hook process must still end within the probe limit plus a margin. Ignored by default
/// because it depends on the network stack: `cargo test --test sync_hub -- --ignored`.
#[test]
#[ignore]
fn a_hanging_hub_never_holds_the_hook_past_its_limit() {
    let w = World::new();
    w.checkout_a();
    w.git("a/workspace", &["remote", "add", "hub", "//10.255.255.1/agentframe-hub/workspace.git"]);
    for event in ["stop", "session-start"] {
        let t = Instant::now();
        let o = Command::cargo_bin("af").unwrap().env("AF_ROOT", w.p("a")).args(["hook", event]).write_stdin("{}").output().unwrap();
        let took = t.elapsed();
        assert_eq!(o.status.code(), Some(0));
        assert!(took < Duration::from_secs(5), "af hook {event} took {took:?}");
        eprintln!("af hook {event}: {took:?}");
    }
    assert_eq!(run_log(&w, "a").matches("skipped: the hub is unreachable").count(), 2);
}

/// The hub as a network share: the temp folder reached through the loopback admin share.
/// Ignored by default (it needs that share): `cargo test --test sync_hub -- --ignored`.
#[test]
#[ignore]
#[cfg(windows)]
fn a_network_share_hub_carries_both_repositories_and_the_copy_list() {
    let w = World::new();
    let local = w.p("hub").to_string_lossy().into_owned();
    let unc = format!(r"\\localhost\{}${}", &local[..1], &local[2..]);
    std::fs::create_dir_all(&local).unwrap();
    if !Path::new(&unc).is_dir() {
        eprintln!("skipped: {unc} is not reachable on this machine");
        return;
    }
    w.checkout_a();
    let (code, out, err) = w.af("a", &["sync", "init", &unc]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(w.git("a/workspace", &["remote", "get-url", "hub"]).starts_with("//localhost/"));
    w.git("", &["clone", "-q", &w.p("a").to_string_lossy(), "b"]);
    assert_eq!(w.af("b", &["sync", "init", &unc]).0, 0);
    edit(&w, "a/workspace/life/open.md", "first loop", "first loop over the share");
    w.write("a/system/backlog.md", "## BB-1 — over the share\n");
    w.sync_ok("a");
    w.sync_ok("b");
    assert!(w.read("b/workspace/life/open.md").contains("first loop over the share"));
    assert_eq!(w.read("b/system/backlog.md"), "## BB-1 — over the share\n");
    assert!(w.p("hub/files/system/backlog.md").is_file());
}

#[test]
fn stop_pushes_and_session_start_pulls() {
    let w = World::new();
    w.two_machines();
    edit(&w, "b/workspace/life/open.md", "third loop", "third loop, from B's chat");
    // The real hook process: no session id, so the stop handler is quiet and the push half runs.
    let o = Command::cargo_bin("af").unwrap().env("AF_ROOT", w.p("b")).args(["hook", "stop"]).write_stdin("{}").output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "", "the stop hook prints nothing and never blocks");
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), w.git("b/workspace", &["rev-parse", "HEAD"]));

    let line = sync::hook_half(&w.ctx("a"), false);
    assert!(line.as_deref().unwrap_or("").contains("brought in"), "{line:?}");
    assert!(w.read("a/workspace/life/open.md").contains("third loop, from B's chat"));

    let o = Command::cargo_bin("af").unwrap().env("AF_ROOT", w.p("a")).args(["hook", "session-start"]).write_stdin("{}").output().unwrap();
    assert_eq!(o.status.code(), Some(0), "session start with nothing new");
}

#[test]
fn a_conflict_at_session_start_is_undone_and_names_af_sync() {
    let w = World::new();
    w.two_machines();
    edit(&w, "a/workspace/life/open.md", "second loop", "second loop, A");
    edit(&w, "b/workspace/life/open.md", "second loop", "second loop, B");
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);

    let line = sync::hook_half(&w.ctx("b"), false).expect("a line for the chat");
    assert!(line.contains("Run af sync") && line.contains("library/process/sync.md"), "{line}");
    assert!(!w.p("b/workspace/.git/MERGE_HEAD").exists(), "the merge was undone");
    assert!(w.read("b/workspace/life/open.md").contains("second loop, B"), "B's own edit is kept, committed");
    assert_eq!(w.git("b/workspace", &["status", "--porcelain"]), "");
    assert!(run_log(&w, "b").contains("the merge conflicted and was undone"));

    // The stop half meets the same hub commit: it does not merge again (no working-tree
    // rewrite), prints nothing, and pushes nothing.
    let hub_before = w.git("hub/workspace.git", &["rev-parse", "master"]);
    let open = w.p("b/workspace/life/open.md");
    set_age(&open, 3600);
    let stamp = std::fs::metadata(&open).unwrap().modified().unwrap();
    assert_eq!(sync::hook_half(&w.ctx("b"), true), None);
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), hub_before);
    assert_eq!(std::fs::metadata(&open).unwrap().modified().unwrap(), stamp, "the recorded conflict is not merged and undone again");
    assert!(run_log(&w, "b").contains("still has the commit that conflicted"), "{}", run_log(&w, "b"));
    let again = sync::hook_half(&w.ctx("b"), false).expect("session start still names the conflict");
    assert!(again.contains("Run af sync"), "{again}");

    let (code, out) = w.sync("b");
    assert_eq!(code, 3);
    assert!(out.contains("SYNC_CONFLICT life/open.md"), "{out}");
}

// ------------------------------------------------------------------ review fixes

/// The old copy a sync replaced, from this machine's backup folder.
fn backup_of(w: &World, root: &str, rel: &str) -> Option<String> {
    let base = w.p(&format!("{root}/system/logs/sync-backups"));
    let stamps = std::fs::read_dir(&base).ok()?;
    for s in stamps.flatten() {
        let p = rel.split('/').fold(s.path(), |p, seg| p.join(seg));
        if let Ok(t) = std::fs::read_to_string(&p) {
            return Some(t.replace("\r\n", "\n"));
        }
    }
    None
}

#[test]
fn a_copy_that_replaces_a_different_file_backs_the_old_one_up_on_this_machine() {
    let w = World::new();
    w.two_machines();
    // The hub's copy is newer: this machine's rows would vanish without a backup.
    w.write("a/system/backlog.md", "## BB-1 — row written here\n");
    w.write("hub/files/system/backlog.md", "## BB-2 — row written on the other machine\n");
    set_age(&w.p("a/system/backlog.md"), 3600);
    // This machine's copy is newer: the hub's old copy is backed up here, never on the hub.
    w.write("a/.env", "KEY=new\n");
    w.write("hub/files/.env", "KEY=old-on-hub\n");
    set_age(&w.p("hub/files/.env"), 3600);

    let out = w.sync_ok("a");
    assert_eq!(w.read("a/system/backlog.md"), "## BB-2 — row written on the other machine\n");
    assert_eq!(backup_of(&w, "a", "system/backlog.md").as_deref(), Some("## BB-1 — row written here\n"));
    assert_eq!(w.read("hub/files/.env"), "KEY=new\n");
    assert_eq!(backup_of(&w, "a", ".env").as_deref(), Some("KEY=old-on-hub\n"));
    assert!(out.contains("system/backlog.md") && out.contains("system/logs/sync-backups/"), "one line names the file and the backup: {out}");
    assert!(!w.p("hub/files/system/logs").exists(), "nothing is backed up into the hub");
    assert!(w.git("a", &["check-ignore", "-q", "system/logs/sync-backups/x"]).is_empty(), "git ignores the backups");
}

#[test]
fn the_copy_list_decides_by_size_and_time_and_leaves_an_undecidable_file_alone() {
    let w = World::new();
    w.two_machines();
    // Same size, same time, different content: nothing can say which is newer.
    w.write("a/system/backlog.md", "AAAA\n");
    w.write("hub/files/system/backlog.md", "BBBB\n");
    let t = SystemTime::now() - Duration::from_secs(600);
    for p in [w.p("a/system/backlog.md"), w.p("hub/files/system/backlog.md")] {
        std::fs::OpenOptions::new().write(true).open(p).unwrap().set_modified(t).unwrap();
    }
    // Identical content, different times: no copy and no backup.
    w.write("a/.env", "SAME\n");
    w.write("hub/files/.env", "SAME\n");
    set_age(&w.p("hub/files/.env"), 3600);
    let out = w.sync_ok("a");
    assert!(out.contains("the copy list is the same on both sides"), "{out}");
    assert_eq!(w.read("a/system/backlog.md"), "AAAA\n");
    assert_eq!(w.read("hub/files/system/backlog.md"), "BBBB\n");
    assert!(!w.p("a/system/logs/sync-backups").exists());
}

#[test]
fn the_copy_list_carries_the_tools_env() {
    let w = World::new();
    w.two_machines();
    w.write("a/system/tools/.env", "NOTIFY_TOKEN=x\n");
    w.sync_ok("a");
    w.sync_ok("b");
    assert_eq!(w.read("b/system/tools/.env"), "NOTIFY_TOKEN=x\n");
}

fn hub_url(w: &World) -> String {
    w.git("a/workspace", &["remote", "get-url", "hub"])
}

#[test]
fn a_push_address_that_leaves_the_folder_is_refused_everywhere() {
    let w = World::new();
    w.two_machines();
    w.git("a/workspace", &["config", "remote.hub.pushurl", "git@github.com:someone/life.git"]);
    let hub_before = w.git("hub/workspace.git", &["rev-parse", "master"]);
    edit(&w, "a/workspace/life/open.md", "first loop", "first loop, must not leave");

    let (code, out) = w.sync("a");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("git@github.com:someone/life.git") && out.contains("pushurl"), "{out}");
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    assert!(run_log(&w, "a").contains("resolves to an SSH address, not a folder"), "{}", run_log(&w, "a"));
    let (issues, _) = afcore::life::doctor_checks(&w.ctx("a")).unwrap();
    assert!(issues.iter().any(|i| i.contains("git@github.com:someone/life.git") && i.contains("not a folder path")), "{issues:?}");

    // A rewrite rule that turns the folder into a web address is refused the same way.
    w.git("a/workspace", &["config", "--unset", "remote.hub.pushurl"]);
    let folder = hub_url(&w);
    let prefix = &folder[..folder.rfind('/').unwrap() + 1];
    w.git("a/workspace", &["config", "url.https://github.com/someone/.insteadOf", prefix]);
    let (code, out) = w.sync("a");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("https://github.com/someone/workspace.git") && out.contains("insteadOf"), "{out}");
    let (issues, _) = afcore::life::doctor_checks(&w.ctx("a")).unwrap();
    assert!(issues.iter().any(|i| i.contains("https://github.com/someone/workspace.git")), "{issues:?}");
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), hub_before, "nothing left this machine");

    // init checks the public repository the same way before it pushes.
    w.git("a/workspace", &["config", "--unset", "url.https://github.com/someone/.insteadOf"]);
    w.git("a", &["config", "remote.hub.pushurl", "https://github.com/someone/agentframe.git"]);
    let (code, _, err) = w.af("a", &["sync", "init", &w.hub()]);
    assert_eq!(code, 1);
    assert!(err.contains("https://github.com/someone/agentframe.git"), "{err}");
}

#[test]
fn a_conflict_resolved_by_deleting_the_file_completes() {
    let w = World::new();
    w.two_machines();
    std::fs::remove_file(w.p("a/workspace/life/knowledge/topic.md")).unwrap();
    edit(&w, "b/workspace/life/knowledge/topic.md", "- fact two", "- fact two, edited on B");
    w.sync_ok("a");
    let (code, out) = w.sync("b");
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("SYNC_CONFLICT life/knowledge/topic.md"), "{out}");
    std::fs::remove_file(w.p("b/workspace/life/knowledge/topic.md")).unwrap();
    w.sync_ok("b");
    w.sync_ok("a");
    assert!(!w.p("a/workspace/life/knowledge/topic.md").exists() && !w.p("b/workspace/life/knowledge/topic.md").exists());
    assert_eq!(w.git("a/workspace", &["rev-parse", "HEAD"]), w.git("b/workspace", &["rev-parse", "HEAD"]));
}

#[test]
fn session_start_says_so_when_the_hooks_cannot_sync() {
    let w = World::new();
    w.two_machines();

    // A stale git lock: one line for the chat; af sync then says what to do.
    w.write("a/workspace/.git/index.lock", "");
    set_age(&w.p("a/workspace/.git/index.lock"), 3600);
    let line = sync::hook_half(&w.ctx("a"), false).expect("a stale git lock is reported");
    assert!(line.contains("index.lock") && line.contains("af sync"), "{line}");
    let (code, out) = w.sync("a");
    assert_eq!(code, 1);
    assert!(out.contains("delete workspace/.git/index.lock"), "{out}");
    std::fs::remove_file(w.p("a/workspace/.git/index.lock")).unwrap();

    // A stale sync lock: reported by the hook, cleared by af sync.
    w.write("a/workspace/.git/af-sync.lock", "1");
    set_age(&w.p("a/workspace/.git/af-sync.lock"), 3600);
    let line = sync::hook_half(&w.ctx("a"), false).expect("a stale sync lock is reported");
    assert!(line.contains("af-sync.lock") && line.contains("af sync"), "{line}");
    let out = w.sync_ok("a");
    assert!(out.contains("removed a stale sync lock"), "{out}");

    // A merge left waiting.
    edit(&w, "a/workspace/life/open.md", "second loop", "second loop, A");
    edit(&w, "b/workspace/life/open.md", "second loop", "second loop, B");
    w.sync_ok("a");
    assert_eq!(w.sync("b").0, 3);
    let line = sync::hook_half(&w.ctx("b"), false).expect("a waiting merge is reported");
    assert!(line.contains("waiting to be resolved") && line.contains("af sync"), "{line}");
    assert_eq!(sync::hook_half(&w.ctx("b"), true), None, "the stop hook never prints");
}

#[test]
fn session_start_says_so_after_several_skipped_syncs() {
    let w = World::new();
    w.checkout_a();
    let gone = w.p("nowhere/workspace.git").to_string_lossy().replace('\\', "/");
    w.git("a/workspace", &["remote", "add", "hub", &gone]);
    // An unreachable hub is normal while the other machine is off or away: never a warning.
    for _ in 0..4 {
        assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    }
    assert_eq!(sync::hook_half(&w.ctx("a"), false), None, "unreachable skips do not count");
    // A skip that needs him does count: a git lock held at every hook.
    w.write("a/workspace/.git/index.lock", "");
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    let line = sync::hook_half(&w.ctx("a"), false).expect("the third counted skip in a row reaches the chat");
    assert!(line.contains("last 3 automatic syncs were skipped") && line.contains("git lock") && line.contains("af sync"), "{line}");
}

#[test]
fn only_network_steps_are_killed_at_the_deadline() {
    let w = World::new();
    w.checkout_a();
    let gone = Instant::now();
    let g = sync::Git::new(&w.p("a/workspace"), Some(gone));
    // A local step past the deadline still runs to its end: no lock or half-written tree.
    let st = g.run(&["status", "--porcelain"]);
    assert_eq!(st.code, Some(0), "{}", st.stderr);
    // A step that talks to the hub is killed.
    let net = g.net(&["-c", "alias.hang=!sleep 30", "hang"]);
    assert_eq!(net.code, None);
}

#[test]
fn a_hook_with_too_little_time_left_does_not_start_the_merge() {
    let w = World::new();
    w.two_machines();
    edit(&w, "a/workspace/life/open.md", "first loop", "first loop, from A");
    w.sync_ok("a");
    let head = w.git("b/workspace", &["rev-parse", "HEAD"]);
    // The whole budget is the reserve, so the fetch fits and the merge never starts.
    assert_eq!(sync::hook_half_with(&w.ctx("b"), false, sync::MERGE_RESERVE), None);
    assert!(run_log(&w, "b").contains("too little time left to start the merge"), "{}", run_log(&w, "b"));
    assert_eq!(w.git("b/workspace", &["rev-parse", "HEAD"]), head, "nothing merged");
    assert!(!w.p("b/workspace/.git/MERGE_HEAD").exists());
}

#[test]
fn the_share_trust_is_spelled_as_git_reads_it() {
    isolate_git();
    let share = "//laptop/agentframe-hub/workspace.git";
    let value = sync::safe_directory_value(share);
    assert_eq!(value, "%(prefix)///laptop/agentframe-hub/workspace.git");
    // git expands the value as a path: what it compares with the repository is the share path.
    let o = std::process::Command::new("git")
        .args(["-c", &format!("af.probe={value}"), "config", "--type=path", "--get", "af.probe"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), share);
    assert_eq!(sync::safe_directory_value(r"D:\agentframe-hub\workspace.git"), "D:/agentframe-hub/workspace.git");

    let refusal = "fatal: detected dubious ownership in repository at '//laptop/agentframe-hub/workspace.git'\n\
'//laptop/agentframe-hub/workspace.git' is owned by:\n\tS-1-5-21-1\nbut the current user is:\n\tS-1-5-21-2\n\
To add an exception for this directory, call:\n\n\tgit config --global --add safe.directory '%(prefix)///laptop/agentframe-hub/workspace.git'\n";
    let fix = sync::ownership_fix(refusal).unwrap();
    assert!(fix.contains("git config --global --add safe.directory '%(prefix)///laptop/agentframe-hub/workspace.git'"), "{fix}");
    let bare = "fatal: detected dubious ownership in repository at '//laptop/agentframe-hub/agentframe.git'\n";
    assert!(sync::ownership_fix(bare).unwrap().contains("safe.directory '%(prefix)///laptop/agentframe-hub/agentframe.git'"));
    assert_eq!(sync::ownership_fix("fatal: something else"), None);
}

/// The laptop kit's watchdog tells "not set up yet" from a failure by this phrase.
#[test]
fn a_machine_not_set_up_says_no_hub_remote() {
    let w = World::new();
    w.checkout_a();
    let (code, _, err) = w.af("a", &["sync"]);
    assert_eq!(code, 1);
    // The watchdog, system/tools/laptop/start-agent.ps1, matches this exact phrase.
    assert!(err.contains("the private layer has no hub remote"), "{err}");
}

/// A pre-auto-gc hook that records that git's automatic cleanup started, then cancels it.
/// `repo` is the repository folder, `git_dir` its git folder (the same for a bare one); the
/// marker `auto-gc-ran` appears in the git folder.
fn watch_auto_gc(w: &World, repo: &str, git_dir: &str) {
    w.write(&format!("{git_dir}/hooks/pre-auto-gc"), "#!/bin/sh\ntouch \"$(git rev-parse --absolute-git-dir)/auto-gc-ran\"\nexit 1\n");
    // Two packs over a limit of one: the next automatic cleanup wants to run.
    w.git(repo, &["config", "gc.autoPackLimit", "1"]);
    for n in 0..2 {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(w.p(repo))
            .args(["-c", "fastimport.unpackLimit=0", "fast-import", "--quiet"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(format!("blob\ndata 6\npack{n}\n").as_bytes())?;
                c.wait()
            })
            .unwrap();
        assert!(o.success());
    }
}

#[test]
fn the_hooks_never_run_git_cleanup_and_the_verb_does() {
    let w = World::new();
    w.two_machines();
    watch_auto_gc(&w, "b/workspace", "b/workspace/.git");
    watch_auto_gc(&w, "hub/workspace.git", "hub/workspace.git");
    let ran = w.p("b/workspace/.git/auto-gc-ran");
    let hub_ran = w.p("hub/workspace.git/auto-gc-ran");
    edit(&w, "b/workspace/life/open.md", "first loop", "first loop, committed by the stop hook");
    assert_eq!(sync::hook_half(&w.ctx("b"), true), None);
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), w.git("b/workspace", &["rev-parse", "HEAD"]), "the hook committed and pushed");
    assert!(!ran.exists(), "no automatic cleanup inside a hook");
    assert!(!hub_ran.exists(), "nor on the hub's side of the hook's push");
    w.sync_ok("b");
    assert!(ran.exists(), "af sync runs the cleanup at the end of its round");
    assert!(hub_ran.exists(), "and on the hub's repositories");
}

#[test]
fn the_hub_never_starts_its_own_cleanup_on_a_push() {
    let w = World::new();
    w.checkout_a();
    assert_eq!(w.af("a", &["sync", "init", &w.hub()]).0, 0);
    for repo in ["hub/agentframe.git", "hub/workspace.git"] {
        assert_eq!(w.git(repo, &["config", "--get", "receive.autogc"]), "false", "{repo}");
    }
    // A hub made before this setting existed gets it when init runs again.
    w.git("hub/workspace.git", &["config", "--unset", "receive.autogc"]);
    assert_eq!(w.af("a", &["sync", "init", &w.hub()]).0, 0);
    assert_eq!(w.git("hub/workspace.git", &["config", "--get", "receive.autogc"]), "false");
}

#[test]
fn a_path_with_one_leading_slash_is_refused() {
    let w = World::new();
    w.checkout_a();
    let kind = not_a_folder(r"\agentframe-hub").expect("one backslash");
    assert!(kind.contains(r"D:\folder") && kind.contains(r"\\machine\share"), "the two valid forms are shown: {kind}");
    assert!(not_a_folder(r"\laptop\agentframe-hub").is_some(), "a share typed with one backslash");
    assert_eq!(not_a_folder(r"\\laptop\agentframe-hub"), None);
    if cfg!(windows) {
        // Git for Windows reads `/name` as a folder under its own install folder.
        assert!(not_a_folder("/laptop/agentframe-hub").is_some());
    }
    let (code, _, err) = w.af("a", &["sync", "init", r"\agentframe-hub"]);
    assert_eq!(code, 1);
    assert!(err.contains(r"D:\folder"), "{err}");
    assert_eq!(w.git("a/workspace", &["remote"]), "", "no remote was added");

    w.git("a/workspace", &["remote", "add", "hub", r"\laptop\agentframe-hub\workspace.git"]);
    let (issues, _) = afcore::life::doctor_checks(&w.ctx("a")).unwrap();
    assert!(issues.iter().any(|i| i.contains(r"\laptop\agentframe-hub\workspace.git") && i.contains("not a folder path")), "{issues:?}");
}

#[test]
fn a_conflict_resolved_with_plain_git_clears_the_record() {
    let w = World::new();
    w.two_machines();
    edit(&w, "a/workspace/life/open.md", "second loop", "second loop, A");
    edit(&w, "b/workspace/life/open.md", "second loop", "second loop, B");
    assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
    assert!(sync::hook_half(&w.ctx("b"), false).is_some(), "the conflict is recorded");
    // He resolves it by hand with git, not af sync.
    let merge = std::process::Command::new("git")
        .arg("-C")
        .arg(w.p("b/workspace"))
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "merge", "-q", "hub/master"])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    assert!(w.p("b/workspace/.git/MERGE_HEAD").is_file(), "the merge stopped on the conflict");
    w.write("b/workspace/life/open.md", &OPEN.replace("second loop", "second loop, A and B"));
    w.git("b/workspace", &["add", "-A"]);
    w.git("b/workspace", &["commit", "-q", "-m", "resolved by hand"]);    assert_eq!(sync::hook_half(&w.ctx("b"), true), None);
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), w.git("b/workspace", &["rev-parse", "HEAD"]), "the stop hook pushes again");
    assert_eq!(sync::hook_half(&w.ctx("b"), false), None, "session start no longer reports the conflict");
}

#[test]
fn a_sync_round_keeps_its_lock_fresh() {
    let w = World::new();
    w.two_machines();
    // A copy-list file whose copy refreshes the lock; the round starts with an old lock time.
    w.write("a/system/backlog.md", "## BB-1 — row\n");
    let lock = w.p("a/workspace/.git/af-sync.lock");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let watcher = {
        let (lock, seen) = (lock.clone(), seen.clone());
        std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(20);
            while Instant::now() < end {
                if let Ok(t) = std::fs::metadata(&lock).and_then(|m| m.modified()) {
                    let mut s = seen.lock().unwrap();
                    if !s.contains(&t) {
                        s.push(t);
                    }
                    if s.len() >= 2 {
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    w.sync_ok("a");
    watcher.join().unwrap();
    assert!(seen.lock().unwrap().len() >= 2, "the lock's time moves forward during the round");
}

#[test]
fn git_variables_from_an_outer_git_never_redirect_the_private_commit() {
    let w = World::new();
    w.two_machines();
    let public_head = w.git("b", &["rev-parse", "HEAD"]);
    edit(&w, "b/workspace/life/open.md", "first loop", "first loop, inside a git hook");
    // af started from a git hook in the public repository inherits these.
    let o = Command::cargo_bin("af")
        .unwrap()
        .env("AF_ROOT", w.p("b"))
        .env("GIT_DIR", w.p("b/.git"))
        .env("GIT_WORK_TREE", w.p("b"))
        .env("GIT_INDEX_FILE", w.p("b/.git/index"))
        .args(["sync"])
        .output()
        .unwrap();
    let out = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert_eq!(w.git("b", &["rev-parse", "HEAD"]), public_head, "nothing was committed to the public repository");
    assert!(w.git("b/workspace", &["log", "-1", "--format=%s"]).starts_with("af sync: private layer"), "the private layer got the commit");
    assert_eq!(w.git("b/workspace", &["status", "--porcelain"]), "", "the edit was committed");
    assert_eq!(w.git("hub/workspace.git", &["rev-parse", "master"]), w.git("b/workspace", &["rev-parse", "HEAD"]), "and sent to the hub");
}

#[test]
fn a_hub_remote_that_runs_a_command_or_reaches_webdav_is_refused() {
    let w = World::new();
    w.two_machines();
    for (key, value) in [("remote.hub.receivepack", "touch pwned; git-receive-pack"), ("remote.hub.uploadpack", "git-upload-pack"), ("remote.hub.vcs", "evil")] {
        w.git("a/workspace", &["config", key, value]);
        let (code, out) = w.sync("a");
        assert_eq!(code, 1, "{key}: {out}");
        assert!(out.contains(key) && out.contains(&format!("--unset-all {key}")), "{key}: {out}");
        assert_eq!(sync::hook_half(&w.ctx("a"), true), None);
        let (issues, _) = afcore::life::doctor_checks(&w.ctx("a")).unwrap();
        assert!(issues.iter().any(|i| i.contains(key)), "{key}: {issues:?}");
        w.git("a/workspace", &["config", "--unset-all", key]);
    }
    assert!(!w.p("a/workspace/pwned").exists() && !w.p("hub/workspace.git/pwned").exists());
    assert!(run_log(&w, "a").contains("runs a command"), "{}", run_log(&w, "a"));

    assert!(not_a_folder(r"\\laptop@SSL\DavWWWRoot\hub").is_some(), "the WebDAV form of a share");
    assert!(not_a_folder("//laptop@8443/DavWWWRoot/hub").is_some());
    let kind = not_a_folder("D:agentframe-hub").unwrap();
    assert!(kind.contains("drive-relative") && !kind.contains("SSH"), "{kind}");
    let (code, out) = {
        w.git("a/workspace", &["config", "remote.hub.pushurl", "git@github.com:someone/life.git"]);
        w.sync("a")
    };
    assert_eq!(code, 1);
    assert!(out.contains("git -C workspace remote remove hub"), "the way out is named: {out}");
}

#[test]
fn init_checks_the_clone_address_in_the_scope_the_clone_uses() {
    let w = World::new();
    w.checkout_a();
    assert_eq!(w.af("a", &["sync", "init", &w.hub()]).0, 0);
    w.git("", &["clone", "-q", &w.p("a").to_string_lossy(), "b"]);
    let folder = w.hub().replace('\\', "/");
    let rule = ("url.https://github.com/someone/.insteadOf", format!("{folder}/workspace"));

    // A rule in git's command scope applies to the clone: init refuses before cloning.
    let o = Command::cargo_bin("af")
        .unwrap()
        .env("AF_ROOT", w.p("b"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", rule.0)
        .env("GIT_CONFIG_VALUE_0", &rule.1)
        .args(["sync", "init", &w.hub()])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("https://github.com/someone/.git"), "{err}");
    assert!(!w.p("b/workspace").exists(), "nothing was cloned");

    // A rule only in the public repository's own config: clone never reads it, so it is
    // not the address the clone uses and does not block init.
    w.git("b", &["config", rule.0, &rule.1]);
    let (code, out, err) = w.af("b", &["sync", "init", &w.hub()]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(w.p("b/workspace/life/open.md").is_file());
}

#[test]
fn briefs_merge_line_by_line() {
    assert!(!with_attr_block("").contains("life/briefs"), "nothing re-renders a brief, so keep-ours would drop one");
}
