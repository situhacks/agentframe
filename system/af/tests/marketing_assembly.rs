//! The post assembly record: what `af ready` lifts into post-FINAL.md, which ingredients a
//! post needs, and that a published record stays published. Rows are found by their file.

mod common;
use afcore::doctor;
use afcore::fm::{get_scalar, row_get};
use common::*;

const C: &str = "workspace/projects/campaign";

fn project(fx: &Fx, manifest: &str, rows: &str) {
    fx.write(
        &format!("{C}/project.md"),
        &format!(
            "---\nname: campaign\nslug: campaign\nschema_version: {SCHEMA}\ncreated_at: 2026-07-19\ndomain: marketing\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-07-19T10:00:00-07:00\npost_manifest:\n  ingredients: [{manifest}]\ndeliverables:\n{rows}---\n"
        ),
    );
}

fn post_row(slug: &str, folder: &str, status: &str, extra: &str) -> String {
    format!("  {slug}:\n    status: {status}\n    file: posts/{folder}/post-FINAL.md\n    last_updated: 2026-07-19\n{extra}")
}

fn head(status: &str, body: &str) -> String {
    format!("---\nstatus: {status}\nlast_updated: 2026-07-19\n---\n\n{body}")
}

fn drafting_final(fx: &Fx, folder: &str) {
    fx.write(&format!("{C}/posts/{folder}/post-FINAL.md"), &head("drafting", "# Post\n"));
}

const WORKING_HEAD: &str = "# Post 1, body copy\n\n## Variants\n\nA rejected hook.\n\n## Ship\n\nThe line that goes out.\n\nThe second paragraph that goes out.\n\n---\n\n## Copyedit pass\n\nAn agent note about a comma.\n";

fn rig() -> Fx {
    Fx::with_packs(&["marketing"])
}

// ------------------------------------------------------------------ shipping text

