//! The run log, draft/adopt routing messages, adopt bookkeeping, assembly-record rows,
//! and the doctor buttons note.

mod common;
use afcore::cli;
use afcore::core::Ctx;
use afcore::doctor;
use afcore::error::{crash, CrashKind};
use afcore::runlog;
use common::*;

// ------------------------------------------------------------------ RunLog

#[test]
fn test_refusal_lands_with_its_message() {
    let fx = Fx::new();
    fx.refused(&["ready", "no-such-project", "x"]);
    let rows = fx.run_log_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0]["verb"].as_str().unwrap(), rows[0]["exit"].as_i64().unwrap()), ("ready", 1));
    assert_eq!(rows[0]["argv"], serde_json::json!(["ready", "no-such-project", "x"]));
    let refusal = rows[0]["refusal"].as_str().unwrap();
    assert!(!refusal.is_empty());
    assert!(!refusal.contains("af: ERROR"));
}

#[test]
fn test_success_lands_with_exit_zero_and_creates_the_folder() {
    let fx = Fx::with_packs(&["project-mgmt"]);
    assert!(!fx.exists("system/logs"));
    fx.ok(&["new-project", "anything"]);
    let row = &fx.run_log_rows()[0];
    assert_eq!((row["verb"].as_str().unwrap(), row["exit"].as_i64().unwrap()), ("new-project", 0));
    assert!(row.get("refusal").is_none());
    assert!(chrono::DateTime::parse_from_rfc3339(row["ts"].as_str().unwrap()).is_ok());
}

#[test]
fn test_usage_error_is_labelled_usage() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["draft", "p", "d"]).0, 2);
    let row = &fx.run_log_rows()[0];
    assert_eq!(
        (row["verb"].as_str().unwrap(), row["exit"].as_i64().unwrap(), row["refusal"].as_str().unwrap()),
        ("draft", 2, "usage")
    );
}

#[test]
fn test_crash_is_recorded_and_still_raised() {
    let fx = Fx::new();
    let ctx = fx.ctx();
    let boom = |_: &Ctx, _: &[String]| -> afcore::error::R<i32> { Err(crash(CrashKind::Other, "RuntimeError: kaboom")) };
    let code = cli::main_with(&ctx, vec!["new-project".into(), "anything".into()], &boom);
    assert_eq!(code, 1, "a crash still fails the process");
    let row = &fx.run_log_rows()[0];
    assert_eq!(row["exit"].as_i64().unwrap(), 2);
    assert_eq!(row["crash"].as_str().unwrap(), "RuntimeError: kaboom");
}

#[test]
fn test_crash_from_the_binary_is_recorded() {
    // The same contract end to end: an uncaught decode error is a crash, not a refusal.
    let fx = Fx::new();
    fx.write_bytes("workspace/projects/broken/project.md", b"---\nname: \xff\n---\n");
    let (code, _, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    let row = &fx.run_log_rows()[0];
    assert_eq!(row["exit"].as_i64().unwrap(), 2);
    assert!(row["crash"].as_str().unwrap().starts_with("UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff"));
}

#[test]
fn test_logging_failure_never_changes_the_outcome() {
    let fx = Fx::new();
    fx.write("blocker", "a file where the log folder should be");
    let mut ctx = fx.ctx();
    ctx.run_log = fx.p("blocker/af-runs.jsonl");
    let noop = |_: &Ctx, _: &[String]| -> afcore::error::R<i32> { Ok(0) };
    assert_eq!(cli::main_with(&ctx, vec!["new-project".into(), "anything".into()], &noop), 0);
}

#[test]
fn test_help_is_not_logged() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--help"]).0, 0);
    assert!(fx.run_log_rows().is_empty());
}

