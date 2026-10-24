//! shipped_media / exports manifest checks.

mod common;
use afcore::doctor;
use common::*;

const PROJECT_FM: &str = "name: Test\nslug: test\nschema_version: 2026-07-19-v2\ncreated_at: 2026-07-07\ndomain: marketing\nstatus: active\ncurrent_phase: active\nlast_activity: 2026-07-07T12:00:00-07:00\npost_manifest:\n  ingredients: [body-copy, image-prompts]\ndeliverables:\n  post-1:\n    status: published\n    file: posts/post-1/post-FINAL.md\n    last_updated: 2026-07-07\n";

fn media_fx() -> (Fx, String) {
    let fx = Fx::new();
    fx.write("project.md", &format!("---\n{PROJECT_FM}---\n"));
    let cdir = fx.root();
    (fx, cdir)
}

#[test]
fn test_missing_shipped_media_path_is_issue() {
    let (fx, cdir) = media_fx();
    fx.write(
        "posts/post-1/post-FINAL.md",
        "---\nstatus: published\nlast_updated: 2026-07-07\nshipped_media:\n  - posts/post-1/media/final.png\n---\n",
    );
    let issues = doctor::media_manifest_issues(&fx.ctx(), &cdir, &fx.fm("project.md")).unwrap();
    assert!(any_contains(&issues, "shipped_media path missing"));
}

#[test]
fn test_existing_owner_relative_media_path_passes() {
    let (fx, cdir) = media_fx();
    fx.write("posts/post-1/media/final.png", "x");
    fx.write(
        "posts/post-1/post-FINAL.md",
        "---\nstatus: published\nlast_updated: 2026-07-07\nshipped_media:\n  - media/final.png\n---\n",
    );
    assert_eq!(doctor::media_manifest_issues(&fx.ctx(), &cdir, &fx.fm("project.md")).unwrap(), Vec::<String>::new());
}

#[test]
fn test_published_image_post_without_shipped_media_is_note() {
    let (fx, cdir) = media_fx();
    fx.write("posts/post-1/visuals/draft.png", "x");
    fx.write("posts/post-1/post-FINAL.md", "---\nstatus: published\nlast_updated: 2026-07-07\n---\n");
    let notes = doctor::media_manifest_notes(&fx.ctx(), &cdir).unwrap();
    assert!(any_contains(&notes, "empty shipped_media"));
}

const CAROUSEL_PROJECT_FM: &str = "name: Test\nslug: test\ndomain: marketing\nstatus: active\ndeliverables:\n  post-1-carousel:\n    status: ready\n    file: post-1-carousel/image-prompts-v1.md\n    last_updated: 2026-07-09\n";

#[test]
fn test_ready_image_prompts_without_exports_is_issue() {
    let fx = Fx::new();
    fx.write("post-1-carousel/image-prompts-v1.md", "---\nstatus: ready\nlast_updated: 2026-07-09\n---\n");
    let issues = doctor::media_manifest_issues(&fx.ctx(), &fx.root(), CAROUSEL_PROJECT_FM).unwrap();
    assert!(any_contains(&issues, "empty exports[]"));
}

#[test]
fn test_ready_image_prompts_with_filed_exports_passes() {
    let fx = Fx::new();
    fx.write("post-1-carousel/media/final.pdf", "x");
    fx.write(
        "post-1-carousel/image-prompts-v1.md",
        "---\nstatus: ready\nlast_updated: 2026-07-09\nexports:\n  - media/final.pdf\n---\n",
    );
    assert_eq!(doctor::media_manifest_issues(&fx.ctx(), &fx.root(), CAROUSEL_PROJECT_FM).unwrap(), Vec::<String>::new());
}

#[test]
fn test_frontmatterless_ready_file_is_reported_without_abort() {
    let fx = Fx::new();
    fx.write("post-1-carousel/image-prompts-v1.md", "# legacy file\n");
    let issues = doctor::media_manifest_issues(&fx.ctx(), &fx.root(), CAROUSEL_PROJECT_FM).unwrap();
    assert!(any_contains(&issues, "no/invalid frontmatter"));
}