#[test]
fn ready_lifts_only_the_ship_region_of_a_working_head() {
    let fx = rig();
    project(&fx, "body-copy", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", WORKING_HEAD));
    fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    let body = fx.body(&format!("{C}/posts/post-1/post-FINAL.md"));
    assert!(body.contains("The line that goes out."), "{body}");
    assert!(body.contains("The second paragraph that goes out."), "{body}");
    assert!(!body.contains("A rejected hook."), "{body}");
    assert!(!body.contains("An agent note about a comma."), "{body}");
}

#[test]
fn a_heading_inside_a_fence_does_not_end_the_ship_region() {
    let fx = rig();
    project(&fx, "body-copy", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(
        &format!("{C}/posts/post-1/body-copy-v1.md"),
        &head("drafting", "## Ship\n\n```\n## Not a section\nStill shipping.\n```\n\n## Notes\n\nNot shipping.\n"),
    );
    fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    let body = fx.body(&format!("{C}/posts/post-1/post-FINAL.md"));
    assert!(body.contains("Still shipping."), "{body}");
    assert!(!body.contains("Not shipping."), "{body}");
}

#[test]
fn a_head_with_no_ship_region_goes_in_whole_and_the_button_says_how_to_narrow_it() {
    let fx = rig();
    project(&fx, "body-copy", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", "Plain caption.\n"));
    let out = fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    assert!(fx.body(&format!("{C}/posts/post-1/post-FINAL.md")).contains("Plain caption."));
    assert!(out.contains("no ## Ship region"), "{out}");
}

#[test]
fn an_empty_ship_region_keeps_what_the_record_has_notes_it_and_does_not_promote() {
    let fx = rig();
    project(&fx, "body-copy", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", "## Ship\n\nThe line that went out first.\n"));
    fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    let before = fx.body(&format!("{C}/posts/post-1/post-FINAL.md"));
    assert!(before.contains("The line that went out first."), "{before}");
    let cfm = fx.fm(&format!("{C}/project.md"));
    assert!(cfm.contains("status: ready"), "{cfm}");

    // A later head whose Ship heading lost its text, with a blank line and a rule under it.
    fx.write(&format!("{C}/posts/post-1/body-copy-v2.md"), &head("drafting", "## Ship\n\n---\n\n## Notes\n\nA note.\n"));
    fx.ok(&["version", "campaign", "post-1", "--artifact", "body-copy"]);
    fx.write(&format!("{C}/posts/post-1/body-copy-v3.md"), &head("drafting", "## Ship\n\n---\n\n## Notes\n\nA note.\n"));
    let out = fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    assert!(out.contains("empty ## Ship region"), "{out}");
    assert!(out.contains("keeps its earlier text"), "{out}");
    let after = fx.body(&format!("{C}/posts/post-1/post-FINAL.md"));
    assert!(after.contains("The line that went out first."), "{after}");
    assert!(!out.contains("post-FINAL ready"), "{out}");
}

#[test]
fn another_ingredient_finishing_does_not_promote_past_an_empty_ship_region() {
    let fx = rig();
    project(&fx, "body-copy, video-spec", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", "## Ship\n\n## Notes\n\nA note.\n"));
    let out = fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    assert!(out.contains("empty ## Ship region"), "{out}");
    fx.write(&format!("{C}/posts/post-1/video-spec-v1.md"), &head("drafting", "## Ship\n\nThe video.\n"));
    let out = fx.ok(&["ready", "campaign", "post-1", "--artifact", "video-spec"]);
    assert!(!out.contains("post-FINAL ready"), "{out}");
    assert!(!fx.fm(&format!("{C}/posts/post-1/post-FINAL.md")).contains("status: ready"));
}

// ------------------------------------------------------------------ which ingredients count

#[test]
fn a_deferred_ingredient_counts_as_finished() {
    let fx = rig();
    project(&fx, "body-copy, video-spec", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/video-spec-v1.md"), &head("deferred", "Photo post, no video.\n"));
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", "Caption.\n"));
    fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    let cfm = fx.fm(&format!("{C}/project.md"));
    assert_eq!(row_get(&cfm, "post-1", "status").as_deref(), Some("ready"));
    assert_eq!(get_scalar(&fx.fm(&format!("{C}/posts/post-1/post-FINAL.md")), "status").as_deref(), Some("ready"));
}

#[test]
fn a_row_ingredients_list_overrides_the_project_manifest() {
    let fx = rig();
    let rows = post_row("post-1", "post-1", "drafting", "    ingredients: [body-copy]\n");
    project(&fx, "body-copy, video-spec", &rows);
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("drafting", "Caption.\n"));
    fx.ok(&["ready", "campaign", "post-1", "--artifact", "body-copy"]);
    assert_eq!(row_get(&fx.fm(&format!("{C}/project.md")), "post-1", "status").as_deref(), Some("ready"));
}

#[test]
fn a_post_with_a_deferred_stub_and_an_override_can_be_readied_by_its_row() {
    let fx = rig();
    let rows = post_row("post-1", "post-1", "drafting", "    ingredients: [body-copy, video-spec]\n");
    project(&fx, "body-copy, slide-copy", &rows);
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/video-spec-v1.md"), &head("deferred", "Photo post.\n"));
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("ready", "Caption.\n"));
    fx.ok(&["ready", "campaign", "post-1"]);
    assert_eq!(row_get(&fx.fm(&format!("{C}/project.md")), "post-1", "status").as_deref(), Some("ready"));
}

#[test]
fn an_ingredient_still_drafting_keeps_blocking_the_post() {
    let fx = rig();
    project(&fx, "body-copy, video-spec", &post_row("post-1", "post-1", "drafting", ""));
    drafting_final(&fx, "post-1");
    fx.write(&format!("{C}/posts/post-1/video-spec-v1.md"), &head("drafting", "Spec.\n"));
    fx.write(&format!("{C}/posts/post-1/body-copy-v1.md"), &head("ready", "Caption.\n"));
    let msg = fx.refused(&["ready", "campaign", "post-1"]);
    assert!(msg.contains("video-spec"), "{msg}");
}

// ------------------------------------------------------------------ the published record

fn published_fx() -> Fx {
    let fx = rig();
    let rows = format!(
        "{}  alt-text:\n    status: drafting\n    file: posts/post-1-approach/alt-text-v1.md\n    last_updated: 2026-07-19\n",
        post_row("post-1-approach", "post-1-approach", "ready", "")
    );
    project(&fx, "body-copy", &rows);
    fx.write(&format!("{C}/posts/post-1-approach/post-FINAL.md"), &head("ready", "# Post\n\n## Body Copy (ready from body-copy-v1.md)\nCaption.\n"));
    fx.write(&format!("{C}/posts/post-1-approach/body-copy-v1.md"), &head("ready", "Caption.\n"));
    fx.write(&format!("{C}/posts/post-1-approach/alt-text-v1.md"), &head("drafting", "Alt line.\n"));
    fx
}

#[test]
fn publish_counts_a_row_named_for_its_folder() {
    let fx = published_fx();
    let out = fx.ok(&["publish", "campaign", "post-1-approach", "--url", "https://example.com/p"]);
    assert!(out.contains("published posts=1"), "{out}");
}

#[test]
fn readying_a_non_manifest_ingredient_after_publish_leaves_the_record_published() {
    let fx = published_fx();
    fx.ok(&["publish", "campaign", "post-1-approach", "--url", "https://example.com/p"]);
    let pf = format!("{C}/posts/post-1-approach/post-FINAL.md");
    let before = fx.read(&pf);
    let out = fx.ok(&["ready", "campaign", "alt-text"]);
    assert_eq!(fx.read(&pf), before, "a published post-FINAL.md is immutable");
    assert_eq!(row_get(&fx.fm(&format!("{C}/project.md")), "post-1-approach", "status").as_deref(), Some("published"));
    assert!(out.contains("post-FINAL.md is published"), "{out}");
}

#[test]
fn readying_a_manifest_ingredient_after_publish_leaves_the_record_published() {
    let fx = published_fx();
    fx.ok(&["publish", "campaign", "post-1-approach", "--url", "https://example.com/p"]);
    fx.write(&format!("{C}/posts/post-1-approach/body-copy-v2.md"), &head("drafting", "New caption.\n"));
    let pf = format!("{C}/posts/post-1-approach/post-FINAL.md");
    let before = fx.read(&pf);
    fx.ok(&["ready", "campaign", "posts/post-1-approach/body-copy-v2.md"]);
    assert_eq!(fx.read(&pf), before);
    assert_eq!(row_get(&fx.fm(&format!("{C}/project.md")), "post-1-approach", "status").as_deref(), Some("published"));
}

#[test]
fn a_non_manifest_ingredient_never_lands_in_the_record() {
    let fx = published_fx();
    let out = fx.ok(&["ready", "campaign", "alt-text"]);
    assert!(!fx.body(&format!("{C}/posts/post-1-approach/post-FINAL.md")).contains("Alt line."));
    assert!(out.contains("not one of this post's manifest ingredients"), "{out}");
}

#[test]
fn doctor_shipped_media_check_sees_a_row_named_for_its_folder() {
    let fx = Fx::new();
    let rows = format!("{}", post_row("post-1-approach", "post-1-approach", "published", ""));
    fx.write(
        "project.md",
        &format!("---\nname: T\nslug: t\ndomain: marketing\nstatus: active\npost_manifest:\n  ingredients: [image-prompts]\ndeliverables:\n{rows}---\n"),
    );
    fx.write("posts/post-1-approach/post-FINAL.md", &head("published", "Post.\n"));
    let notes = doctor::media_manifest_notes(&fx.ctx(), &fx.root()).unwrap();
    assert!(any_contains(&notes, "post-1-approach has imagery signals"), "{notes:?}");
}

// ------------------------------------------------------------------ write-first

#[test]
fn draft_registers_a_canonical_v1_written_first() {
    let fx = rig();
    project(&fx, "body-copy", "");
    fx.write(&format!("{C}/spine/spine-v1.md"), &head("drafting", "Written first.\n"));
    let out = fx.ok(&["draft", "campaign", "spine", "--file", "spine/spine-v1.md"]);
    assert!(out.contains("spine/spine-v1.md"), "{out}");
    let cfm = fx.fm(&format!("{C}/project.md"));
    assert_eq!(row_get(&cfm, "spine", "file").as_deref(), Some("spine/spine-v1.md"));
    assert_eq!(row_get(&cfm, "spine", "status").as_deref(), Some("drafting"));
    assert!(fx.read(&format!("{C}/spine/spine-v1.md")).contains("Written first."));
}

#[test]
fn draft_gives_a_written_head_with_no_frontmatter_the_shared_container() {
    let fx = rig();
    project(&fx, "body-copy", "");
    fx.write(&format!("{C}/spine/spine-v1.md"), "Written first, no fences.\n");
    fx.ok(&["draft", "campaign", "spine", "--file", "spine/spine-v1.md"]);
    let file = format!("{C}/spine/spine-v1.md");
    assert_eq!(get_scalar(&fx.fm(&file), "status").as_deref(), Some("drafting"));
    assert!(fx.body(&file).contains("Written first, no fences."));
}

#[test]
fn draft_refuses_a_real_chain_and_a_finished_head() {
    let fx = rig();
    project(&fx, "body-copy", "");
    fx.write(&format!("{C}/spine/spine-v1.md"), &head("superseded", "One.\n"));
    fx.write(&format!("{C}/spine/spine-v2.md"), &head("drafting", "Two.\n"));
    let msg = fx.refused(&["draft", "campaign", "spine", "--file", "spine/spine-v1.md"]);
    assert!(msg.contains("already has a version chain"), "{msg}");
    fx.write(&format!("{C}/other/other-v1.md"), &head("ready", "Done.\n"));
    let msg = fx.refused(&["draft", "campaign", "other", "--file", "other/other-v1.md"]);
    assert!(msg.contains("status must be drafting"), "{msg}");
}