#[test]
fn test_subcommand_verbs_keep_their_second_token() {
    let v = |a: &[&str]| runlog::verb_of(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(v(&["pipe", "stage", "acme", "applied"]), "pipe stage");
    assert_eq!(v(&["draft", "p", "d", "--file", "x-v1.md"]), "draft");
    assert_eq!(v(&["studio", "post", "clip", "--url", "u"]), "studio post");
    assert_eq!(v(&[]), "?");
}

// ------------------------------------------------------------------ TempProject

fn campaign(fx: &Fx, rows: &str, domain: &str) {
    fx.write(
        "workspace/projects/campaign/project.md",
        &format!(
            "---\nname: campaign\nslug: campaign\nschema_version: 2026-07-19-v2\ncreated_at: 2026-09-01\ndomain: {domain}\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-09-09T10:00:00-07:00\ndeliverables:\n{rows}---\n"
        ),
    );
}

fn tfx() -> Fx {
    Fx::with_packs(&["marketing", "project-mgmt"])
}

// ------------------------------------------------------------------ DraftAdoptRouting

#[test]
fn test_existing_v1_names_adopt() {
    let fx = tfx();
    campaign(&fx, "", "marketing");
    fx.write("workspace/projects/campaign/brief/brief-v1.md", "---\nstatus: drafting\n---\nbody\n");
    let msg = fx.refused(&["draft", "campaign", "brief", "--file", "brief/brief-v1.md"]);
    assert!(msg.contains("af adopt campaign brief --file"));
}

#[test]
fn test_row_with_an_artifact_names_version() {
    let fx = tfx();
    campaign(&fx, "  brief:\n    status: drafting\n    file: brief/brief-v1.md\n    last_updated: 2026-09-01\n", "marketing");
    fx.write("workspace/projects/campaign/brief/brief-v1.md", "---\nstatus: drafting\n---\nbody\n");
    let msg = fx.refused(&["draft", "campaign", "brief", "--file", "brief/brief-v1.md"]);
    assert!(msg.contains("af version campaign brief"));
}

#[test]
fn test_non_canonical_name_says_adopt_registers_existing_files() {
    let fx = tfx();
    campaign(&fx, "", "marketing");
    let msg = fx.refused(&["draft", "campaign", "brief", "--file", "brief/_seed.md"]);
    assert!(msg.contains("<slug>-v1.md"));
    assert!(msg.contains("af adopt"));
}

#[test]
fn test_missing_parent_row_names_the_file_route_not_a_hand_edit() {
    let fx = tfx();
    campaign(&fx, "", "marketing");
    let msg = fx.refused(&["draft", "campaign", "post-9", "--artifact", "body-copy"]);
    assert!(msg.contains("af draft campaign post-9 --file"));
    assert!(msg.contains("Never hand-write the row"));
}

// ------------------------------------------------------------------ AdoptStampsBookkeeping

#[test]
fn test_missing_last_updated_is_stamped_and_version_then_works() {
    let fx = tfx();
    campaign(&fx, "", "marketing");
    let rel = "workspace/projects/campaign/spine/spine-v1.md";
    fx.write(rel, "---\nstatus: drafting\n---\n# Spine\n\nbody\n");
    let out = fx.ok(&["adopt", "campaign", "spine", "--file", "spine/spine-v1.md"]);
    assert!(out.contains("last_updated stamped"));
    assert_eq!(get_scalar(&fx.fm(rel), "last_updated"), Some(today()));
    assert!(fx.body(rel).contains("# Spine"));
    fx.ok(&["version", "campaign", "spine"]);
    assert!(fx.isfile("workspace/projects/campaign/spine/spine-v2.md"));
}

#[test]
fn test_present_last_updated_is_left_alone() {
    let fx = tfx();
    campaign(&fx, "", "marketing");
    let rel = "workspace/projects/campaign/spine/spine-v1.md";
    fx.write(rel, "---\nstatus: drafting\nlast_updated: 2026-01-01\n---\nbody\n");
    let out = fx.ok(&["adopt", "campaign", "spine", "--file", "spine/spine-v1.md"]);
    assert!(!out.contains("stamped"));
    assert_eq!(get_scalar(&fx.fm(rel), "last_updated").as_deref(), Some("2026-01-01"));
}

// ------------------------------------------------------------------ AssemblyRecordRows

fn assembly_issues(domain: &str) -> Vec<String> {
    let fx = tfx();
    campaign(
        &fx,
        "  post-2-launch:\n    status: drafting\n    file: posts/post-2-launch/post-FINAL.md\n    last_updated: 2026-09-07\n",
        domain,
    );
    fx.write(
        "workspace/projects/campaign/posts/post-2-launch/post-FINAL.md",
        "---\nstatus: drafting\nlast_updated: 2026-09-07\n---\n",
    );
    doctor::check_project(&fx.ctx(), &fx.p("workspace/projects/campaign"))
        .unwrap()
        .into_iter()
        .filter(|i| i.contains("numeric version head"))
        .collect()
}

#[test]
fn test_any_marketing_row_on_an_assembly_record_is_clean() {
    assert_eq!(assembly_issues("marketing"), Vec::<String>::new());
}

#[test]
fn test_outside_marketing_the_check_still_fires() {
    assert_eq!(assembly_issues("project-mgmt").len(), 1);
}

// ------------------------------------------------------------------ ButtonNotes

struct Log {
    fx: Fx,
    ctx: Ctx,
}

impl Log {
    fn new() -> Log {
        let fx = Fx::new();
        let mut ctx = fx.ctx();
        ctx.run_log = fx.p("logs/af-runs.jsonl");
        Log { fx, ctx }
    }

    fn add(&self, verb: &str, exit: i64, refusal: Option<&str>, crash: Option<&str>, days_ago: i64) {
        let ts = (chrono::Utc::now() - chrono::Duration::days(days_ago)).format("%Y-%m-%dT%H:%M:%S+00:00").to_string();
        let mut row = serde_json::json!({"ts": ts, "verb": verb, "argv": verb.split(' ').collect::<Vec<_>>(), "exit": exit});
        if let Some(r) = refusal {
            row["refusal"] = r.into();
        }
        if let Some(c) = crash {
            row["crash"] = c.into();
        }
        std::fs::create_dir_all(self.fx.path("logs")).unwrap();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.ctx.run_log).unwrap();
        writeln!(f, "{row}").unwrap();
    }
}

