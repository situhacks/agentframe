//! The `af studio` verb cases of system/tests/test_af_studio.py (TestNew, TestStateMachine,
//! TestTopologyPack, and the verb-driven TestStudioDoctor fixtures), driven through the
//! binary on a temp root carrying the real creator pack. af_studio.rs keeps the cases R1
//! ported against fixture-written folders.

mod common;
use afcore::core::{project_dir, state_doc};
use afcore::doctor;
use afcore::pipe;
use afcore::studio;
use common::*;

struct S {
    fx: Fx,
    clip: String,
}

fn sfx() -> S {
    let fx = Fx::with_packs(&["creator"]);
    fx.mkdir("workspace/projects");
    let clip = fx.write("inbox/take1.mov", "fake video bytes");
    S { fx, clip }
}

const CAL: &str = "workspace/studio/calendar.md";

fn post_rel(slug: &str) -> String {
    format!("workspace/studio/posts/{slug}")
}

impl S {
    fn new_dated(&self, slug: &str, date: &str) -> String {
        self.fx.ok(&["studio", "new", slug, "--date", date]);
        slug.to_string()
    }
    fn new_captured(&self, slug: &str) -> String {
        self.fx.ok(&["studio", "new", slug, "--from", &self.clip]);
        slug.to_string()
    }
    fn stage(&self, slug: &str, state: &str) {
        self.fx.ok(&["studio", "stage", slug, state]);
    }
    fn post(&self, slug: &str, posted_at: Option<&str>) {
        let mut args = vec!["studio", "post", slug, "--url", "https://www.tiktok.com/@b/video/1"];
        if let Some(p) = posted_at {
            args.extend(["--posted-at", p]);
        }
        self.fx.ok(&args);
    }
    fn render(&self, slug: &str) {
        self.fx.write(&format!("{}/renders/final.mp4", post_rel(slug)), "mp4 bytes");
    }
    fn cal_fm(&self) -> String {
        self.fx.fm(CAL)
    }
    fn post_fm(&self, slug: &str) -> String {
        self.fx.fm(&format!("{}/post.md", post_rel(slug)))
    }
    fn state(&self, slug: &str) -> Option<String> {
        studio::slot_get(&self.cal_fm(), slug, "state")
    }
}

// ------------------------------------------------------------------ TestNew

#[test]
fn test_planned_needs_a_date() {
    let s = sfx();
    s.fx.refused(&["studio", "new", "meta-read-1"]);
}

#[test]
fn test_planned_creates_row_folder_and_records() {
    let s = sfx();
    s.fx.ok(&["studio", "new", "meta-read-1", "--date", "2026-10-01", "--series", "patch-notes"]);
    let slug = "meta-read-1";
    let fm = s.cal_fm();
    assert_eq!(studio::slot_rows(&fm), vec![slug.to_string()]);
    assert_eq!(studio::slot_get(&fm, slug, "state").as_deref(), Some("planned"));
    assert_eq!(studio::slot_get(&fm, slug, "date").as_deref(), Some("2026-10-01"));
    assert_eq!(studio::slot_get(&fm, slug, "series").as_deref(), Some("patch-notes"));
    let pfm = s.post_fm(slug);
    assert_eq!(get_scalar(&pfm, "domain").as_deref(), Some("creator"));
    assert_eq!(get_scalar(&pfm, "slug").as_deref(), Some(slug));
    assert_eq!(get_scalar(&pfm, "platform").as_deref(), Some("tiktok"));
    assert_eq!(get_scalar(&pfm, "date").as_deref(), Some("2026-10-01"));
    let d = post_rel(slug);
    assert!(s.fx.isfile(&format!("{d}/edit.md")));
    assert!(s.fx.path(&format!("{d}/video")).is_dir());
    assert!(s.fx.path(&format!("{d}/renders")).is_dir());
}

