//! Port of system/tests/test_af.py: new-project, project state contract, activity notes,
//! marketing receipts, ready/version/draft/adopt/publish, binary heads, the parser, and a
//! full lifecycle smoke run.

mod common;
use afcore::core::{version_target, PROJECT_SCHEMA_VERSION};
use afcore::doctor;
use afcore::fm::{all_rows, has_field, join_fm, row_set, split_fm};
use afcore::packs::marketing;
use common::*;

fn make_project(fx: &Fx, slug: &str, status: &str, created_at: &str, last_activity: &str) -> String {
    let rel = format!("workspace/projects/{slug}");
    fx.mkdir(&format!("{rel}/knowledge"));
    fx.write(
        &format!("{rel}/project.md"),
        &format!(
            "---\nname: {slug}\nslug: {slug}\nschema_version: 2026-07-19-v2\ncreated_at: {created_at}\ndomain: marketing\nstatus: {status}\ncurrent_phase: active\nlast_activity: {last_activity}T10:00:00+00:00\n---\n"
        ),
    );
    fx.p(&rel)
}

// ------------------------------------------------------------------ NewProjectDefaultTests

#[test]
fn test_cli_defaults_to_neutral_project_management() {
    let fx = Fx::with_packs(&["project-mgmt", "marketing"]);
    fx.ok(&["new-project", "anything"]);
    let cfm = fx.fm("workspace/projects/anything/project.md");
    assert_eq!(get_scalar(&cfm, "domain").as_deref(), Some("project-mgmt"));
    assert!(!has_field(&cfm, "flow"));
}

