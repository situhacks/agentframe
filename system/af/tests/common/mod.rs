//! Shared fixture helpers for the integration tests: a temp repo root, real packs copied
//! in, the binary pointed at it through AF_ROOT, and the library API for helper checks.
#![allow(dead_code)]

use afcore::core::Ctx;
use afcore::fm;
use assert_cmd::Command;
use std::path::{Path, PathBuf};

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
        c.env("AF_ROOT", self.root()).current_dir(self.dir.path());
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
