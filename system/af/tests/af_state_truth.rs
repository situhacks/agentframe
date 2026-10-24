//! The repo-root path guard, published-and-
//! not-ready drift, empty-head notes, and readying a nested manifest ingredient.

mod common;
use afcore::core::safe_project_rel;
use afcore::doctor;
use common::*;

fn write_project(fx: &Fx, rows: &str) {
    fx.write(
        "workspace/projects/campaign/project.md",
        &format!(
            "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\nlast_activity: 2026-08-27T10:00:00-07:00\ndeliverables:\n{rows}---\n"
        ),
    );
}

fn tfx() -> Fx {
    Fx::with_packs(&["marketing"])
}

fn cdir(fx: &Fx) -> String {
    fx.p("workspace/projects/campaign")
}

fn pfm(fx: &Fx) -> String {
    fx.fm("workspace/projects/campaign/project.md")
}

// ------------------------------------------------------------------ RepoRootPathGuard

fn guard_fx() -> Fx {
    let fx = tfx();
    write_project(&fx, "  brief:\n    status: not_started\n    file: brief/brief-v1.md\n    last_updated: 2026-08-27\n");
    fx
}

#[test]
fn test_draft_refuses_repo_root_relative() {
    let fx = guard_fx();
    let err = fx.refused(&["draft", "campaign", "brief", "--file", "workspace/projects/campaign/brief/brief-v1.md"]);
    assert!(err.contains("repo-root-relative"));
}

#[test]
fn test_adopt_refuses_repo_root_relative() {
    let fx = guard_fx();
    let err = fx.refused(&["adopt", "campaign", "brief", "--file", "workspace/projects/campaign/brief/brief-v1.md"]);
    assert!(err.contains("PROJECT-relative"));
}

#[test]
fn test_ordinary_project_relative_path_still_accepted() {
    let fx = guard_fx();
    let (rel, target) = safe_project_rel(&cdir(&fx), Some("brief/brief-v1.md")).unwrap();
    assert_eq!(rel, "brief/brief-v1.md");
    let tail = std::path::Path::new("campaign").join("brief").join("brief-v1.md");
    assert!(target.ends_with(&*tail.to_string_lossy()));
}

#[test]
fn test_a_folder_literally_named_workspace_is_still_refused() {
    // Deliberate: the guard is a blunt prefix rule.
    let fx = guard_fx();
    assert!(safe_project_rel(&cdir(&fx), Some("workspace/notes-v1.md")).is_err());
}

// ------------------------------------------------------------------ PublishDrift

fn drift(fx: &Fx) -> Vec<String> {
    doctor::state_truth_issues(&fx.ctx(), &cdir(fx), &pfm(fx)).unwrap()
}

#[test]
fn test_url_below_ready_is_an_issue() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: drafting\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    substack_url: \"https://example.com/p/x\"\n",
    );
    let issues = drift(&fx);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(issues[0].contains("substack_url"));
    assert!(issues[0].contains("published and not"));
}

#[test]
fn test_draft_id_below_ready_is_an_issue() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: drafting\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    substack_draft: 210153401\n",
    );
    assert_eq!(drift(&fx).len(), 1);
}

#[test]
fn test_published_row_with_url_is_clean() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: published\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    substack_url: \"https://example.com/p/x\"\n",
    );
    assert_eq!(drift(&fx), Vec::<String>::new());
}

#[test]
fn test_ready_row_with_url_is_clean() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: ready\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    posted_url: \"https://example.com/p/x\"\n",
    );
    assert_eq!(drift(&fx), Vec::<String>::new());
}

#[test]
fn test_null_url_does_not_fire() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: drafting\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    substack_url: null\n",
    );
    assert_eq!(drift(&fx), Vec::<String>::new());
}

#[test]
fn test_matches_by_shape_not_by_vendor() {
    let fx = tfx();
    write_project(
        &fx,
        "  essay:\n    status: drafting\n    file: essay/essay-v1.md\n    last_updated: 2026-08-27\n    mastodon_url: \"https://example.social/@x/1\"\n",
    );
    assert_eq!(drift(&fx).len(), 1);
}

// ------------------------------------------------------------------ EmptyHeadNotes

fn head_fx(body: &str) -> Fx {
    let fx = tfx();
    write_project(&fx, "  brief:\n    status: drafting\n    file: brief/brief-v1.md\n    last_updated: 2026-08-27\n");
    fx.write(
        "workspace/projects/campaign/brief/brief-v1.md",
        &format!("---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n{body}"),
    );
    fx
}

#[test]
fn test_empty_body_is_a_note() {
    let fx = head_fx("\n");
    let notes = doctor::empty_head_notes(&fx.ctx(), &cdir(&fx)).unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("empty head"));
}

#[test]
fn test_written_body_is_quiet() {
    let fx = head_fx("\n# Brief\n\nReal content.\n");
    assert_eq!(doctor::empty_head_notes(&fx.ctx(), &cdir(&fx)).unwrap(), Vec::<String>::new());
}

#[test]
fn test_note_never_becomes_an_issue() {
    let fx = head_fx("\n");
    assert_eq!(drift(&fx), Vec::<String>::new());
}

// ------------------------------------------------------------------ ReadyArtifact

fn artifact_fx() -> Fx {
    let fx = tfx();
    let c = "workspace/projects/campaign";
    let post = "workspace/projects/campaign/posts/post-1.5-vitamix";
    fx.write(
        &format!("{c}/project.md"),
        "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\nlast_activity: 2026-08-27T10:00:00-07:00\npost_manifest:\n  ingredients: [body-copy]\ndeliverables:\n  post-1-5:\n    status: drafting\n    file: posts/post-1.5-vitamix/post-FINAL.md\n    last_updated: 2026-08-27\n---\n",
    );
    fx.write(&format!("{post}/post-FINAL.md"), "---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n\n# Post\n");
    fx.write(&format!("{post}/body-copy-v2.md"), "---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n\nBody text.\n");
    fx
}

#[test]
fn test_artifact_flag_readies_the_ingredient_head() {
    let fx = artifact_fx();
    fx.ok(&["ready", "campaign", "post-1-5", "--artifact", "body-copy"]);
    let head = fx.fm("workspace/projects/campaign/posts/post-1.5-vitamix/body-copy-v2.md");
    assert_eq!(get_scalar(&head, "status").as_deref(), Some("ready"));
}

#[test]
fn test_readying_the_last_ingredient_promotes_the_parent() {
    let fx = artifact_fx();
    fx.ok(&["ready", "campaign", "post-1-5", "--artifact", "body-copy"]);
    assert_eq!(row_get(&pfm(&fx), "post-1-5", "status").as_deref(), Some("ready"));
}

#[test]
fn test_blocked_parent_names_the_exact_next_command() {
    let fx = artifact_fx();
    let msg = fx.refused(&["ready", "campaign", "post-1-5"]);
    assert!(msg.contains("post-1-5 --artifact body-copy")); // the tracker slug, not the folder name
    assert!(msg.contains("Still drafting"));
}