#[test]
fn test_scaffold_writes_no_flow_key_and_no_lessons_file() {
    let fx = Fx::with_packs(&["marketing"]);
    fx.ok(&["new-project", "scaffold", "--domain", "marketing"]);
    let cfm = fx.fm("workspace/projects/scaffold/project.md");
    assert!(!has_field(&cfm, "flow"));
    assert_eq!(get_scalar(&cfm, "current_phase").as_deref(), Some("active"));
    let mut names: Vec<String> = std::fs::read_dir(fx.path("workspace/projects/scaffold"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["activity.md", "knowledge", "project.md", "sources"]);
}

// ------------------------------------------------------------------ ProjectStateContractTests

fn contract_fx(status: &str, terminal: &str) -> (Fx, String) {
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.write(
        "workspace/projects/minimal/project.md",
        &format!(
            "---\nname: Minimal\nslug: minimal\nschema_version: 2026-07-19-v2\ncreated_at: 2026-07-19\ndomain: project-mgmt\nstatus: {status}\ncurrent_phase: active\nlast_activity: 2026-07-19T10:00:00-07:00\n{terminal}deliverables: {{}}\n---\n\n# Minimal\n"
        ),
    );
    let cdir = fx.p("workspace/projects/minimal");
    (fx, cdir)
}

#[test]
fn test_minimal_active_index_needs_no_null_placeholders() {
    let (fx, cdir) = contract_fx("active", "");
    assert_eq!(doctor::check_project(&fx.ctx(), &cdir).unwrap(), Vec::<String>::new());
}

#[test]
fn test_legacy_flow_and_build_repo_keys_are_silently_accepted() {
    let (fx, cdir) = contract_fx("active", "flow: marketing-solo-flow\nbuild_repo: C:/code/app\n");
    assert_eq!(doctor::check_project(&fx.ctx(), &cdir).unwrap(), Vec::<String>::new());
}

#[test]
fn test_complete_requires_completed_at() {
    let (fx, cdir) = contract_fx("complete", "");
    assert!(any_contains(&doctor::check_project(&fx.ctx(), &cdir).unwrap(), "complete requires completed_at"));
}

#[test]
fn test_terminal_timestamp_must_match_lifecycle() {
    let (fx, cdir) = contract_fx("active", "completed_at: 2026-07-19\n");
    assert!(any_contains(
        &doctor::check_project(&fx.ctx(), &cdir).unwrap(),
        "completed_at is present but status is active"
    ));
}

// ------------------------------------------------------------------ ActivitySizeNoteTests

fn chatty(fx: &Fx, slug: &str, lines: usize, status: &str) -> String {
    let cdir = make_project(fx, slug, status, &days_ago(90), &days_ago(1));
    let text: String = (0..lines).map(|i| format!("2026-07-01 — note: event {i}\n")).collect();
    fx.write(&format!("workspace/projects/{slug}/activity.md"), &text);
    cdir
}

#[test]
fn test_fresh_active_project_gets_no_note() {
    let fx = Fx::new();
    let cdir = make_project(&fx, "fresh", "active", &days_ago(90), &days_ago(1));
    assert_eq!(doctor::activity_size_note(&fx.ctx(), &cdir).unwrap(), None);
}

#[test]
fn test_activity_archive_threshold_is_200_lines() {
    let fx = Fx::new();
    assert_eq!(doctor::activity_size_note(&fx.ctx(), &chatty(&fx, "at-cap", 200, "active")).unwrap(), None);
    let note = doctor::activity_size_note(&fx.ctx(), &chatty(&fx, "over-cap", 201, "active")).unwrap().unwrap();
    assert!(note.contains("activity.md 201 lines, over 200; archive the tail to knowledge/_archive/"));
    assert!(!note.contains("skills"));
}

#[test]
fn test_non_active_project_never_fires() {
    let fx = Fx::new();
    assert_eq!(doctor::activity_size_note(&fx.ctx(), &chatty(&fx, "done", 400, "complete")).unwrap(), None);
}

// ------------------------------------------------------------------ ArchivedRowDerivedTotalTests

const PROJECT_FM: &str = "name: arch\nslug: arch\ndomain: marketing\nstatus: active\ndeliverables:\n  post-1:\n    status: published\n    file: posts/post-1/post-FINAL.md\n    last_updated: 2026-06-01\n  post-2:\n    status: drafting\n    file: posts/post-2/post-FINAL.md\n    last_updated: 2026-07-01";

const ARCHIVE_FM: &str = "---\ndeliverables:\n  post-0:\n    status: published\n    file: posts/post-0/post-FINAL.md\n    last_updated: 2026-05-01\n---\n\n> Published rows archived from project.md.\n";

fn derived_total(cdir: &str) -> usize {
    marketing::published_posts(PROJECT_FM) + marketing::archived_published_posts(cdir).unwrap()
}

#[test]
fn test_derived_total_includes_archive() {
    let fx = Fx::new();
    fx.write("workspace/projects/arch/knowledge/_archive/deliverables-archive.md", ARCHIVE_FM);
    assert_eq!(derived_total(&fx.p("workspace/projects/arch")), 2);
}

#[test]
fn test_derived_total_without_archive_counts_tracker_only() {
    let fx = Fx::new();
    fx.mkdir("workspace/projects/arch/knowledge/_archive");
    assert_eq!(derived_total(&fx.p("workspace/projects/arch")), 1);
}

// ------------------------------------------------------------------ ReadyExportGateTests

fn export_fx(slug: &str, fname: &str, extra_fm: &str) -> (Fx, String) {
    let fx = Fx::with_packs(&["marketing", "project-mgmt"]);
    let rel = format!("{slug}/{fname}");
    fx.write(
        "workspace/projects/carousel-proj/project.md",
        &format!(
            "---\nname: carousel-proj\nslug: carousel-proj\ndomain: marketing\nstatus: active\nlast_activity: 2026-07-01T10:00:00+00:00\ndeliverables:\n  {slug}:\n    status: drafting\n    file: {rel}\n    last_updated: 2026-07-01\n---\n"
        ),
    );
    let mut dfm = "status: drafting\nlast_updated: 2026-07-01".to_string();
    if !extra_fm.is_empty() {
        dfm.push('\n');
        dfm.push_str(extra_fm);
    }
    fx.write(&format!("workspace/projects/carousel-proj/{rel}"), &format!("---\n{dfm}\n---\n\nbody\n"));
    (fx, rel)
}

fn status_of(fx: &Fx, project: &str, rel: &str) -> Option<String> {
    get_scalar(&fx.fm(&format!("workspace/projects/{project}/{rel}")), "status")
}

#[test]
fn test_ready_refuses_image_prompts_with_no_exports() {
    let (fx, rel) = export_fx("post-1-carousel", "image-prompts-v1.md", "");
    fx.refused(&["ready", "carousel-proj", "post-1-carousel"]);
    assert_eq!(status_of(&fx, "carousel-proj", &rel).as_deref(), Some("drafting"));
}

#[test]
fn test_ready_refuses_dangling_exports_path() {
    let (fx, rel) = export_fx("post-1-carousel", "image-prompts-v1.md", "exports:\n  - media/missing.png");
    fx.refused(&["ready", "carousel-proj", "post-1-carousel"]);
    assert_eq!(status_of(&fx, "carousel-proj", &rel).as_deref(), Some("drafting"));
}

#[test]
fn test_ready_accepts_image_prompts_with_filed_exports() {
    let (fx, rel) = export_fx("post-1-carousel", "image-prompts-v1.md", "exports:\n  - media/final.pdf");
    fx.write("workspace/projects/carousel-proj/post-1-carousel/media/final.pdf", "x");
    fx.ok(&["ready", "carousel-proj", "post-1-carousel"]);
    assert_eq!(status_of(&fx, "carousel-proj", &rel).as_deref(), Some("ready"));
}

#[test]
fn test_override_marks_ready_and_activity() {
    let (fx, rel) = export_fx("post-1-carousel", "image-prompts-v1.md", "");
    fx.ok(&["ready", "carousel-proj", "post-1-carousel", "--allow-missing-exports"]);
    assert_eq!(status_of(&fx, "carousel-proj", &rel).as_deref(), Some("ready"));
    assert!(fx.read("workspace/projects/carousel-proj/activity.md").contains("WITHOUT EXPORTS"));
}

#[test]
fn test_non_exportable_deliverable_becomes_ready_without_exports() {
    let (fx, rel) = export_fx("post-1-body", "body-copy-v1.md", "");
    fx.ok(&["ready", "carousel-proj", "post-1-body"]);
    assert_eq!(status_of(&fx, "carousel-proj", &rel).as_deref(), Some("ready"));
}

#[test]
fn test_direct_path_synchronizes_matching_tracker_row() {
    let (fx, rel) = export_fx("post-1-body", "body-copy-v1.md", "");
    fx.ok(&["ready", "carousel-proj", &rel]);
    let cfm = fx.fm("workspace/projects/carousel-proj/project.md");
    assert_eq!(row_get(&cfm, "post-1-body", "status").as_deref(), Some("ready"));
}

// ------------------------------------------------------------------ VersionCommandCharacterizationTests

fn version_fm(row: &str, status: &str, file: &str) -> String {
    format!(
        "---\nname: Version Project\nslug: version-project\nschema_version: 2026-07-19-v2\ncreated_at: 2026-07-01\ndomain: marketing\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-07-01T10:00:00+00:00\ndeliverables:\n  {row}:\n    status: {status}\n    file: {file}\n    last_updated: 2026-07-01\n---\n"
    )
}

const VP: &str = "workspace/projects/version-project";

fn vfx() -> Fx {
    Fx::with_packs(&["marketing", "project-mgmt"])
}

fn make_state(fx: &Fx, row: &str, rel: &str, status: &str) {
    fx.write(&format!("{VP}/project.md"), &version_fm(row, status, rel));
}

fn make_artifact(fx: &Fx, rel: &str, status: &str, body: &str) -> String {
    fx.write(&format!("{VP}/{rel}"), &format!("---\nstatus: {status}\nlast_updated: 2026-07-01\n---\n\n{body}"))
}

fn tracker_file(fx: &Fx, row: &str) -> Option<String> {
    row_get(&fx.fm(&format!("{VP}/project.md")), row, "file")
}

#[test]
fn test_tracker_owned_head_versions_and_moves_pointer() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/brief-v1.md", "drafting");
    make_artifact(&fx, "brief/brief-v1.md", "drafting", "source snapshot\n");
    let before = fx.read(&format!("{VP}/brief/brief-v1.md"));
    fx.ok(&["version", "version-project", "brief"]);
    assert_eq!(tracker_file(&fx, "brief").as_deref(), Some("brief/brief-v2.md"));
    assert!(fx.isfile(&format!("{VP}/brief/brief-v2.md")));
    assert_eq!(fx.read(&format!("{VP}/brief/brief-v1.md")), before);
}

