//! Port of system/tests/test_af_pipe_rounds.py: start's raw-input folders, the inbound
//! skip of `applied`, `af pipe round`, and archive/unarchive. TestRoundDoctorNotes and the
//! fixture-written archived-doctor case are in af_pipe.rs.

mod common;
use afcore::pipe;
use common::*;
use std::path::MAIN_SEPARATOR_STR as SEP;

fn pfx() -> Fx {
    let fx = Fx::with_packs(&["careers"]);
    fx.mkdir("workspace/projects");
    fx
}

fn save(fx: &Fx, slug: &str) -> String {
    fx.ok(&[
        "pipe", "save", "--company", "Acme", "--role", "AI PM", "--url", "https://boards.greenhouse.io/acme/jobs/1",
        "--ats", "greenhouse", "--slug", slug,
    ]);
    slug.to_string()
}

fn stage(fx: &Fx, slug: &str, st: &str) {
    fx.ok(&["pipe", "stage", slug, st]);
}

/// `RoundBase.started()`: saved, started, jd.md written.
fn started(fx: &Fx) -> String {
    let slug = save(fx, "acme-ai-pm");
    fx.ok(&["pipe", "start", &slug]);
    fx.write(&format!("{}/jd.md", app_dir_rel(&slug, false)), "verbatim JD");
    slug
}

fn round(fx: &Fx, slug: &str, n: &str, extra: &[&str]) {
    let mut args = vec!["pipe", "round", slug, n];
    args.extend_from_slice(extra);
    fx.ok(&args);
}

fn readme(fx: &Fx, slug: &str, folder: &str) -> String {
    fx.read(&format!("{}/{folder}/README.md", app_dir_rel(slug, false)))
}

fn board_fm(fx: &Fx) -> String {
    fx.fm("workspace/pipeline/pipeline.md")
}

// ------------------------------------------------------------------ TestStartScaffoldsInputs

#[test]
fn test_sources_correspondence_people_created() {
    let fx = pfx();
    let slug = started(&fx);
    for sub in ["sources", "correspondence", "people"] {
        assert!(fx.path(&format!("{}/{sub}", app_dir_rel(&slug, false))).is_dir(), "{sub}");
    }
}

#[test]
fn test_sources_index_written_from_pack_skeleton() {
    let fx = pfx();
    let slug = started(&fx);
    let idx = fx.read(&format!("{}/sources/INDEX.md", app_dir_rel(&slug, false)));
    assert!(idx.contains("Acme - AI PM"));
    assert!(idx.contains("Never prep from this folder"));
    assert!(!idx.contains("{name}"));
}

// ------------------------------------------------------------------ TestInboundSkipsApplied

#[test]
fn test_preparing_to_interviewing_is_legal() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "interviewing");
    let fm = board_fm(&fx);
    assert_eq!(row_get(&fm, &slug, "stage").as_deref(), Some("interviewing"));
    assert_eq!(row_get(&fm, &slug, "applied"), None);
    assert_eq!(row_get(&fm, &slug, "shipped"), None);
}

#[test]
fn test_skip_is_named_in_output() {
    let fx = pfx();
    let slug = started(&fx);
    let out = fx.ok(&["pipe", "stage", &slug, "interviewing"]);
    assert!(out.contains("submitted_by"), "{out}");
}

#[test]
fn test_saved_to_interviewing_still_refused() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.refused(&["pipe", "stage", &slug, "interviewing"]);
}

// ------------------------------------------------------------------ TestPipeRound

#[test]
fn test_folder_and_readme_from_pack_skeleton() {
    let fx = pfx();
    let slug = started(&fx);
    round(
        &fx,
        &slug,
        "2",
        &["--with", "Charles Duckworth, Senior Director", "--type", "hiring-manager", "--at", "2026-09-02T10:00:00-07:00"],
    );
    assert!(fx.path(&format!("{}/round-2-charles", app_dir_rel(&slug, false))).is_dir());
    let text = readme(&fx, &slug, "round-2-charles");
    assert!(text.contains("Charles Duckworth, Senior Director"));
    assert!(text.contains("hiring-manager"));
    assert!(text.contains("round 1")); // {prev} resolves to the prior round
    assert!(!text.contains("{people}"));
}

#[test]
fn test_panel_lists_every_person() {
    let fx = pfx();
    let slug = started(&fx);
    round(&fx, &slug, "3", &["--with", "Dave McKellar, Partner", "--with", "Jeff O'Neill, Senior Manager", "--type", "panel"]);
    let text = readme(&fx, &slug, "round-3-dave");
    assert!(text.contains("Dave McKellar"));
    assert!(text.contains("Jeff O'Neill"));
}

