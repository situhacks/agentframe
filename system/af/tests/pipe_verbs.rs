//! The `af pipe` verb cases (save and start, the stage machine, close-search, and the
//! `pipe stage applied` half of the ready gates), driven through the binary on a temp root carrying the real careers pack.
//! The doctor-only and ready-gate cases are in af_pipe.rs.

mod common;
use afcore::fm::{join_fm, pipe_rows, row_set, set_scalar, split_fm};
use afcore::pipe;
use common::*;

fn pfx() -> Fx {
    let fx = Fx::with_packs(&["careers"]);
    fx.mkdir("workspace/projects");
    fx
}

const BOARD: &str = "workspace/pipeline/pipeline.md";

fn save(fx: &Fx, slug: &str) -> String {
    fx.ok(&[
        "pipe", "save", "--company", "Acme", "--role", "AI PM", "--url", "https://boards.greenhouse.io/acme/jobs/1",
        "--ats", "greenhouse", "--source", "manual", "--slug", slug,
    ]);
    slug.to_string()
}

fn start(fx: &Fx, slug: &str) {
    fx.ok(&["pipe", "start", slug]);
}

fn stage(fx: &Fx, slug: &str, st: &str) {
    fx.ok(&["pipe", "stage", slug, st]);
}

fn board_fm(fx: &Fx) -> String {
    fx.fm(BOARD)
}

fn jd_cache(slug: &str) -> String {
    format!("workspace/pipeline/scout/jd-cache/{slug}.jd.md")
}

fn adir(fx: &Fx, slug: &str) -> String {
    pipe::app_dir(&fx.ctx(), slug)
}

fn edit_app(fx: &Fx, slug: &str, f: impl Fn(String) -> String) {
    let ap = format!("{}/application.md", app_dir_rel(slug, false));
    let (afm, abody) = split_fm(&fx.read(&ap), "application.md").unwrap();
    fx.write(&ap, &join_fm(&f(afm), &abody));
}

// ------------------------------------------------------------------ TestSaveAndStart

#[test]
fn test_save_creates_board_row() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    let fm = board_fm(&fx);
    assert_eq!(pipe_rows(&fm), vec![slug.clone()]);
    assert_eq!(row_get(&fm, &slug, "stage").as_deref(), Some("saved"));
    assert_eq!(row_get(&fm, &slug, "saved"), Some(today()));
    assert_eq!(row_get(&fm, &slug, "company").as_deref(), Some("Acme"));
}

#[test]
fn test_duplicate_save_refused() {
    let fx = pfx();
    save(&fx, "acme-ai-pm");
    let err = fx.refused(&["pipe", "save", "--company", "Acme", "--role", "AI PM", "--url", "u", "--slug", "acme-ai-pm"]);
    assert!(err.contains("already exists"), "{err}");
}

#[test]
fn test_start_scaffolds_and_moves_jd_cache() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "verbatim JD text");
    start(&fx, &slug);
    let d = app_dir_rel(&slug, false);
    assert_eq!(adir(&fx, &slug), fx.p(&d));
    assert!(fx.isfile(&format!("{d}/application.md")));
    assert_eq!(fx.read(&format!("{d}/jd.md")), "verbatim JD text");
    assert!(!fx.exists(&jd_cache(&slug)));
    assert_eq!(row_get(&board_fm(&fx), &slug, "stage").as_deref(), Some("preparing"));
    let afm = fx.fm(&format!("{d}/application.md"));
    assert_eq!(get_scalar(&afm, "company").as_deref(), Some("Acme"));
    assert_eq!(get_scalar(&afm, "domain").as_deref(), Some("careers"));
    assert_eq!(row_get(&afm, "resume", "status").as_deref(), Some("not_started"));
}

#[test]
fn test_start_requires_saved_stage() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    start(&fx, &slug);
    fx.refused(&["pipe", "start", &slug]);
}

// ------------------------------------------------------------------ TestStageMachine

#[test]
fn test_applied_stamps_dates_and_warns_on_unready_resume() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    start(&fx, &slug);
    stage(&fx, &slug, "applied");
    let fm = board_fm(&fx);
    assert_eq!(row_get(&fm, &slug, "applied"), Some(today()));
    assert_eq!(row_get(&fm, &slug, "next_nudge"), Some(days_ahead(pipe::PIPE_NUDGE_DAYS)));
    assert_eq!(row_get(&fm, &slug, "shipped"), None); // resume never ready
}

#[test]
fn test_illegal_transition_refused() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.refused(&["pipe", "stage", &slug, "interviewing"]);
}