#[test]
fn test_tracker_owned_new_head_resets_drafting_fields() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/brief-v1.md", "ready");
    make_artifact(&fx, "brief/brief-v1.md", "ready", "body\n");
    fx.ok(&["version", "version-project", "brief"]);
    let new_fm = fx.fm(&format!("{VP}/brief/brief-v2.md"));
    assert_eq!(get_scalar(&new_fm, "status").as_deref(), Some("drafting"));
    assert_eq!(get_scalar(&new_fm, "last_updated"), Some(today()));
}

#[test]
fn test_post_final_pointer_versions_named_nested_body_copy() {
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "drafting");
    make_artifact(&fx, rel, "drafting", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-v1.md", "drafting", "ingredient\n");
    let before = fx.read(&format!("{VP}/posts/post-8/body-copy-v1.md"));
    fx.ok(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(fx.exists(&format!("{VP}/posts/post-8/body-copy-v2.md")));
    assert_eq!(fx.read(&format!("{VP}/posts/post-8/body-copy-v1.md")), before);
    assert_eq!(tracker_file(&fx, "post-8").as_deref(), Some(rel));
}

#[test]
fn test_nested_artifact_name_is_exact() {
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "drafting");
    make_artifact(&fx, rel, "drafting", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-notes-v1.md", "drafting", "body\n");
    let err = fx.refused(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(err.contains("artifact 'body-copy' has no versioned head"));
}

#[test]
fn test_nested_artifact_updates_parent_status_but_not_pointer() {
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "ready");
    make_artifact(&fx, rel, "ready", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-v1.md", "ready", "body\n");
    fx.ok(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    let cfm = fx.fm(&format!("{VP}/project.md"));
    assert_eq!(row_get(&cfm, "post-8", "file").as_deref(), Some(rel));
    assert_eq!(row_get(&cfm, "post-8", "status").as_deref(), Some("drafting"));
}

#[test]
fn test_versioning_ready_head_records_source_status_without_unlock_ceremony() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/brief-v1.md", "ready");
    make_artifact(&fx, "brief/brief-v1.md", "ready", "body\n");
    fx.ok(&["version", "version-project", "brief"]);
    assert!(fx.isfile(&format!("{VP}/brief/brief-v2.md")));
    let activity = fx.read(&format!("{VP}/activity.md"));
    assert!(activity.contains("artifact_versioned: brief"));
    assert!(activity.contains("source_status=ready"));
    assert!(!activity.contains("unlock_version"));
}

#[test]
fn test_routine_version_appends_artifact_versioned_pulse() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/brief-v1.md", "drafting");
    make_artifact(&fx, "brief/brief-v1.md", "drafting", "body\n");
    fx.ok(&["version", "version-project", "brief"]);
    let activity = fx.read(&format!("{VP}/activity.md"));
    assert!(activity.contains("artifact_versioned: brief v1 -> v2; brief/brief-v2.md"));
    assert!(!activity.contains("unlock_version"));
    let last = activity.trim().lines().last().unwrap();
    assert!(doctor::activity_line_re().is_match(last));
}

#[test]
fn test_nested_version_pulse_uses_artifact_label() {
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "drafting");
    make_artifact(&fx, rel, "drafting", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-v1.md", "drafting", "body\n");
    fx.ok(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    let activity = fx.read(&format!("{VP}/activity.md"));
    assert!(activity.contains("artifact_versioned: body-copy v1 -> v2; posts/post-8/body-copy-v2.md"));
}

#[test]
fn test_failed_version_appends_no_activity() {
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "drafting");
    make_artifact(&fx, rel, "drafting", "body\n");
    fx.refused(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(!fx.isfile(&format!("{VP}/activity.md")));
}

#[test]
fn test_version_receipt_states_copy_forward_contract() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/brief-v1.md", "drafting");
    make_artifact(&fx, "brief/brief-v1.md", "drafting", "body\n");
    let out = fx.ok(&["version", "version-project", "brief"]);
    assert!(out.contains("already contains"));
    // Every content change versions first, copyedits included.
    assert!(out.contains("versions first"));
    assert!(!out.contains("surgical"));
}

#[test]
fn test_cli_parser_accepts_nested_artifact_address() {
    // Python patches cmd_version and inspects args.artifact; here the parsed address has to
    // reach the command: the nested head versions, the parent pointer stays.
    let fx = vfx();
    let rel = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", rel, "drafting");
    make_artifact(&fx, rel, "drafting", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-v1.md", "drafting", "body\n");
    fx.ok(&["version", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(fx.isfile(&format!("{VP}/posts/post-8/body-copy-v2.md")));
}

// ------------------------------------------------------------------ DraftCommandTests

#[test]
fn test_tracker_owned_draft_creates_v1_and_moves_pointer() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/placeholder.md", "not_started");
    fx.ok(&["draft", "version-project", "brief", "--file", "brief/brief-v1.md"]);
    let path = format!("{VP}/brief/brief-v1.md");
    assert!(fx.isfile(&path));
    assert_eq!(tracker_file(&fx, "brief").as_deref(), Some("brief/brief-v1.md"));
    assert_eq!(get_scalar(&fx.fm(&path), "status").as_deref(), Some("drafting"));
}

#[test]
fn test_tracker_owned_draft_refuses_non_v1_path() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/placeholder.md", "not_started");
    let err = fx.refused(&["draft", "version-project", "brief", "--file", "brief/brief.md"]);
    assert!(err.contains("must name a canonical -v1.md"));
}

#[test]
fn test_marketing_artifact_draft_creates_assembly_record_and_preserves_parent_address() {
    let fx = vfx();
    let parent = "phase-3-production/posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", parent, "not_started");
    fx.ok(&["draft", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(fx.isfile(&format!("{VP}/phase-3-production/posts/post-8/body-copy-v1.md")));
    assert!(fx.isfile(&format!("{VP}/phase-3-production/posts/post-8/post-FINAL.md")));
    assert_eq!(tracker_file(&fx, "post-8").as_deref(), Some(parent));
}

#[test]
fn test_artifact_draft_refuses_existing_chain() {
    let fx = vfx();
    let parent = "posts/post-8/post-FINAL.md";
    make_state(&fx, "post-8", parent, "drafting");
    make_artifact(&fx, parent, "drafting", "body\n");
    make_artifact(&fx, "posts/post-8/body-copy-v1.md", "drafting", "body\n");
    let err = fx.refused(&["draft", "version-project", "post-8", "--artifact", "body-copy"]);
    assert!(err.contains("already has a version chain"));
}

#[test]
fn test_tracker_owned_draft_appends_artifact_drafted_pulse() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/placeholder.md", "not_started");
    fx.ok(&["draft", "version-project", "brief", "--file", "brief/brief-v1.md"]);
    let activity = fx.read(&format!("{VP}/activity.md"));
    assert!(activity.contains("artifact_drafted: brief created; brief/brief-v1.md"));
    assert!(doctor::activity_line_re().is_match(activity.trim().lines().last().unwrap()));
}

#[test]
fn test_nested_artifact_draft_appends_artifact_drafted_pulse() {
    let fx = vfx();
    make_state(&fx, "post-8", "phase-3-production/posts/post-8/post-FINAL.md", "not_started");
    fx.ok(&["draft", "version-project", "post-8", "--artifact", "body-copy"]);
    let activity = fx.read(&format!("{VP}/activity.md"));
    assert!(activity.contains("artifact_drafted: body-copy created; phase-3-production/posts/post-8/body-copy-v1.md"));
}

#[test]
fn test_failed_draft_appends_no_activity() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/placeholder.md", "not_started");
    fx.refused(&["draft", "version-project", "brief", "--file", "brief/brief.md"]);
    assert!(!fx.isfile(&format!("{VP}/activity.md")));
}

#[test]
fn test_cli_parser_requires_one_draft_address() {
    // --artifact alone parses (and reaches cmd_draft); neither or both is a usage error.
    let fx = vfx();
    make_state(&fx, "post-8", "posts/post-8/post-FINAL.md", "not_started");
    fx.ok(&["draft", "version-project", "post-8", "--artifact", "body-copy"]);
    assert_eq!(fx.run(&["draft", "version-project", "post-8"]).0, 2);
    assert_eq!(fx.run(&["draft", "version-project", "post-8", "--artifact", "a", "--file", "b-v1.md"]).0, 2);
}

#[test]
fn test_adopt_creates_row_from_empty_deliverables_map() {
    let fx = vfx();
    let state = version_fm("brief", "drafting", "brief/placeholder.md").replace(
        "deliverables:\n  brief:\n    status: drafting\n    file: brief/placeholder.md\n    last_updated: 2026-07-01",
        "deliverables: {}",
    );
    fx.write(&format!("{VP}/project.md"), &state);
    make_artifact(&fx, "brief/brief-v1.md", "drafting", "body\n");
    fx.ok(&["adopt", "version-project", "brief", "--file", "brief/brief-v1.md", "--notes", "existing renderer output"]);
    let cfm = fx.fm(&format!("{VP}/project.md"));
    assert_eq!(all_rows(&cfm), ["brief"]);
    assert_eq!(row_get(&cfm, "brief", "file").as_deref(), Some("brief/brief-v1.md"));
    assert_eq!(row_get(&cfm, "brief", "status").as_deref(), Some("drafting"));
}

#[test]
fn test_adopt_never_overwrites_existing_artifact() {
    let fx = vfx();
    make_state(&fx, "brief", "brief/current-v1.md", "drafting");
    make_artifact(&fx, "brief/current-v1.md", "drafting", "keep\n");
    make_artifact(&fx, "brief/replacement-v1.md", "drafting", "replacement\n");
    fx.refused(&["adopt", "version-project", "brief", "--file", "brief/replacement-v1.md"]);
    assert!(fx.read(&format!("{VP}/brief/current-v1.md")).contains("keep"));
}

// ------------------------------------------------------------------ MarketingReadyAssemblyTests

fn assembly_fx() -> Fx {
    let fx = Fx::with_packs(&["marketing"]);
    let c = "workspace/projects/campaign";
    fx.write(
        &format!("{c}/project.md"),
        "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\nlast_activity: 2026-07-19T10:00:00-07:00\npost_manifest:\n  ingredients: [body-copy, image-prompts]\ndeliverables:\n  post-1:\n    status: drafting\n    file: posts/post-1/post-FINAL.md\n    last_updated: 2026-07-19\n---\n",
    );
    fx.write(&format!("{c}/posts/post-1/post-FINAL.md"), "---\nstatus: drafting\nlast_updated: 2026-07-19\n---\n\n# Post\n");
    fx.write(&format!("{c}/posts/post-1/body-copy-v1.md"), "---\nstatus: ready\nlast_updated: 2026-07-19\n---\n\nBody\n");
    fx.write(&format!("{c}/posts/post-1/media/final.png"), "png");
    fx.write(
        &format!("{c}/posts/post-1/image-prompts-v1.md"),
        "---\nstatus: drafting\nlast_updated: 2026-07-19\nexports:\n  - media/final.png\n---\n\nPrompts\n",
    );
    fx
}

#[test]
fn test_last_ready_ingredient_marks_assembly_and_tracker_ready() {
    let fx = assembly_fx();
    fx.ok(&["ready", "campaign", "posts/post-1/image-prompts-v1.md"]);
    let cfm = fx.fm("workspace/projects/campaign/project.md");
    let pfm = fx.fm("workspace/projects/campaign/posts/post-1/post-FINAL.md");
    let pbody = fx.body("workspace/projects/campaign/posts/post-1/post-FINAL.md");
    assert_eq!(row_get(&cfm, "post-1", "status").as_deref(), Some("ready"));
    assert_eq!(get_scalar(&pfm, "status").as_deref(), Some("ready"));
    assert!(pbody.contains("Image Prompts (ready from image-prompts-v1.md)"));
}

#[test]
fn test_post_assembly_cannot_be_ready_before_its_ingredients() {
    let fx = assembly_fx();
    fx.refused(&["ready", "campaign", "post-1"]);
    let cfm = fx.fm("workspace/projects/campaign/project.md");
    assert_eq!(row_get(&cfm, "post-1", "status").as_deref(), Some("drafting"));
}

// ------------------------------------------------------------------ PublishCommandTests

fn publish_project(fx: &Fx, slug: &str, domain: &str, row: &str, rel: &str, status: &str) -> String {
    let c = format!("workspace/projects/{slug}");
    fx.write(
        &format!("{c}/project.md"),
        &format!(
            "---\nname: {slug}\nslug: {slug}\nschema_version: {PROJECT_SCHEMA_VERSION}\ncreated_at: 2026-07-19\ndomain: {domain}\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-07-19T10:00:00-07:00\ndeliverables:\n  {row}:\n    status: {status}\n    file: {rel}\n    last_updated: 2026-07-19\n---\n"
        ),
    );
    fx.write(&format!("{c}/{rel}"), &format!("---\nstatus: {status}\nlast_updated: 2026-07-19\n---\n\nbody\n"));
    c
}

fn pfx() -> Fx {
    Fx::with_packs(&["marketing", "project-mgmt"])
}

#[test]
fn test_generic_publish_moves_ready_artifact_and_tracker_to_published() {
    let fx = pfx();
    let c = publish_project(&fx, "generic", "project-mgmt", "brief", "brief/brief-v1.md", "ready");
    fx.ok(&["publish", "generic", "brief"]);
    let cfm = fx.fm(&format!("{c}/project.md"));
    let dfm = fx.fm(&format!("{c}/brief/brief-v1.md"));
    assert_eq!(row_get(&cfm, "brief", "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&dfm, "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&dfm, "published_url"), None);
    assert!(fx.read(&format!("{c}/activity.md")).contains("publish: brief published"));
}

#[test]
fn test_generic_publish_records_url_only_when_supplied() {
    let fx = pfx();
    let c = publish_project(&fx, "generic", "project-mgmt", "brief", "brief/brief-v1.md", "ready");
    fx.ok(&["publish", "generic", "brief", "--url", "https://example.com/brief"]);
    let dfm = fx.fm(&format!("{c}/brief/brief-v1.md"));
    assert_eq!(get_scalar(&dfm, "published_url").as_deref(), Some("https://example.com/brief"));
}

#[test]
fn test_generic_publish_refuses_drafting_artifact() {
    let fx = pfx();
    let c = publish_project(&fx, "generic", "project-mgmt", "brief", "brief/brief-v1.md", "drafting");
    fx.refused(&["publish", "generic", "brief"]);
    assert_eq!(row_get(&fx.fm(&format!("{c}/project.md")), "brief", "status").as_deref(), Some("drafting"));
}

#[test]
fn test_marketing_publish_records_receipt_and_adds_optional_ship_date() {
    let fx = pfx();
    let c = publish_project(&fx, "campaign", "marketing", "post-1", "posts/post-1/post-FINAL.md", "ready");
    fx.ok(&["publish", "campaign", "post-1", "--url", "https://example.com/post-1"]);
    let cfm = fx.fm(&format!("{c}/project.md"));
    let pfm = fx.fm(&format!("{c}/posts/post-1/post-FINAL.md"));
    assert_eq!(row_get(&cfm, "post-1", "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&pfm, "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&cfm, "shipped_at"), Some(today()));
    assert!(pfm.contains("url: https://example.com/post-1"));
}

#[test]
fn test_backfilled_publish_ships_on_the_posted_date_not_today() {
    let fx = pfx();
    let rel = "posts/post-1/post-FINAL.md";
    let c = publish_project(&fx, "campaign", "marketing", "post-1", rel, "ready");
    fx.ok(&["publish", "campaign", "post-1", "--url", "https://example.com/post-1", "--posted-at", "2026-07-27"]);
    let cfm = fx.fm(&format!("{c}/project.md"));
    let pfm = fx.fm(&format!("{c}/{rel}"));
    assert!(pfm.contains("shipped_at: 2026-07-27"));
    assert!(pfm.contains("posted_at: 2026-07-27"));
    assert_eq!(get_scalar(&cfm, "shipped_at").as_deref(), Some("2026-07-27"));
}

#[test]
fn test_backfilled_publish_accepts_a_full_timestamp_posted_at() {
    let fx = pfx();
    let rel = "posts/post-1/post-FINAL.md";
    let c = publish_project(&fx, "campaign", "marketing", "post-1", rel, "ready");
    fx.ok(&["publish", "campaign", "post-1", "--url", "https://example.com/post-1", "--posted-at", "2026-07-27T18:40:00-07:00"]);
    assert!(fx.fm(&format!("{c}/{rel}")).contains("shipped_at: 2026-07-27"));
}

#[test]
fn test_non_post_marketing_deliverable_uses_generic_publish() {
    let fx = pfx();
    let c = publish_project(&fx, "campaign", "marketing", "essay", "essay/substack-essay-v1.md", "ready");
    fx.ok(&["publish", "campaign", "essay"]);
    assert_eq!(row_get(&fx.fm(&format!("{c}/project.md")), "essay", "status").as_deref(), Some("published"));
}

#[test]
fn test_marketing_post_path_keeps_receipt_aware_publish() {
    let fx = pfx();
    let rel = "posts/post-1/post-FINAL.md";
    let c = publish_project(&fx, "campaign", "marketing", "post-1", rel, "ready");
    fx.ok(&["publish", "campaign", rel, "--url", "https://example.com/by-path"]);
    let cfm = fx.fm(&format!("{c}/project.md"));
    let pfm = fx.fm(&format!("{c}/{rel}"));
    assert_eq!(row_get(&cfm, "post-1", "status").as_deref(), Some("published"));
    assert!(pfm.contains("url: https://example.com/by-path"));
}

// ------------------------------------------------------------------ BinaryHeadTests

const BIN_REL: &str = "phase-4-production/decks/session-deck/deck_20260812_120000.pptx";

fn binary_fx() -> (Fx, String) {
    let fx = Fx::with_packs(&["project-mgmt"]);
    let c = "workspace/projects/deckproj";
    // Real PPTX bytes: undecodable as UTF-8, which is what caused the crash.
    fx.write_bytes(&format!("{c}/{BIN_REL}"), b"PK\x03\x04\xfe\xff\x00\x01binary");
    fx.write(
        &format!("{c}/project.md"),
        &format!(
            "---\nname: deckproj\nslug: deckproj\nschema_version: {PROJECT_SCHEMA_VERSION}\ncreated_at: 2026-08-12\ndomain: project-mgmt\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-08-12T10:00:00-07:00\ndeliverables:\n  session-deck:\n    status: drafting\n    file: {BIN_REL}\n    last_updated: 2026-08-12\n---\n"
        ),
    );
    let cdir = fx.p(c);
    (fx, cdir)
}

#[test]
fn test_ready_refuses_with_the_convention_instead_of_crashing() {
    let (fx, _) = binary_fx();
    let msg = fx.refused(&["ready", "deckproj", "session-deck", "--allow-missing-exports"]);
    assert!(msg.contains("not a Markdown head"));
    assert!(msg.contains("exports[]"));
    assert!(msg.contains("deck-production.md"));
}

#[test]
fn test_publish_refuses_with_the_convention() {
    let (fx, _) = binary_fx();
    assert!(fx.refused(&["publish", "deckproj", "session-deck"]).contains("not a Markdown head"));
}

#[test]
fn test_version_refuses_with_the_convention() {
    let (fx, cdir) = binary_fx();
    let (cfm, _) = split_fm(&fx.read("workspace/projects/deckproj/project.md"), "project.md").unwrap();
    let err = version_target(&cdir, &cfm, "session-deck", None).unwrap_err();
    assert!(err.to_string().contains("not a Markdown head"));
}

#[test]
fn test_doctor_surfaces_the_row_as_an_advisory_note() {
    let (fx, cdir) = binary_fx();
    let notes = doctor::binary_head_notes(&fx.ctx(), &cdir).unwrap();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("session-deck"));
    assert!(notes[0].contains("no button can stamp it"));
}

