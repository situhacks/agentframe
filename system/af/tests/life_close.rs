//! `af close <slug>`: project closeout mechanics and their refusals.

mod common;
mod life_fx;
use afcore::fm::{join_fm, row_add, set_scalar, split_fm};
use common::*;
use life_fx::*;

const PM: &str = "workspace/projects/acme/project.md";

fn project(rows: &[(&str, &str)]) -> Fx {
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.mkdir("workspace/projects");
    fx.ok(&["new-project", "acme"]);
    let (mut cfm, body) = split_fm(&fx.read(PM), "project.md").unwrap();
    for (slug, status) in rows {
        let file = format!("{slug}/{slug}-v1.md");
        cfm = row_add(&cfm, slug, &[("file", file.clone()), ("status", status.to_string())]).unwrap();
        fx.write(&format!("workspace/projects/acme/{file}"), &format!("---\nstatus: {status}\nlast_updated: 2026-10-01\n---\nbody\n"));
    }
    fx.write(PM, &join_fm(&cfm, &body));
    fx
}

#[test]
fn close_moves_stamps_logs_and_prints_the_harvest_checklist() {
    let fx = project(&[("essay", "published"), ("post", "published"), ("notes", "ready")]);
    let act = "workspace/projects/acme/activity.md";
    let prior = fx.read(act);
    fx.write(act, &format!("{prior}2026-10-20 10:00 — voice_harvest: essay mined; voice-harvest ran on essay\n"));
    let out = fx.ok(&["close", "acme"]);

    assert!(!fx.exists("workspace/projects/acme"));
    let done = "workspace/projects/completed/acme";
    let cfm = fx.fm(&format!("{done}/project.md"));
    assert_eq!(get_scalar(&cfm, "status").as_deref(), Some("complete"));
    assert_eq!(get_scalar(&cfm, "completed_at"), Some(today()));
    assert!(cfm.contains("status: complete\ncompleted_at: "), "{cfm}");

    let activity = fx.read(&format!("{done}/activity.md"));
    assert!(activity.starts_with(&prior), "earlier lines untouched");
    assert!(activity.trim_end().ends_with("— project_closed: acme; folder moved to completed/"), "{activity}");
    let log = fx.read(&format!("workspace/life/log/{}.md", month()));
    assert!(log.ends_with(" · project_closed: acme\n"), "{log}");

    assert!(out.contains("af close: acme -> complete"));
    assert!(out.contains("[ ] essay (essay/essay-v1.md): deliverable-harvest\r\n") || out.contains("[ ] essay (essay/essay-v1.md): deliverable-harvest\n"), "{out}");
    assert!(out.contains("[ ] post (post/post-v1.md): deliverable-harvest, voice-harvest"));
    assert!(!out.contains("notes ("), "only published heads");
    assert!(out.contains("career-harvest"));

    let issues = afcore::doctor::check_project(&fx.ctx(), &fx.p(done)).unwrap();
    assert!(!issues.iter().any(|i| i.contains("completed_at") || i.contains("lifecycle")), "{issues:?}");
}

#[test]
fn close_refuses_a_project_that_is_not_active_unless_forced() {
    let fx = project(&[]);
    let (cfm, body) = split_fm(&fx.read(PM), "project.md").unwrap();
    fx.write(PM, &join_fm(&set_scalar(&cfm, "status", "cancelled", "project.md").unwrap(), &body));
    let err = fx.refused(&["close", "acme"]);
    assert!(err.contains("'cancelled', not active"), "{err}");
    assert!(fx.exists(PM));
    fx.ok(&["close", "acme", "--force"]);
    assert!(fx.exists("workspace/projects/completed/acme/project.md"));
}

#[test]
fn close_refuses_drafting_deliverables_unless_forced() {
    let fx = project(&[("essay", "drafting"), ("post", "ready")]);
    let err = fx.refused(&["close", "acme"]);
    assert!(err.contains("drafting deliverables: essay"), "{err}");
    assert!(fx.exists(PM));
    assert!(!fx.exists(&format!("workspace/life/log/{}.md", month())));
    fx.ok(&["close", "acme", "--force"]);
}

#[test]
fn close_refuses_unknown_and_already_closed_projects() {
    let fx = project(&[]);
    fx.refused(&["close", "nope"]);
    fx.refused(&["close", "../acme"]);
    fx.ok(&["close", "acme"]);
    let err = fx.refused(&["close", "acme"]);
    assert!(err.contains("already closed"), "{err}");
}