#[test]
fn test_captured_from_a_file() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    let fm = s.cal_fm();
    assert_eq!(studio::slot_get(&fm, &slug, "state").as_deref(), Some("captured"));
    assert_eq!(studio::slot_get(&fm, &slug, "date"), None);
    let src = format!("{}/video/source.mov", post_rel(&slug));
    assert!(s.fx.isfile(&src));
    assert_eq!(s.fx.read(&src), "fake video bytes");
    assert_eq!(get_scalar(&s.post_fm(&slug), "source_file").as_deref(), Some("video/source.mov"));
}

#[test]
fn test_duplicate_and_bad_date_refused() {
    let s = sfx();
    s.new_dated("meta-read-1", "2026-10-01");
    s.fx.refused(&["studio", "new", "meta-read-1", "--date", "2026-10-02"]);
    s.fx.refused(&["studio", "new", "other", "--date", "Oct 1"]);
}

#[test]
fn test_posts_resolve_as_work_folders_after_new() {
    let s = sfx();
    let slug = s.new_dated("meta-read-1", "2026-10-01");
    let ctx = s.fx.ctx();
    assert_eq!(project_dir(&ctx, &slug).unwrap(), studio::post_dir(&ctx, &slug));
    assert_eq!(state_doc(&studio::post_dir(&ctx, &slug)), "post.md");
}

#[test]
fn test_draft_script_on_a_post_after_new() {
    let s = sfx();
    let slug = s.new_dated("meta-read-1", "2026-10-01");
    s.fx.ok(&["draft", &slug, "script", "--file", "script/script-v1.md"]);
    assert!(s.fx.isfile(&format!("{}/script/script-v1.md", post_rel(&slug))));
    assert_eq!(row_get(&s.post_fm(&slug), "script", "status").as_deref(), Some("drafting"));
}

// ------------------------------------------------------------------ TestStateMachine

#[test]
fn test_captured_requires_a_source() {
    let s = sfx();
    let slug = s.new_dated("meta-read-1", "2026-10-01");
    s.fx.refused(&["studio", "stage", &slug, "captured"]);
    s.fx.write(&format!("{}/video/source.mp4", post_rel(&slug)), "v");
    s.stage(&slug, "captured");
    assert_eq!(s.state(&slug).as_deref(), Some("captured"));
}

#[test]
fn test_cut_and_scheduled_require_a_render() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.fx.refused(&["studio", "stage", &slug, "cut"]);
    s.render(&slug);
    s.stage(&slug, "cut");
    s.stage(&slug, "scheduled");
    assert_eq!(s.state(&slug).as_deref(), Some("scheduled"));
}

#[test]
fn test_illegal_transition_refused() {
    let s = sfx();
    let slug = s.new_dated("meta-read-1", "2026-10-01");
    s.fx.refused(&["studio", "stage", &slug, "scheduled"]);
}

#[test]
fn test_stage_posted_is_refused_in_favour_of_post() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.render(&slug);
    s.stage(&slug, "cut");
    s.fx.refused(&["studio", "stage", &slug, "posted"]);
}

#[test]
fn test_post_requires_a_render_state() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.fx.refused(&["studio", "post", &slug, "--url", "https://www.tiktok.com/@b/video/1"]);
}

#[test]
fn test_post_writes_receipt_on_board_and_record() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.render(&slug);
    s.stage(&slug, "cut");
    s.fx.ok(&[
        "studio", "post", &slug, "--url", "https://www.tiktok.com/@b/video/42", "--posted-at", "2026-09-08T10:00:00-07:00",
    ]);
    let fm = s.cal_fm();
    assert_eq!(studio::slot_get(&fm, &slug, "state").as_deref(), Some("posted"));
    assert_eq!(studio::slot_get(&fm, &slug, "link").as_deref(), Some("https://www.tiktok.com/@b/video/42"));
    assert_eq!(studio::slot_get(&fm, &slug, "posted").as_deref(), Some("2026-09-08"));
    let pfm = s.post_fm(&slug);
    assert_eq!(get_scalar(&pfm, "posted_at").as_deref(), Some("2026-09-08T10:00:00-07:00"));
    assert_eq!(get_scalar(&pfm, "url").as_deref(), Some("https://www.tiktok.com/@b/video/42"));
}