#[test]
fn test_a_markdown_head_beside_the_binary_is_accepted() {
    let (fx, cdir) = binary_fx();
    let rel = "phase-4-production/decks/session-deck/session-deck-v1.md";
    fx.write(
        &format!("workspace/projects/deckproj/{rel}"),
        "---\nstatus: drafting\nlast_updated: 2026-08-12\nexports:\n  - deck_20260812_120000.pptx\n---\n\nPointer head.\n",
    );
    let cpath = "workspace/projects/deckproj/project.md";
    let (cfm, cbody) = split_fm(&fx.read(cpath), "project.md").unwrap();
    let cfm = row_set(&cfm, "session-deck", "file", rel).unwrap();
    fx.write(cpath, &join_fm(&cfm, &cbody));
    fx.ok(&["ready", "deckproj", "session-deck", "--allow-missing-exports"]);
    assert_eq!(row_get(&fx.fm(cpath), "session-deck", "status").as_deref(), Some("ready"));
    assert_eq!(doctor::binary_head_notes(&fx.ctx(), &cdir).unwrap(), Vec::<String>::new());
}

// ------------------------------------------------------------------ LifecycleCliParserTests

#[test]
fn test_ready_is_the_quality_gate_command() {
    // Python patches cmd_ready and reads args.deliverable; here the deliverable must reach it.
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.write(
        "workspace/projects/project/project.md",
        "---\nname: p\nslug: project\ndomain: project-mgmt\nstatus: active\nlast_activity: 2026-07-19T10:00:00-07:00\ndeliverables:\n  brief:\n    status: drafting\n    file: brief/brief-v1.md\n    last_updated: 2026-07-19\n---\n",
    );
    fx.write("workspace/projects/project/brief/brief-v1.md", "---\nstatus: drafting\nlast_updated: 2026-07-19\n---\nbody\n");
    assert!(fx.ok(&["ready", "project", "brief"]).contains("brief/brief-v1.md -> ready"));
}

