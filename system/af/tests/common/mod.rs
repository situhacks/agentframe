//! Shared fixture helpers for the integration tests: a temp repo root, real packs copied
//! in, the binary pointed at it through AF_ROOT, and the library API for helper checks.
#![allow(dead_code)]

use afcore::core::Ctx;
use afcore::fm;
use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const SCHEMA: &str = "2026-07-19-v2";

/// The worktree the crate lives in (system/af/ -> repo root).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

pub fn days_ago(n: i64) -> String {
    (chrono::Local::now().date_naive() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string()
}

pub fn days_ahead(n: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(n)).format("%Y-%m-%d").to_string()
}

pub fn today() -> String {
    afcore::core::today()
}

pub struct Fx {
    pub dir: tempfile::TempDir,
}

impl Fx {
    pub fn new() -> Fx {
        Fx { dir: tempfile::tempdir().unwrap() }
    }

    /// A temp root carrying copies of the named real packs under library/domains/.
    pub fn with_packs(packs: &[&str]) -> Fx {
        let fx = Fx::new();
        for p in packs {
            fx.copy_pack(p, p);
        }
        fx
    }

    pub fn copy_pack(&self, pack: &str, as_name: &str) {
        let src = repo_root().join("library").join("domains").join(pack);
        copy_tree(&src, &self.path(&format!("library/domains/{as_name}")));
    }

    pub fn root(&self) -> String {
        self.dir.path().to_string_lossy().into_owned()
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        let mut p = self.dir.path().to_path_buf();
        for part in rel.split('/') {
            if !part.is_empty() {
                p.push(part);
            }
        }
        p
    }

    pub fn p(&self, rel: &str) -> String {
        self.path(rel).to_string_lossy().into_owned()
    }

    pub fn write(&self, rel: &str, text: &str) -> String {
        self.write_bytes(rel, text.as_bytes())
    }

