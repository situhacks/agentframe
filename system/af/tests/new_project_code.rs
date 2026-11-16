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

/// A copy of the code pack named `name` whose pack.md has `from` replaced by `to`.
fn pack_with(fx: &Fx, name: &str, from: &str, to: &str) {
    fx.copy_pack("code", name);
    let pack = fx.read(&format!("library/domains/{name}/pack.md"));
    assert!(pack.contains(from), "{from} not in pack.md");
    fx.write(&format!("library/domains/{name}/pack.md"), &pack.replace(from, to));
}

/// The refusal names the field and leaves nothing behind: no project folder, no learnings log.
fn refused_cleanly(fx: &Fx, pack: &str, slug: &str, field: &str) {
    let out = fx.refused(&["new-project", slug, "--domain", pack]);
    assert!(out.contains(field), "the refusal names '{field}': {out}");
    assert!(!fx.exists(&format!("workspace/projects/{slug}")), "a refused pack left a project folder");
    assert!(!fx.exists("workspace/learnings.md"), "a refused pack left a learnings log");
}

#[test]
fn a_dirs_entry_that_leaves_the_project_is_refused_before_anything_is_written() {
    for (i, bad) in ["../outside", "/abs", "\\\\server\\share", "C:/x", "C:x", "D:\\x", "a/../../b"].iter().enumerate() {
        let fx = code_fx();
        pack_with(&fx, "escape", "dirs: [code]", &format!("dirs: [\"{bad}\"]"));
        refused_cleanly(&fx, "escape", &format!("w{i}"), "dirs");
        assert!(!fx.exists("workspace/projects/outside"));
    }
}

#[test]
fn a_bad_dirs_entry_after_a_good_one_leaves_no_half_made_project() {
    let fx = code_fx();
    pack_with(&fx, "late", "dirs: [code]", "dirs: [code, C:/x]");
    refused_cleanly(&fx, "late", "w", "dirs");
}

#[test]
fn a_dirs_field_that_is_not_a_list_is_refused_naming_the_field() {
    for (i, bad) in ["dirs: code", "dirs: [code", "dirs: code]", "dirs:"].iter().enumerate() {
        let fx = code_fx();
        pack_with(&fx, "badlist", "dirs: [code]", bad);
        refused_cleanly(&fx, "badlist", &format!("w{i}"), "dirs");
    }
}

#[test]
fn an_empty_dirs_list_and_a_block_list_are_accepted() {
    let fx = code_fx();
    pack_with(&fx, "nodirs", "dirs: [code]", "dirs: []");
    fx.ok(&["new-project", "a", "--domain", "nodirs"]);
    assert!(fx.path("workspace/projects/a/code/.gitignore").is_file(), "the scaffold still copies");
    pack_with(&fx, "block", "dirs: [code]", "dirs:\n  - code\n  - docs");
    fx.ok(&["new-project", "b", "--domain", "block"]);
    assert!(fx.path("workspace/projects/b/docs").is_dir());
}

#[test]
fn a_learnings_field_that_is_not_true_or_false_is_refused_naming_the_field() {
    for (i, bad) in ["learnings: yes", "learnings: 1", "learnings: ", "learnings: maybe"].iter().enumerate() {
        let fx = code_fx();
        pack_with(&fx, "badflag", "learnings: true", bad);
        refused_cleanly(&fx, "badflag", &format!("w{i}"), "learnings");
    }
}

#[test]
fn learnings_false_creates_no_log() {
    let fx = code_fx();
    pack_with(&fx, "nolog", "learnings: true", "learnings: false");
    fx.ok(&["new-project", "a", "--domain", "nolog"]);
    assert!(!fx.exists("workspace/learnings.md"));
}

#[test]
fn a_skeleton_that_cannot_render_is_refused_before_anything_is_written() {
    let fx = code_fx();
    fx.copy_pack("code", "badskel");
    fx.write("library/domains/badskel/skeleton.md", "---\nname: {nope}\n---\n");
    let (code, _, _) = fx.run(&["new-project", "w", "--domain", "badskel"]);
    assert_ne!(code, 0);
    assert!(!fx.exists("workspace/projects/w"), "a refused render left a project folder");
    assert!(!fx.exists("workspace/learnings.md"));
}

#[test]
fn doctor_reports_a_git_folder_or_file_under_a_projects_code_folder() {
    let fx = code_fx();
    for slug in ["clean", "cloned", "worktree"] {
        fx.ok(&["new-project", slug, "--domain", "code"]);
    }
    let out = fx.ok(&["doctor", "clean"]);
    assert!(out.contains("books clean"), "{out}");
    fx.write("workspace/projects/cloned/code/.git/HEAD", "ref: refs/heads/main
");
    fx.write("workspace/projects/worktree/code/sub/.git", "gitdir: ../elsewhere
");
    let (code, out, _) = fx.run(&["doctor", "cloned"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("workspace/projects/cloned/code/.git: a .git inside a project's code/ folder"), "{out}");
    let (code, out, _) = fx.run(&["doctor", "worktree"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("workspace/projects/worktree/code/sub/.git: a .git inside"), "a .git file counts: {out}");
    let (_, out, _) = fx.run(&["doctor", "clean"]);
    assert!(!out.contains(".git:"), "{out}");
}

#[test]
fn doctor_leaves_a_git_folder_inside_ignored_dependency_folders_alone() {
    let fx = code_fx();
    fx.ok(&["new-project", "deps", "--domain", "code"]);
    fx.write("workspace/projects/deps/code/node_modules/pkg/.git/HEAD", "x
");
    let out = fx.ok(&["doctor", "deps"]);
    assert!(out.contains("books clean"), "{out}");
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

#[test]
fn production_runbook_states_the_rules_a_review_asked_for() {
    let text = std::fs::read_to_string(repo_root().join("library/domains/code/production.md")).unwrap();
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "never a `.git`",
        "copy its files without `.git`",
        "start at the vault root",
        "workspace/projects/<slug>/code/tests/test_x.py",
        "Review findings go back to the same builder.",
        "fails its tests or its review is rebuilt on Opus, then on Fable",
        "it says the code is private",
        "only if the chat's own brief carries that line",
    ] {
        assert!(flat.contains(needle), "production.md lacks: {needle}");
    }
    assert!(!flat.contains("same test twice"), "models.md has no such rule");
    let pack = std::fs::read_to_string(repo_root().join("library/domains/code/pack.md")).unwrap();
    assert!(!pack.contains("holds the repository"));
    assert!(pack.contains("never a `.git`"));
}