#[test]
fn test_retired_lock_command_is_not_parsed() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["lock", "project", "brief"]).0, 2);
}

// ------------------------------------------------------------------ LifecycleCompositionSmokeTests

#[test]
fn test_new_project_through_publish_and_doctor() {
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.ok(&["new-project", "lifecycle-smoke", "--domain", "project-mgmt"]);
    fx.ok(&["draft", "lifecycle-smoke", "brief", "--file", "brief/brief-v1.md"]);
    fx.ok(&["version", "lifecycle-smoke", "brief"]);
    fx.ok(&["ready", "lifecycle-smoke", "brief"]);
    fx.ok(&["publish", "lifecycle-smoke", "brief", "--url", "https://example.com/brief"]);
    let doctor_out = fx.ok(&["doctor", "lifecycle-smoke"]);

    let c = "workspace/projects/lifecycle-smoke";
    let cfm = fx.fm(&format!("{c}/project.md"));
    let v1fm = fx.fm(&format!("{c}/brief/brief-v1.md"));
    let v2fm = fx.fm(&format!("{c}/brief/brief-v2.md"));
    let activity = fx.read(&format!("{c}/activity.md"));
    assert_eq!(row_get(&cfm, "brief", "file").as_deref(), Some("brief/brief-v2.md"));
    assert_eq!(row_get(&cfm, "brief", "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&v1fm, "status").as_deref(), Some("drafting"));
    assert_eq!(get_scalar(&v2fm, "status").as_deref(), Some("published"));
    assert_eq!(get_scalar(&v2fm, "published_url").as_deref(), Some("https://example.com/brief"));
    for event in ["project_started", "artifact_drafted", "artifact_versioned", "ready", "publish"] {
        assert!(activity.contains(event), "{event} missing from activity.md");
    }
    assert!(doctor_out.contains("1 project(s) checked, books clean"));
}
