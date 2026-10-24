//! In-scope cases of system/tests/test_af_pipe.py and test_af_pipe_rounds.py: new-project's
//! topology refusal, the pipeline doctor (board, folders, careers lint, round notes), and
//! the careers readiness gates on application folders.
//!
//! The `pipe` verbs are not ported (R2+), so fixtures write the board and application
//! folders `af pipe save` / `af pipe start` / `af pipe archive` leave behind (common::App).
//! Cases that exercise only pipe verbs stay on the Python suite.

mod common;
use afcore::doctor;
use afcore::fm::{join_fm, row_set, set_scalar, split_fm};
use common::*;

fn pfx() -> Fx {
    Fx::with_packs(&["careers"])
}

fn check(fx: &Fx) -> (Vec<String>, Vec<String>) {
    doctor::check_pipeline(&fx.ctx()).unwrap()
}

fn edit_app(fx: &Fx, d: &str, f: impl Fn(String) -> String) {
    let ap = format!("{d}/application.md");
    let (afm, abody) = split_fm(&fx.read(&ap), "application.md").unwrap();
    fx.write(&ap, &join_fm(&f(afm), &abody));
}

// ------------------------------------------------------------------ TestSaveAndStart

#[test]
fn test_new_project_refuses_pipeline_topology() {
    let fx = pfx();
    fx.mkdir("workspace/projects");
    fx.refused(&["new-project", "acme", "--domain", "careers"]);
}

// ------------------------------------------------------------------ TestPipelineDoctor

#[test]
fn test_clean_pipeline_is_quiet() {
    let fx = pfx();
    pipe_started(&fx, &App::new("acme-ai-pm"));
    let (issues, notes) = check(&fx);
    assert_eq!(issues, Vec::<String>::new());
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn test_missing_folder_and_orphan_folder_are_issues() {
    let fx = pfx();
    let d = pipe_started(&fx, &App::new("acme-ai-pm"));
    fx.rmtree(&d); // preparing row, folder gone
    fx.write("workspace/pipeline/applications/orphan/application.md", "---\nslug: orphan\n---\n");
    let (issues, _) = check(&fx);
    assert!(any_contains(&issues, "no application folder"));
    assert!(issues.iter().any(|i| i.contains("orphan") && i.contains("no board row")));
}

#[test]
fn test_nudge_and_stale_saved_notes() {
    let fx = pfx();
    // started, staged applied, next_nudge moved into the past, plus an old saved row
    let rows = board_row("acme-ai-pm", "applied", &format!("    applied: {}\n    next_nudge: 2026-01-01\n", today()))
        + &format!("  old-save:\n    stage: saved\n    saved: {}\n", days_ago(40));
    write_board(&fx, &rows);
    write_application(&fx, &App { jd: false, ..App::new("acme-ai-pm") });
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "follow-up due"));
    assert!(notes.iter().any(|n| n.contains("old-save") && n.contains("start it or drop it")));
}

