//! The project shape doctor holds: a folder under workspace/projects/ without project.md is
//! an issue, and a drafting file outside the version chain is a note.

mod common;
use afcore::doctor;
use common::*;

const DRAFT: &str = "---\nstatus: drafting\nchanges_from_v1: shorter opening\n---\n\nBody.\n";

/// The shape a chat produced by hand: drafts with invented names and no project.md.
fn hand_made(fx: &Fx) {
    fx.write("workspace/projects/hand-made/README.md", "# Hand made\n");
    fx.write("workspace/projects/hand-made/draft-en.md", DRAFT);
    fx.write("workspace/projects/hand-made/superseded-draft-en-2.md", "---\nstatus: superseded\n---\n\nOld.\n");
}

#[test]
fn test_hand_made_folder_without_project_md_is_an_issue() {
    let fx = Fx::with_packs(&["project-mgmt"]);
    hand_made(&fx);
    let issues = doctor::stray_project_folder_issues(&fx.ctx()).unwrap();
    assert_eq!(
        issues,
        ["workspace/projects/hand-made: folder under workspace/projects/ with no project.md; create projects with af new-project, and register an existing draft with af adopt"]
    );
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("workspace/projects/hand-made: folder under workspace/projects/ with no project.md"));
    assert!(out.contains(
        "note — workspace/projects/hand-made/draft-en.md: drafting file outside the version chain; af adopt it or name it {name}-v{N}.md"
    ));
    assert!(!out.contains("superseded-draft-en-2.md"), "only drafting files are noted");
}

#[test]
fn test_completed_and_dot_folders_are_not_projects_to_check() {
    let fx = Fx::new();
    fx.mkdir("workspace/projects/completed/old");
    fx.mkdir("workspace/projects/.obsidian");
    fx.write("workspace/projects/loose-file.md", "x\n");
    assert!(doctor::stray_project_folder_issues(&fx.ctx()).unwrap().is_empty());
}

#[test]
fn test_new_project_folder_passes() {
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.ok(&["new-project", "proper", "--domain", "project-mgmt"]);
    fx.ok(&["draft", "proper", "eulogy", "--file", "eulogy-en-v1.md"]);
    fx.ok(&["version", "proper", "eulogy"]);
    assert!(doctor::stray_project_folder_issues(&fx.ctx()).unwrap().is_empty());
    let cdir = fx.p("workspace/projects/proper");
    assert!(doctor::unchained_draft_notes(&fx.ctx(), &cdir).unwrap().is_empty());
    let out = fx.ok(&["doctor", "proper"]);
    assert!(out.contains("books clean"));
}

#[test]
fn test_stray_drafting_file_in_a_project_is_noted() {
    let fx = Fx::with_packs(&["project-mgmt"]);
    fx.ok(&["new-project", "proper", "--domain", "project-mgmt"]);
    fx.ok(&["draft", "proper", "eulogy", "--file", "eulogy-en-v1.md"]);
    fx.write("workspace/projects/proper/draft-yue.md", DRAFT);
    fx.write("workspace/projects/proper/notes-scratchpad.md", DRAFT);
    fx.write("workspace/projects/proper/sources/raw.md", DRAFT);
    let cdir = fx.p("workspace/projects/proper");
    let notes = doctor::unchained_draft_notes(&fx.ctx(), &cdir).unwrap();
    assert_eq!(
        notes,
        ["workspace/projects/proper/draft-yue.md: drafting file outside the version chain; af adopt it or name it {name}-v{N}.md"]
    );
    let out = fx.ok(&["doctor", "proper"]);
    assert!(out.contains("draft-yue.md: drafting file outside the version chain"));
}