#[test]
fn test_silent_when_nothing_failed() {
    let l = Log::new();
    l.add("draft", 0, None, None, 0);
    l.add("doctor", 1, None, None, 0); // findings, not a refusal
    l.add("search", 1, None, None, 0);
    assert_eq!(runlog::button_notes(&l.ctx), Vec::<String>::new());
}

#[test]
fn test_silent_when_the_log_is_missing_or_garbled() {
    let l = Log::new();
    assert_eq!(runlog::button_notes(&l.ctx), Vec::<String>::new());
    l.fx.write("logs/af-runs.jsonl", "not json\n{}\n");
    assert_eq!(runlog::button_notes(&l.ctx), Vec::<String>::new());
}

#[test]
fn test_counts_refusals_crashes_and_slips_by_verb() {
    let l = Log::new();
    l.add("draft", 1, Some("already has a version chain"), None, 0);
    l.add("draft", 1, Some("repo-root-relative"), None, 0);
    l.add("adopt", 1, Some("status must be drafting"), None, 0);
    l.add("draft", 2, None, Some("SyntaxError: mid-edit"), 0);
    l.add("version", 2, Some("usage"), None, 0);
    l.add("pipe board", 1, None, None, 0);
    let notes = runlog::button_notes(&l.ctx);
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("3 refusal(s): draft 2, adopt 1"));
    assert!(notes[0].contains("1 crash(es): draft 1"));
    assert!(notes[0].contains("1 usage slip(s)"));
    assert!(notes[0].contains("needs a row"));
}

#[test]
fn test_old_rows_fall_out_of_the_window() {
    let l = Log::new();
    l.add("draft", 1, Some("old"), None, runlog::BUTTON_WINDOW_DAYS + 1);
    assert_eq!(runlog::button_notes(&l.ctx), Vec::<String>::new());
}

#[test]
fn test_open_backlog_rows_naming_af_are_counted() {
    let l = Log::new();
    l.fx.write(
        "root/system/backlog.md",
        "# Backlog\n\nprose mentioning af ready that is not a row\n\n\
## BB-2026-09-01-01 — prose only\nnothing about buttons here\nStatus: open\n\n\
## BB-2026-09-01-02 — `af ready` refuses a shape\nthe button dies on it\nStatus: open\n\n\
## BB-2026-09-01-04 — `af draft` crashed once\nfixed in the parser\nStatus: done\n\n\
```yaml\n- id: BB-2026-09-01-03\n  one_line: af draft path bug\n```\n",
    );
    let ctx = Ctx::new(&l.fx.p("root"));
    assert_eq!(runlog::open_backlog_rows_naming_af(&ctx), 2);
}