#[test]
fn test_terminal_clears_nudge_and_ghosted_can_return() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    start(&fx, &slug);
    stage(&fx, &slug, "applied");
    stage(&fx, &slug, "ghosted");
    assert_eq!(row_get(&board_fm(&fx), &slug, "next_nudge").as_deref(), Some("null"));
    stage(&fx, &slug, "interviewing");
    assert_eq!(row_get(&board_fm(&fx), &slug, "stage").as_deref(), Some("interviewing"));
}

/// A started application whose primary material is a ready deck at v2.
fn started_with_ready_deck(fx: &Fx) -> String {
    let slug = save(fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(fx, &slug);
    let d = app_dir_rel(&slug, false);
    fx.write(
        &format!("{d}/deck/deck-v2.md"),
        "---\nstatus: ready\nlast_updated: 2026-07-10\nexports: [media/deck-v2.pptx]\n---\n# Deck\n",
    );
    fx.write(&format!("{d}/deck/media/deck-v2.pptx"), "pptx");
    edit_app(fx, &slug, |afm| {
        set_scalar(&afm, "materials", "[deck]", "frontmatter")
            .unwrap()
            .replace("deliverables:", "deliverables:\n  deck:\n    file: deck/deck-v2.md\n    status: ready")
    });
    slug
}

#[test]
fn test_shipped_derives_from_primary_material() {
    let fx = pfx();
    let slug = started_with_ready_deck(&fx);
    stage(&fx, &slug, "applied");
    assert_eq!(row_get(&board_fm(&fx), &slug, "shipped").as_deref(), Some("v2"));
}

#[test]
fn test_inbound_interviewing_stamps_shipped_from_the_ready_head() {
    // BB-2026-09-06-01: the recruiter path ships material too, so it stamps the same way.
    let fx = pfx();
    let slug = started_with_ready_deck(&fx);
    stage(&fx, &slug, "interviewing");
    let fm = board_fm(&fx);
    assert_eq!(row_get(&fm, &slug, "stage").as_deref(), Some("interviewing"));
    assert_eq!(row_get(&fm, &slug, "shipped").as_deref(), Some("v2"));
}

fn to_offer(fx: &Fx) -> String {
    let slug = save(fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(fx, &slug);
    for st in ["applied", "interviewing", "offer"] {
        stage(fx, &slug, st);
    }
    slug
}

#[test]
fn test_offer_is_answered_by_the_terminal_pair() {
    let fx = pfx();
    let slug = to_offer(&fx);
    fx.ok(&["pipe", "stage", &slug, "accepted", "--offered-role", "Forward Deployed Engineer"]);
    let fm = board_fm(&fx);
    assert_eq!(row_get(&fm, &slug, "stage").as_deref(), Some("accepted"));
    assert_eq!(row_get(&fm, &slug, "offered_role").as_deref(), Some("Forward Deployed Engineer"));
    fx.refused(&["pipe", "stage", &slug, "declined"]); // accepted is terminal
}

#[test]
fn test_offer_cannot_be_walked_away_from_without_an_answer() {
    let fx = pfx();
    let slug = to_offer(&fx);
    fx.refused(&["pipe", "stage", &slug, "dropped"]);
    stage(&fx, &slug, "declined");
    assert_eq!(row_get(&board_fm(&fx), &slug, "stage").as_deref(), Some("declined"));
}

#[test]
fn test_offered_role_only_at_the_offer_end() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(&fx, &slug);
    fx.refused(&["pipe", "stage", &slug, "applied", "--offered-role", "X"]);
}

#[test]
fn test_accepted_archives_but_an_open_offer_does_not() {
    let fx = pfx();
    let slug = to_offer(&fx);
    fx.refused(&["pipe", "archive", &slug]); // live until answered
    stage(&fx, &slug, "accepted");
    fx.ok(&["pipe", "archive", &slug]);
    assert!(pipe::app_is_archived(&fx.ctx(), &slug));
}

#[test]
fn test_inbound_interviewing_with_unready_resume_nudges_instead() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(&fx, &slug);
    stage(&fx, &slug, "interviewing");
    assert_eq!(row_get(&board_fm(&fx), &slug, "shipped"), None);
    let activity = fx.read(&format!("{}/activity.md", app_dir_rel(&slug, false)));
    assert!(activity.contains("went to the recruiter unverified"), "{activity}");
    assert!(activity.contains("submitted_by"), "{activity}");
}

// ------------------------------------------------------------------ TestCloseSearch