#[test]
fn test_resume_lint_flags_hazards() {
    let fx = pfx();
    let d = pipe_started(&fx, &App::new("acme-ai-pm"));
    fx.write(
        &format!("{d}/resume/resume-v1.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: []\n---\n## My Journey\n- spearheaded X — delivered Y\n- Acme 2022 - 2023\n",
    );
    edit_app(&fx, &d, |afm| row_set(&afm, "resume", "status", "drafting").unwrap());
    let (issues, notes) = check(&fx);
    assert!(any_contains(&issues, "em dash"));
    assert!(any_contains(&issues, "non-canonical heading"));
    assert!(any_contains(&issues, "year-only date range"));
    assert!(any_contains(&notes, "AI-tell"));
}

#[test]
fn test_materials_naming_missing_row_is_issue() {
    let fx = pfx();
    let d = pipe_started(&fx, &App::new("acme-ai-pm"));
    edit_app(&fx, &d, |afm| set_scalar(&afm, "materials", "[deck]", "frontmatter").unwrap());
    let (issues, _) = check(&fx);
    assert!(issues.iter().any(|i| i.contains("materials") && i.contains("deck")));
}

// ------------------------------------------------------------------ TestReadyGates

fn prep_application(fx: &Fx, verification: bool) -> String {
    let d = pipe_started(fx, &App { resume_status: "drafting", ..App::new("acme-ai-pm") });
    fx.write(
        &format!("{d}/resume/resume-v1.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/resume-v1.pdf]\n---\n## Work Experience\n- Cut latency 38% (Jan 2022 - Dec 2023)\n## Skills\n- Python\n",
    );
    fx.write(&format!("{d}/resume/media/resume-v1.pdf"), "pdf");
    let mut jd_map = "---\nstatus: drafting\nlast_updated: 2026-07-10\n---\n## Requirements\n- Python\n".to_string();
    if verification {
        jd_map.push_str("## Verification\n- hard requirements mirrored; hazards clean; tells clean\n");
    }
    fx.write(&format!("{d}/jd-map.md"), &jd_map);
    d
}

const ADD_DECK: (&str, &str) = ("deliverables:", "deliverables:\n  deck:\n    file: deck/deck-v1.md\n    status: drafting");

#[test]
fn test_ready_refused_without_verification() {
    let fx = pfx();
    prep_application(&fx, false);
    fx.refused(&["ready", "acme-ai-pm", "resume"]);
}

#[test]
fn test_ready_refused_without_exports() {
    let fx = pfx();
    let d = prep_application(&fx, true);
    fx.remove(&format!("{d}/resume/media/resume-v1.pdf"));
    fx.refused(&["ready", "acme-ai-pm", "resume"]);
}

#[test]
fn test_deck_material_is_export_gated_via_pack() {
    let fx = pfx();
    let d = prep_application(&fx, true);
    fx.write(&format!("{d}/deck/deck-v1.md"), "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: []\n---\n# Deck spec\n");
    edit_app(&fx, &d, |afm| afm.replacen(ADD_DECK.0, ADD_DECK.1, 1));
    fx.refused(&["ready", "acme-ai-pm", "deck"]);
}

#[test]
fn test_deck_ready_needs_verification_but_skips_ats_lint() {
    let fx = pfx();
    let d = prep_application(&fx, false);
    fx.write(
        &format!("{d}/deck/deck-v1.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/deck-v1.pptx]\n---\n# Deck — narrative arc\n- slide 1 — the hook\n",
    );
    fx.write(&format!("{d}/deck/media/deck-v1.pptx"), "pptx");
    edit_app(&fx, &d, |afm| set_scalar(&afm, "materials", "[deck]", "frontmatter").unwrap().replacen(ADD_DECK.0, ADD_DECK.1, 1));
    fx.refused(&["ready", "acme-ai-pm", "deck"]);
    assert_eq!(get_scalar(&fx.fm(&format!("{d}/deck/deck-v1.md")), "status").as_deref(), Some("drafting"));
    let jm = format!("{d}/jd-map.md");
    fx.write(&jm, &(fx.read(&jm) + "## Verification\n- criteria mapped; deck reviewed\n"));
    fx.ok(&["ready", "acme-ai-pm", "deck"]);
    assert_eq!(row_get(&fx.fm(&format!("{d}/application.md")), "deck", "status").as_deref(), Some("ready"));
}

#[test]
fn test_ready_succeeds_and_applied_records_shipped() {
    // The readiness half; the `pipe stage applied` half (board `shipped`) stays on the Python.
    let fx = pfx();
    let d = prep_application(&fx, true);
    fx.ok(&["ready", "acme-ai-pm", "resume"]);
    assert_eq!(row_get(&fx.fm(&format!("{d}/application.md")), "resume", "status").as_deref(), Some("ready"));
}

// ------------------------------------------------------------------ test_af_pipe_rounds: TestArchive

#[test]
fn test_archived_application_stays_clean_in_doctor() {
    let fx = pfx();
    // started, staged rejected, archived: the folder lives under applications/completed/
    pipe_started(
        &fx,
        &App { stage: "rejected", archived: true, board_extra: "    next_nudge: null\n", ..App::new("acme-ai-pm") },
    );
    let (issues, _) = check(&fx);
    assert_eq!(issues, Vec::<String>::new());
}

// ------------------------------------------------------------------ test_af_pipe_rounds: TestRoundDoctorNotes

fn started_with_round(held: &str, debrief: Option<&str>) -> Fx {
    let fx = pfx();
    let d = pipe_started(&fx, &App { stage: "interviewing", ..App::new("acme-ai-pm") });
    fx.write(&format!("{d}/round-1-jeff/README.md"), &format!("---\nheld_at: {held}\n---\n\n# Round 1\n"));
    if let Some(text) = debrief {
        fx.write(&format!("{d}/round-1-jeff/debrief.md"), text);
    }
    fx
}

#[test]
fn test_held_round_without_debrief_is_a_note() {
    let fx = started_with_round("2026-01-05T10:00:00-07:00", None);
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "has no debrief.md"), "{notes:?}");
}

#[test]
fn test_future_round_without_debrief_is_quiet() {
    let fx = started_with_round("2099-01-05T10:00:00-07:00", None);
    let (_, notes) = check(&fx);
    assert!(!any_contains(&notes, "debrief.md"), "{notes:?}");
}

#[test]
fn test_unpromoted_debrief_is_a_note() {
    let fx = started_with_round(
        "2026-01-05T10:00:00-07:00",
        Some("---\nstatus: ready\ncompleteness: full\npromoted: false\n---\n\n# D\n"),
    );
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "have not received this round's facts"), "{notes:?}");
}

#[test]
fn test_partial_completeness_is_a_note() {
    let fx = started_with_round(
        "2026-01-05T10:00:00-07:00",
        Some("---\nstatus: drafting\ncompleteness: partial - operator has not reported\npromoted: false\n---\n\n# D\n"),
    );
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "completeness is partial"), "{notes:?}");
}

#[test]
fn test_promoted_full_debrief_is_quiet() {
    let fx = started_with_round(
        "2026-01-05T10:00:00-07:00",
        Some("---\nstatus: ready\ncompleteness: full\npromoted: true\n---\n\n# D\n"),
    );
    let (_, notes) = check(&fx);
    assert!(!any_contains(&notes, "debrief.md"), "{notes:?}");
}
