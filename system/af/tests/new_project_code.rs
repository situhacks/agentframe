//! The code domain pack: `dirs`, `scaffold/` and `learnings` in a pack descriptor, the
//! learnings log they create, that `af search` finds its entries, and the runbook's word cap.

mod common;

use afcore::cmds::copy_scaffold;
use common::*;

const DEAD: &str = "http://127.0.0.1:9/api/embed";

fn code_fx() -> Fx {
    Fx::with_packs(&["project-mgmt", "code"])
}

fn dead(fx: &Fx, args: &[&str]) -> (i32, String, String) {
    let out = fx.af().env("AF_OLLAMA_URL", DEAD).args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n"),
    )
}

#[test]
fn code_project_gets_a_code_folder_a_gitignore_the_four_stubs_and_a_learnings_log() {
    let fx = code_fx();
    let out = fx.ok(&["new-project", "x", "--domain", "code"]);
    assert!(fx.path("workspace/projects/x/code").is_dir());
    let ignore = fx.read("workspace/projects/x/code/.gitignore");
    let lines: Vec<&str> = ignore.lines().collect();
    assert_eq!(
        lines,
        ["node_modules/", ".venv/", "venv/", "target/", "dist/", "build/", "__pycache__/", ".env", ".env.*", "*.log"]
    );
    let doc = fx.read("workspace/projects/x/project.md");
    for head in ["## Goal", "## Stack", "## Test command", "## Done when"] {
        assert!(doc.contains(head), "{head} missing from project.md");
    }
    assert_eq!(get_scalar(&fx.fm("workspace/projects/x/project.md"), "domain").as_deref(), Some("code"));
    let log = fx.read("workspace/learnings.md");
    assert!(log.starts_with("# Learnings\n"), "{log}");
    assert!(log.contains("library/schema/learnings.md"), "{log}");
    assert!(out.contains("workspace/learnings.md created"), "{out}");
    assert!(out.contains("library/domains/code/production.md"), "{out}");
}

#[test]
fn a_second_code_project_leaves_an_existing_learnings_log_byte_for_byte() {
    let fx = code_fx();
    fx.ok(&["new-project", "first", "--domain", "code"]);
    let mut kept = fx.read("workspace/learnings.md");
    kept.push_str("\n## L-2026-11-16-01 · a lesson he wrote\nSeen: by hand\n");
    fx.write("workspace/learnings.md", &kept);
    let out = fx.ok(&["new-project", "second", "--domain", "code"]);
    assert_eq!(fx.read("workspace/learnings.md"), kept);
    assert!(!out.contains("workspace/learnings.md created"), "{out}");
    assert!(fx.path("workspace/projects/second/code/.gitignore").is_file());
}

#[test]
fn a_project_mgmt_project_gets_no_code_folder_and_no_learnings_log() {
    let fx = code_fx();
    let out = fx.ok(&["new-project", "y", "--domain", "project-mgmt"]);
    assert!(!fx.exists("workspace/projects/y/code"));
    assert!(!fx.exists("workspace/learnings.md"));
    assert!(!out.contains("production.md"), "a pack with no runbook names none: {out}");
}

#[test]
fn a_scaffold_file_never_overwrites_a_file_the_scaffold_already_made() {
    let fx = code_fx();
    fx.copy_pack("code", "collide");
    // The pack ships a scaffold file at a path new-project writes itself.
    fx.write("library/domains/collide/scaffold/sources/INDEX.md", "FROM THE PACK\n");
    fx.ok(&["new-project", "z", "--domain", "collide"]);
    let index = fx.read("workspace/projects/z/sources/INDEX.md");
    assert!(index.starts_with("# Source Index"), "{index}");
    assert!(!index.contains("FROM THE PACK"));
    assert!(fx.path("workspace/projects/z/code/.gitignore").is_file(), "the other scaffold files still copy");
}

#[test]
fn copy_scaffold_copies_nested_files_skips_existing_ones_and_tolerates_no_scaffold_folder() {
    let fx = Fx::new();
    fx.write("src/a/b/new.txt", "new\n");
    fx.write("src/a/keep.txt", "from the pack\n");
    fx.write("dst/a/keep.txt", "already here\n");
    let copied = copy_scaffold(&fx.p("src"), &fx.p("dst")).unwrap();
    assert_eq!(copied, ["a/b/new.txt"]);
    assert_eq!(fx.read("dst/a/b/new.txt"), "new\n");
    assert_eq!(fx.read("dst/a/keep.txt"), "already here\n");
    assert!(copy_scaffold(&fx.p("nothing-here"), &fx.p("dst")).unwrap().is_empty());
}

#[test]
fn a_dirs_entry_that_leaves_the_project_is_refused() {
    let fx = code_fx();
    fx.copy_pack("code", "escape");
    let pack = fx.read("library/domains/escape/pack.md").replace("dirs: [code]", "dirs: [../outside]");
    fx.write("library/domains/escape/pack.md", &pack);
    fx.refused(&["new-project", "w", "--domain", "escape"]);
    assert!(!fx.exists("workspace/projects/outside"));
}

#[test]
fn a_code_project_passes_doctor() {
    let fx = code_fx();
    fx.ok(&["new-project", "x", "--domain", "code"]);
    let out = fx.ok(&["doctor", "x"]);
    assert!(out.contains("1 project(s) checked, books clean"), "{out}");
}

#[test]
fn search_finds_a_learnings_entry_by_a_path_in_its_watch_line() {
    let fx = code_fx();
    fx.write("AGENTS.md", "# root\n");
    fx.mkdir("system");
    fx.ok(&["new-project", "x", "--domain", "code"]);
    let mut log = fx.read("workspace/learnings.md");
    log.push_str(
        "\n## L-2026-11-16-01 · A parser change broke the importer\n\
         Seen: a project chat, 2026-11-16\n\
         Root cause: the importer read the field the parser renamed.\n\
         Guard: `workspace/projects/x/code/tests/test_importer.py`\n\
         Watch: `workspace/projects/x/code/importer.py`\n",
    );
    fx.write("workspace/learnings.md", &log);
    let (code, _, err) = dead(&fx, &["index", "update"]);
    assert_eq!(code, 0, "{err}");
    let (code, out, err) = dead(&fx, &["search", "importer.py", "--json", "-n", "10"]);
    assert_eq!(code, 0, "{err}");
    let hits: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    assert!(hits.iter().any(|h| h["path"].as_str().unwrap() == "workspace/learnings.md"), "{out}");
}

#[test]
fn production_runbook_stays_within_600_words() {
    let text = std::fs::read_to_string(repo_root().join("library/domains/code/production.md")).unwrap();
    let words = text.split_whitespace().count();
    assert!(words <= 600, "production.md is {words} words");
}