#[test]
fn test_drops_live_rows_and_stamps_the_board() {
    let fx = pfx();
    let live = save(&fx, "live-one");
    fx.write(&jd_cache(&live), "jd");
    start(&fx, &live);
    let saved = save(&fx, "saved-only");
    let done = save(&fx, "done-one");
    fx.write(&jd_cache(&done), "jd");
    start(&fx, &done);
    stage(&fx, &done, "dropped");
    let out = fx.ok(&["pipe", "close-search", "--reason", "Banyan accepted"]);
    let fm = board_fm(&fx);
    assert_eq!(get_scalar(&fm, "search_status").as_deref(), Some("closed"));
    assert_eq!(get_scalar(&fm, "search_closed_at"), Some(today()));
    assert_eq!(get_scalar(&fm, "search_closed_reason").as_deref(), Some("Banyan accepted"));
    assert_eq!(row_get(&fm, &live, "stage").as_deref(), Some("dropped"));
    assert_eq!(row_get(&fm, &saved, "stage").as_deref(), Some("dropped"));
    assert!(out.contains("live-one (preparing)"), "{out}");
    let activity = fx.read(&format!("{}/activity.md", app_dir_rel(&live, false)));
    assert!(activity.contains("search closed: Banyan accepted"), "{activity}");
}

#[test]
fn test_closing_twice_is_refused_and_save_reopens() {
    let fx = pfx();
    save(&fx, "a");
    fx.ok(&["pipe", "close-search", "--reason", "done"]);
    fx.refused(&["pipe", "close-search", "--reason", "again"]);
    save(&fx, "b");
    assert_eq!(get_scalar(&board_fm(&fx), "search_status").as_deref(), Some("open"));
    fx.ok(&["pipe", "close-search", "--reason", "done again"]);
    assert_eq!(get_scalar(&board_fm(&fx), "search_status").as_deref(), Some("closed"));
}

#[test]
fn test_reason_required() {
    let fx = pfx();
    save(&fx, "a");
    fx.refused(&["pipe", "close-search", "--reason", "  "]);
}

// ------------------------------------------------------------------ TestPipelineDoctor, through the verbs

#[test]
fn test_clean_pipeline_is_quiet_after_save_and_start() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(&fx, &slug);
    let (issues, notes) = afcore::doctor::check_pipeline(&fx.ctx()).unwrap();
    assert_eq!(issues, Vec::<String>::new());
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn test_nudge_and_stale_saved_notes_through_the_verbs() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    start(&fx, &slug);
    stage(&fx, &slug, "applied");
    let ctx = fx.ctx();
    let (fm, body) = pipe::load_board(&ctx).unwrap();
    let fm = row_set(&fm, &slug, "next_nudge", "2026-01-01").unwrap();
    let fm = pipe::pipe_row_add(&fm, "old-save", &[("stage", Some("saved".into())), ("saved", Some(days_ago(40)))]).unwrap();
    pipe::write_board(&ctx, &fm, &body).unwrap();
    let (_, notes) = afcore::doctor::check_pipeline(&ctx).unwrap();
    assert!(any_contains(&notes, "follow-up due"), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("old-save") && n.contains("start it or drop it")), "{notes:?}");
}

// ------------------------------------------------------------------ TestReadyGates: the stage half

#[test]
fn test_ready_succeeds_and_applied_records_shipped() {
    let fx = pfx();
    let slug = save(&fx, "acme-ai-pm");
    fx.write(&jd_cache(&slug), "jd");
    start(&fx, &slug);
    let d = app_dir_rel(&slug, false);
    fx.write(
        &format!("{d}/resume/resume-v1.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/resume-v1.pdf]\n---\n## Work Experience\n- Cut latency 38% (Jan 2022 - Dec 2023)\n## Skills\n- Python\n",
    );
    fx.write(&format!("{d}/resume/media/resume-v1.pdf"), "pdf");
    fx.write(
        &format!("{d}/jd-map.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-10\n---\n## Requirements\n- Python\n## Verification\n- hard requirements mirrored; hazards clean; tells clean\n",
    );
    edit_app(&fx, &slug, |afm| row_set(&afm, "resume", "status", "drafting").unwrap());
    fx.ok(&["ready", &slug, "resume"]);
    assert_eq!(row_get(&fx.fm(&format!("{d}/application.md")), "resume", "status").as_deref(), Some("ready"));
    stage(&fx, &slug, "applied");
    assert_eq!(row_get(&board_fm(&fx), &slug, "shipped").as_deref(), Some("v1"));
}

// ------------------------------------------------------------------ run log

#[test]
fn test_pipe_verbs_log_their_subcommand() {
    let fx = pfx();
    save(&fx, "a");
    fx.refused(&["pipe", "start", "ghost"]);
    fx.ok(&["pipe", "board"]);
    let rows = fx.run_log_rows();
    let verbs: Vec<&str> = rows.iter().map(|r| r["verb"].as_str().unwrap()).collect();
    assert_eq!(verbs, ["pipe save", "pipe start", "pipe board"]);
    assert_eq!(rows[1]["exit"], 1);
    assert_eq!(rows[1]["refusal"], "no board row 'ghost' — 'af pipe save' it first");
}
