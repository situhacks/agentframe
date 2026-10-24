//! In-scope cases of system/tests/test_af_studio.py: new-project's topology refusal, posts
//! as work folders, draft on a post, and the studio doctor.
//!
//! The `studio` verbs are not ported (R2+), so fixtures write what `af studio new|stage|post`
//! leave behind (common::write_post / write_calendar).

mod common;
use afcore::core::{project_dir, state_doc};
use afcore::doctor;
use common::*;

fn sfx() -> Fx {
    let fx = Fx::with_packs(&["creator"]);
    fx.mkdir("workspace/projects");
    fx
}

fn check(fx: &Fx) -> (Vec<String>, Vec<String>) {
    doctor::check_studio(&fx.ctx()).unwrap()
}

#[test]
fn test_new_project_refuses_studio_topology() {
    let fx = sfx();
    fx.refused(&["new-project", "x", "--domain", "creator"]);
}

#[test]
fn test_posts_resolve_as_work_folders() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "planned", "    date: 2026-10-01\n"));
    let d = write_post(&fx, "meta-read-1", Some("2026-10-01"), false, None);
    assert_eq!(project_dir(&fx.ctx(), "meta-read-1").unwrap(), fx.p(&d));
    assert_eq!(state_doc(&fx.p(&d)), "post.md");
}

#[test]
fn test_draft_script_on_a_post() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "planned", "    date: 2026-10-01\n"));
    let d = write_post(&fx, "meta-read-1", Some("2026-10-01"), false, None);
    fx.ok(&["draft", "meta-read-1", "script", "--file", "script/script-v1.md"]);
    assert!(fx.isfile(&format!("{d}/script/script-v1.md")));
    assert_eq!(row_get(&fx.fm(&format!("{d}/post.md")), "script", "status").as_deref(), Some("drafting"));
}

// ------------------------------------------------------------------ TestStudioDoctor

#[test]
fn test_clean_studio_is_quiet() {
    let fx = sfx();
    write_calendar(
        &fx,
        &(slot_row("meta-read-1", "captured", "") + &slot_row("planned-one", "planned", "    date: 2099-01-01\n")),
    );
    write_post(&fx, "meta-read-1", None, true, None);
    write_post(&fx, "planned-one", Some("2099-01-01"), false, None);
    let (issues, notes) = check(&fx);
    assert_eq!(issues, Vec::<String>::new());
    assert_eq!(notes, Vec::<String>::new());
}

#[test]
fn test_missing_folder_and_orphan_folder_are_issues() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "planned", "    date: 2099-01-01\n"));
    // the post folder was never written (removed), and an orphan has no board row
    fx.write("workspace/studio/posts/orphan/post.md", "---\nslug: orphan\n---\n");
    let (issues, _) = check(&fx);
    assert!(any_contains(&issues, "has no post folder"));
    assert!(issues.iter().any(|i| i.contains("orphan") && i.contains("no board row")));
}

#[test]
fn test_missing_render_behind_cut_is_an_issue() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "cut", ""));
    write_post(&fx, "meta-read-1", None, true, None);
    let (issues, _) = check(&fx);
    assert!(any_contains(&issues, "renders/final.mp4 is missing"));
}

#[test]
fn test_overdue_scheduled_and_capture_due_notes() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "scheduled", "    date: 2026-01-01\n"));
    let d = write_post(&fx, "meta-read-1", None, true, None);
    fx.write(&format!("{d}/renders/final.mp4"), "mp4 bytes");
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "has no receipt"));
    // `af studio post` with a posted_at 30 days back
    let old = format!("{}T09:00:00-07:00", days_ago(30));
    write_calendar(
        &fx,
        &slot_row(
            "meta-read-1",
            "posted",
            &format!("    date: 2026-01-01\n    link: https://www.tiktok.com/@b/video/1\n    posted: {}\n", days_ago(30)),
        ),
    );
    write_post(&fx, "meta-read-1", None, true, Some((&old, "https://www.tiktok.com/@b/video/1")));
    let (_, notes) = check(&fx);
    assert!(any_contains(&notes, "metrics not captured"));
}

#[test]
fn test_doctor_studio_scope_runs_clean() {
    let fx = sfx();
    write_calendar(&fx, &slot_row("meta-read-1", "captured", ""));
    write_post(&fx, "meta-read-1", None, true, None);
    let out = fx.ok(&["doctor", "studio"]);
    assert!(out.contains("+ studio"));
}