    pub fn write_bytes(&self, rel: &str, data: &[u8]) -> String {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, data).unwrap();
        p.to_string_lossy().into_owned()
    }

    pub fn read(&self, rel: &str) -> String {
        afcore::core::read(&self.p(rel)).unwrap()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    pub fn isfile(&self, rel: &str) -> bool {
        self.path(rel).is_file()
    }

    pub fn mkdir(&self, rel: &str) {
        std::fs::create_dir_all(self.path(rel)).unwrap();
    }

    pub fn rmtree(&self, rel: &str) {
        std::fs::remove_dir_all(self.path(rel)).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.path(rel)).unwrap();
    }

    /// Frontmatter of a file under the root.
    pub fn fm(&self, rel: &str) -> String {
        fm::split_fm(&self.read(rel), rel).unwrap().0
    }

    pub fn body(&self, rel: &str) -> String {
        fm::split_fm(&self.read(rel), rel).unwrap().1
    }

    pub fn ctx(&self) -> Ctx {
        Ctx::new(&self.root())
    }

    /// The binary, rooted at this fixture.
    pub fn af(&self) -> Command {
        let mut c = Command::cargo_bin("af").unwrap();
        // No test reaches the real panel: the driver is a program that does not exist unless a test names a stub.
        c.env("AF_ROOT", self.root()).env("AF_PANEL_DRIVER", self.path("no-such-panel-driver")).current_dir(self.dir.path());
        // The chat running the tests is not the caller of a test's af.
        c.env_remove("CLAUDE_CODE_SESSION_ID");
        c
    }

    /// Run the binary; (exit code, stdout, stderr).
    pub fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.af().args(args).output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// Run and require exit 0; stdout.
    pub fn ok(&self, args: &[&str]) -> String {
        let (code, out, err) = self.run(args);
        assert_eq!(code, 0, "af {args:?} failed: {err}{out}");
        out
    }

    /// Run and require a refusal (exit 1); stderr.
    pub fn refused(&self, args: &[&str]) -> String {
        let (code, out, err) = self.run(args);
        assert_eq!(code, 1, "af {args:?} expected a refusal, got {code}: {out}{err}");
        err
    }

    pub fn run_log_rows(&self) -> Vec<serde_json::Value> {
        let p = self.path("system/logs/af-runs.jsonl");
        if !p.is_file() {
            return Vec::new();
        }
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

pub fn copy_tree(src: &Path, dst: &Path) {
    for e in walkdir::WalkDir::new(src) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(src).unwrap();
        if rel.components().any(|c| c.as_os_str() == "__pycache__") {
            continue;
        }
        let target = dst.join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
        } else {
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

pub fn row_get(fm_text: &str, slug: &str, key: &str) -> Option<String> {
    fm::row_get(fm_text, slug, key)
}

pub fn get_scalar(fm_text: &str, key: &str) -> Option<String> {
    fm::get_scalar(fm_text, key)
}

pub fn any_contains(items: &[String], needle: &str) -> bool {
    items.iter().any(|i| i.contains(needle))
}

// ------------------------------------------------------------------ pipeline fixtures

/// What `af pipe save` + `af pipe start` leave behind (board row + application folder),
/// written directly because the pipe verbs are not ported yet.
pub struct App<'a> {
    pub slug: &'a str,
    pub stage: &'a str,
    pub jd: bool,
    pub materials: &'a str,
    pub resume_status: &'a str,
    pub extra_rows: &'a str,
    pub board_extra: &'a str,
    pub archived: bool,
}

impl<'a> App<'a> {
    pub fn new(slug: &'a str) -> App<'a> {
        App {
            slug,
            stage: "preparing",
            jd: true,
            materials: "[resume]",
            resume_status: "not_started",
            extra_rows: "",
            board_extra: "",
            archived: false,
        }
    }
}

pub fn board_row(slug: &str, stage: &str, extra: &str) -> String {
    format!(
        "  {slug}:\n    stage: {stage}\n    company: \"Acme\"\n    role: \"AI PM\"\n    url: https://boards.greenhouse.io/acme/jobs/1\n    ats: greenhouse\n    source: manual\n    saved: {}\n{extra}",
        today()
    )
}

pub fn write_board(fx: &Fx, rows: &str) {
    let t = today();
    fx.write(
        "workspace/pipeline/pipeline.md",
        &format!(
            "---\nschema_version: 1\ncreated_at: {t}\nlast_activity: {t}T10:00:00-04:00\napplications:\n{rows}\n---\n\n# Job Pipeline\n"
        ),
    );
}

pub fn app_dir_rel(slug: &str, archived: bool) -> String {
    if archived {
        format!("workspace/pipeline/applications/completed/{slug}")
    } else {
        format!("workspace/pipeline/applications/{slug}")
    }
}

pub fn write_application(fx: &Fx, a: &App) -> String {
    let t = today();
    let d = app_dir_rel(a.slug, a.archived);
    fx.write(
        &format!("{d}/application.md"),
        &format!(
            "---\n# IDENTITY\nname: \"Acme - AI PM\"\nslug: {slug}\nschema_version: 1\ncreated_at: {t}\ndomain: careers\n\n\
# POSTING (fixed facts — funnel stage lives on the pipeline board, never here)\ncompany: \"Acme\"\nrole: \"AI PM\"\n\
job_url: https://boards.greenhouse.io/acme/jobs/1\nsource: manual\nats: greenhouse\ndate_posted: null\nsalary_range: \"\"\n\n\
# LIFECYCLE\nlast_activity: {t}T10:00:00-04:00\n\n# MATERIALS (submission rows; first = primary and drives `shipped`)\n\
materials: {materials}\n\n# DELIVERABLES\ndeliverables:\n{extra}  resume:\n    file: resume/resume-v1.md\n    status: {rs}\n\
  cover-letter:\n    file: cover-letter/cover-letter-v1.md\n    status: not_started\n---\n\n# Acme - AI PM\n\n## Notes\n",
            slug = a.slug,
            materials = a.materials,
            extra = a.extra_rows,
            rs = a.resume_status
        ),
    );
    fx.write(
        &format!("{d}/activity.md"),
        &format!("{t} 10:00 — application_started: Acme - AI PM scaffolded (careers)\n"),
    );
    fx.write(&format!("{d}/sources/INDEX.md"), "# Sources — Acme - AI PM\n");
    fx.mkdir(&format!("{d}/correspondence"));
    fx.mkdir(&format!("{d}/people"));
    if a.jd {
        fx.write(&format!("{d}/jd.md"), "verbatim JD");
    }
    d
}

/// One started application on a fresh board.
pub fn pipe_started(fx: &Fx, a: &App) -> String {
    write_board(fx, &board_row(a.slug, a.stage, a.board_extra));
    write_application(fx, a)
}

// ------------------------------------------------------------------ studio fixtures

pub fn write_calendar(fx: &Fx, rows: &str) {
    let t = today();
    fx.write(
        "workspace/studio/calendar.md",
        &format!(
            "---\nschema_version: 1\ncreated_at: {t}\nlast_activity: {t}T10:00:00-04:00\nslots:\n{rows}\n---\n\n# Studio Calendar\n"
        ),
    );
}

pub fn slot_row(slug: &str, state: &str, extra: &str) -> String {
    format!("  {slug}:\n    state: {state}\n    platform: tiktok\n    created: {}\n{extra}", today())
}

/// What `af studio new` writes for one post folder (post.md + edit.md, source when captured).
pub fn write_post(fx: &Fx, slug: &str, date: Option<&str>, captured: bool, receipt: Option<(&str, &str)>) -> String {
    let t = today();
    let d = format!("workspace/studio/posts/{slug}");
    let (posted_at, url) = match receipt {
        Some((p, u)) => (p.to_string(), format!("\"{u}\"")),
        None => ("null".to_string(), "null".to_string()),
    };
    let title: String = afcore::pyos::py_title(&slug.replace('-', " "));
    fx.write(
        &format!("{d}/post.md"),
        &format!(
            "---\n# IDENTITY\nname: \"{title}\"\nslug: {slug}\nschema_version: 1\ncreated_at: {t}\ndomain: creator\n\n\
# POST (fixed facts; state lives on the calendar board, never here)\ntype: video\nplatform: tiktok\ndate: {date}\n\
series: null\nhook: null\nsource_project: null\nsource_file: {src}\n\n# RECEIPT (written by `af studio post`)\n\
posted_at: {posted_at}\nurl: {url}\nmetrics_captured_at: null\n\n# LIFECYCLE\nlast_activity: {t}T10:00:00-04:00\n\n\
# DELIVERABLES (only a drafted script.md earns a row; edit.md, media and renders are never rows)\ndeliverables: {{}}\n---\n\n# {title}\n",
            date = date.unwrap_or("null"),
            src = if captured { "video/source.mov" } else { "null" },
        ),
    );
    fx.write(&format!("{d}/edit.md"), &format!("# Edit — {title}\n"));
    if captured {
        fx.write(&format!("{d}/video/source.mov"), "fake video bytes");
    }
    d
}

/// The panel driver stub's source. It is the driver for `AF_PANEL_DRIVER` and, answering `agents`,
/// the program for `AF_CLAUDE_BIN` as well. It appends each call to `STUB_PANEL_CALLS` as a JSON line
/// (its arguments, and the text of the file named by `-TextFile`, read while it still exists).
/// `STUB_PANEL_EXITS` (comma list, one per call, the last repeats) or `STUB_PANEL_EXIT` set the exit
/// code, `STUB_PANEL_STDERR` its message. `list` prints `STUB_PANEL_LIST`; `new` prints `STUB_PANEL_NEW_ID`;
/// a `send` of text holding `/compact` appends a `compact_boundary` line to `STUB_PANEL_TRANSCRIPT`
/// unless `STUB_PANEL_NO_BOUNDARY` is set. `STUB_PANEL_SLEEP_SECS` makes every call slow;
/// `STUB_PANEL_NOTE` prints a `note: ...` line on stderr, as the driver does, on success and failure.
/// With `STUB_LOCK_SNAPSHOT` and `STUB_LOCK_FILE` every call appends what the lock file holds then.
/// `STUB_PANEL_WAIT_FOR_CANCEL` (seconds), `STUB_PANEL_SENDING` and `STUB_PANEL_IGNORE_CANCEL` make a call that
/// is given `-CancelFile` wait for that file as the real driver does (see below).
const PANEL_STUB_SRC: &str = r##"
use std::io::Write;
fn append(path: &str, text: &str) {
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
}
fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action = args.first().cloned().unwrap_or_default();
    if action == "agents" {
        if std::env::var("STUB_ROSTER_FAIL").is_ok() {
            std::process::exit(1);
        }
        print!("{}", std::env::var("STUB_ROSTER").unwrap_or_else(|_| "[]".into()));
        return;
    }
    if let (Ok(snap), Ok(lock)) = (std::env::var("STUB_LOCK_SNAPSHOT"), std::env::var("STUB_LOCK_FILE")) {
        let held = std::fs::read_to_string(&lock).unwrap_or_else(|_| "none".into());
        append(&snap, &format!("{held}
"));
    }
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let text = value("-TextFile").and_then(|p| std::fs::read_to_string(p).ok());
    let mut index = 0usize;
    if let Ok(calls) = std::env::var("STUB_PANEL_CALLS") {
        index = std::fs::read_to_string(&calls).map(|t| t.lines().count()).unwrap_or(0);
        let quoted: Vec<String> = args.iter().map(|a| format!("\"{}\"", esc(a))).collect();
        let text_json = match &text {
            Some(t) => format!("\"{}\"", esc(t)),
            None => "null".to_string(),
        };
        append(&calls, &format!("{{\"args\":[{}],\"text\":{}}}\n", quoted.join(","), text_json));
    }
    if let Ok(secs) = std::env::var("STUB_PANEL_SLEEP_SECS") {
        std::thread::sleep(std::time::Duration::from_secs_f64(secs.parse().unwrap()));
    }
    // A driver that waits and honours -CancelFile: it looks for the file for up to STUB_PANEL_WAIT_FOR_CANCEL
    // seconds and exits 7 when it finds it. STUB_PANEL_SENDING makes it write the `.sending` marker first (the
    // real driver does that when it first changes the panel) and ignore the cancel; STUB_PANEL_IGNORE_CANCEL
    // ignores it with no marker (a driver that does not stop in time).
    if let (Some(cancel), Ok(secs)) = (value("-CancelFile"), std::env::var("STUB_PANEL_WAIT_FOR_CANCEL")) {
        let sending = std::env::var("STUB_PANEL_SENDING").is_ok();
        if sending {
            std::fs::write(std::path::Path::new(&cancel).with_extension("sending"), "begun").unwrap();
        }
        let honour = !sending && std::env::var("STUB_PANEL_IGNORE_CANCEL").is_err();
        let until = std::time::Instant::now() + std::time::Duration::from_secs_f64(secs.parse().unwrap());
        while std::time::Instant::now() < until {
            if honour && std::path::Path::new(&cancel).exists() {
                eprintln!("cancelled; nothing was changed");
                std::process::exit(7);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    let code = match std::env::var("STUB_PANEL_EXITS") {
        Ok(list) => {
            let codes: Vec<i32> = list.split(',').map(|c| c.trim().parse().unwrap()).collect();
            codes[index.min(codes.len() - 1)]
        }
        Err(_) => std::env::var("STUB_PANEL_EXIT").ok().and_then(|c| c.parse().ok()).unwrap_or(0),
    };
    if code != 0 {
        eprintln!("{}", std::env::var("STUB_PANEL_STDERR").unwrap_or_else(|_| "the stub driver refused".into()));
        if let Ok(note) = std::env::var("STUB_PANEL_NOTE") {
            eprintln!("note: {note}");
        }
        std::process::exit(code);
    }
    if let Ok(note) = std::env::var("STUB_PANEL_NOTE") {
        eprintln!("note: {note}");
    }
    match action.as_str() {
        "list" => print!("{}", std::env::var("STUB_PANEL_LIST").unwrap_or_else(|_| "[]".into())),
        "new" => println!("{}", std::env::var("STUB_PANEL_NEW_ID").unwrap_or_else(|_| "11111111-2222-3333-4444-555555555555".into())),
        "send" => {
            if let Ok(t) = std::env::var("STUB_PANEL_TRANSCRIPT") {
                if text.as_deref().is_some_and(|x| x.contains("/compact")) && std::env::var("STUB_PANEL_NO_BOUNDARY").is_err() {
                    append(&t, "{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"content\":\"Conversation compacted\"}\n");
                    if let Ok(line) = std::env::var("STUB_PANEL_AFTER_LINE") {
                        append(&t, &format!("{line}\n"));
                    }
                }
            }
            println!("sent");
        }
        _ => println!("ok"),
    }
}
"##;

/// The panel stub, compiled once for the whole test run.
pub fn panel_stub() -> &'static Path {
    static STUB: OnceLock<PathBuf> = OnceLock::new();
    STUB.get_or_init(|| {
        let target = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
        let dir = target.join("af-test-stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("panel_stub.rs");
        let exe = dir.join(if cfg!(windows) { "panel_stub.exe" } else { "panel_stub" });
        std::fs::write(&src, PANEL_STUB_SRC).unwrap();
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
        let status = std::process::Command::new(rustc).args(["--edition", "2021", "-o"]).arg(&exe).arg(&src).status().unwrap();
        assert!(status.success(), "the panel stub did not compile");
        exe
    })
}