#[test]
fn test_scheduled_can_return_to_cut() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.render(&slug);
    s.stage(&slug, "cut");
    s.stage(&slug, "scheduled");
    s.stage(&slug, "cut");
    assert_eq!(s.state(&slug).as_deref(), Some("cut"));
}

// ------------------------------------------------------------------ TestStudioDoctor, through the verbs

fn check(s: &S) -> (Vec<String>, Vec<String>) {
    doctor::check_studio(&s.fx.ctx()).unwrap()
}

#[test]
fn test_clean_studio_is_quiet_after_new() {
    let s = sfx();
    s.new_captured("meta-read-1");
    s.new_dated("planned-one", "2099-01-01");
    let (issues, notes) = check(&s);
    assert_eq!(issues, Vec::<String>::new());
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn test_missing_folder_and_orphan_folder_are_issues_after_new() {
    let s = sfx();
    let slug = s.new_dated("meta-read-1", "2099-01-01");
    s.fx.rmtree(&post_rel(&slug));
    s.fx.write("workspace/studio/posts/orphan/post.md", "---\nslug: orphan\n---\n");
    let (issues, _) = check(&s);
    assert!(any_contains(&issues, "has no post folder"));
    assert!(issues.iter().any(|i| i.contains("orphan") && i.contains("no board row")));
}

#[test]
fn test_missing_render_behind_cut_is_an_issue_after_stage() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.render(&slug);
    s.stage(&slug, "cut");
    s.fx.remove(&format!("{}/renders/final.mp4", post_rel(&slug)));
    let (issues, _) = check(&s);
    assert!(any_contains(&issues, "renders/final.mp4 is missing"));
}

#[test]
fn test_overdue_scheduled_and_capture_due_notes_through_the_verbs() {
    let s = sfx();
    let slug = s.new_captured("meta-read-1");
    s.render(&slug);
    s.stage(&slug, "cut");
    s.stage(&slug, "scheduled");
    let ctx = s.fx.ctx();
    let (fm, body) = studio::load_calendar(&ctx).unwrap();
    studio::write_calendar(&ctx, &studio::slot_set(&fm, &slug, "date", "2026-01-01").unwrap(), &body).unwrap();
    let (_, notes) = check(&s);
    assert!(any_contains(&notes, "has no receipt"), "{notes:?}");
    let old = format!("{}T09:00:00-07:00", days_ago(30));
    s.post(&slug, Some(&old));
    let (_, notes) = check(&s);
    assert!(any_contains(&notes, "metrics not captured"), "{notes:?}");
}

#[test]
fn test_doctor_studio_scope_runs_clean_after_new() {
    let s = sfx();
    s.new_captured("meta-read-1");
    let out = s.fx.ok(&["doctor", "studio"]);
    assert!(out.contains("+ studio"), "{out}");
}

// ------------------------------------------------------------------ TestTopologyPack

#[test]
fn test_singleton_is_enforced() {
    let s = sfx();
    s.fx.copy_pack("creator", "creator-two");
    let err = pipe::topology_pack(&s.fx.ctx(), "studio").unwrap_err();
    assert!(err.is_refusal());
    assert!(err.to_string().contains("multiple studio-topology packs (creator, creator-two)"), "{err}");
}

#[test]
fn test_missing_topology_dies() {
    let s = sfx();
    let err = pipe::topology_pack(&s.fx.ctx(), "pipeline").unwrap_err();
    assert!(err.is_refusal());
}

#[test]
fn test_pipe_pack_alias_resolves_careers() {
    let s = sfx();
    s.fx.copy_pack("careers", "careers");
    assert_eq!(pipe::pipe_pack(&s.fx.ctx()).unwrap().0, "careers");
}