#[test]
fn test_unnamed_interviewer_is_called_out() {
    let fx = pfx();
    let slug = started(&fx);
    round(&fx, &slug, "1", &["--type", "recruiter-screen"]);
    assert!(readme(&fx, &slug, "round-1-recruiter-screen").contains("No interviewer named yet"));
}

#[test]
fn test_first_round_prev_points_at_application_root() {
    let fx = pfx();
    let slug = started(&fx);
    round(&fx, &slug, "1", &["--with", "Mike Duffy, Recruiter"]);
    assert!(readme(&fx, &slug, "round-1-mike").contains("the application root"));
}

#[test]
fn test_duplicate_round_refused() {
    let fx = pfx();
    let slug = started(&fx);
    round(&fx, &slug, "1", &["--with", "Mike Duffy, Recruiter"]);
    fx.refused(&["pipe", "round", &slug, "1", "--with", "Mike Duffy, Recruiter"]);
}

#[test]
fn test_terminal_stage_refused() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "dropped");
    fx.refused(&["pipe", "round", &slug, "1", "--with", "Mike Duffy, Recruiter"]);
}

#[test]
fn test_missing_application_folder_refused() {
    let fx = pfx();
    let slug = save(&fx, "no-folder-yet");
    fx.refused(&["pipe", "round", &slug, "1", "--with", "Mike Duffy, Recruiter"]);
}

// ------------------------------------------------------------------ TestArchive

#[test]
fn test_moves_folder_and_keeps_board_row() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "rejected");
    fx.ok(&["pipe", "archive", &slug]);
    assert!(!fx.path(&app_dir_rel(&slug, false)).is_dir());
    assert!(fx.isfile(&format!("{}/application.md", app_dir_rel(&slug, true))));
    assert_eq!(row_get(&board_fm(&fx), &slug, "stage").as_deref(), Some("rejected"));
}

#[test]
fn test_app_dir_resolves_archived() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "rejected");
    fx.ok(&["pipe", "archive", &slug]);
    let ctx = fx.ctx();
    assert!(pipe::app_is_archived(&ctx, &slug));
    assert!(pipe::app_dir(&ctx, &slug).ends_with(&format!("{}{SEP}{slug}", pipe::APPS_ARCHIVE)));
}

#[test]
fn test_active_stage_refused() {
    // No override exists: retiring a live row means staging it, which keeps the record honest.
    let fx = pfx();
    let slug = started(&fx);
    fx.refused(&["pipe", "archive", &slug]);
    stage(&fx, &slug, "dropped");
    fx.ok(&["pipe", "archive", &slug]);
    assert!(pipe::app_is_archived(&fx.ctx(), &slug));
}

#[test]
fn test_unarchive_returns_the_folder() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "ghosted");
    fx.ok(&["pipe", "archive", &slug]);
    fx.ok(&["pipe", "unarchive", &slug]);
    assert!(!pipe::app_is_archived(&fx.ctx(), &slug));
    assert!(fx.isfile(&format!("{}/application.md", app_dir_rel(&slug, false))));
}

#[test]
fn test_unarchive_refused_when_not_archived() {
    let fx = pfx();
    let slug = started(&fx);
    fx.refused(&["pipe", "unarchive", &slug]);
}

#[test]
fn test_archived_application_stays_clean_in_doctor_through_the_verbs() {
    let fx = pfx();
    let slug = started(&fx);
    stage(&fx, &slug, "rejected");
    fx.ok(&["pipe", "archive", &slug]);
    let (issues, _) = afcore::doctor::check_pipeline(&fx.ctx()).unwrap();
    assert_eq!(issues, Vec::<String>::new());
}

// ------------------------------------------------------------------ argparse surface

#[test]
fn test_round_number_parses_like_python_int() {
    let fx = pfx();
    let slug = started(&fx);
    round(&fx, &slug, "007", &["--folder", "lead"]);
    assert!(fx.path(&format!("{}/round-7-lead", app_dir_rel(&slug, false))).is_dir());
    let (code, _, _) = fx.run(&["pipe", "round", &slug, "x"]);
    assert_eq!(code, 2);
    let (code, _, _) = fx.run(&["pipe", "round", &slug, "8", "--type", "bogus"]);
    assert_eq!(code, 2);
}

#[test]
fn test_unnamed_lead_person_is_the_python_crash() {
    // af.py indexes `split()[0]` of the first person's name; an empty name raises IndexError.
    let fx = pfx();
    let slug = started(&fx);
    let (code, _, err) = fx.run(&["pipe", "round", &slug, "5", "--with", ", nobody"]);
    assert_eq!(code, 1);
    assert!(err.contains("IndexError: list index out of range"), "{err}");
    let last = fx.run_log_rows().pop().unwrap();
    assert_eq!(last["exit"], 2);
    assert_eq!(last["crash"], "IndexError: list index out of range");
}
