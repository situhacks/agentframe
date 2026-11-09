//! The doctor additions: life.md cap, open.md grammar, stale registry rows, a remote on
//! the private repo, stale handoffs, and source-id collisions.

mod common;
mod life_fx;
use afcore::life::doctor_checks;
use common::*;
use life_fx::*;

fn check(fx: &Fx) -> (Vec<String>, Vec<String>) {
    doctor_checks(&fx.ctx()).unwrap()
}

fn life_with(fx: &Fx, cap: Option<usize>, lines: usize) {
    let fm = cap.map(|c| format!("---\ncap_lines: {c}\n---\n")).unwrap_or_default();
    let body: String = (0..lines).map(|i| format!("- fact {i}\n")).collect();
    fx.write("workspace/life/life.md", &format!("{fm}{body}"));
}

#[test]
fn an_empty_root_is_quiet() {
    let fx = Fx::new();
    assert_eq!(check(&fx), (vec![], vec![]));
}

#[test]
fn life_cap_is_an_issue_over_and_a_note_at_ninety_percent() {
    let fx = Fx::new();
    life_with(&fx, Some(200), 201);
    let (i, n) = check(&fx);
    assert!(any_contains(&i, "life.md: body 201 lines, over its cap of 200"), "{i:?}");
    assert!(n.is_empty());

    life_with(&fx, Some(200), 185);
    let (i, n) = check(&fx);
    assert!(i.is_empty());
    assert!(any_contains(&n, "life.md: body 185 of 200 lines"), "{n:?}");

    life_with(&fx, None, 150);
    assert_eq!(check(&fx), (vec![], vec![]), "the cap defaults to 200");

    life_with(&fx, Some(10), 11);
    assert!(any_contains(&check(&fx).0, "over its cap of 10"));
}

#[test]
fn open_md_grammar_breaks_are_issues_with_line_numbers() {
    let fx = Fx::new();
    fx.write(
        "workspace/life/open.md",
        "# Open loops\n\nProse is fine here.\n\n## Waiting on the operator\n\n\
- [ ] 2026-10-24 · good · nudge 2026-10-27 · from: main\n\
- [ ] 24/10/2026 · bad date · from: main\n\
- [ ] 2026-10-24 · no source\n\
- [x] 2026-10-24 · done · from: main · closed 2026-10-25: ok\n\
<!-- - not a loop -->\n",
    );
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 3, "{i:?}");
    assert!(i[0].starts_with("workspace/life/open.md:8: line does not parse"), "{}", i[0]);
    assert!(i[1].starts_with("workspace/life/open.md:9: "));
    assert!(i[2].starts_with("workspace/life/open.md:10: closed loop left in open.md"));
}

#[test]
fn idle_rows_older_than_a_week_are_notes() {
    let fx = Fx::new();
    write_registry(
        &fx,
        &[
            row("project/old", "operator", "s1", &ts(-8 * 24 * 60), "idle"),
            row("worker/stale-live", "worker", "s2", &ts(-10 * 24 * 60), "live"),
            row("project/recent", "operator", "s3", &ts(-2 * 24 * 60), "idle"),
            row("worker/ended", "worker", "s4", &ts(-30 * 24 * 60), "ended 2026-09-01T10:00 exit"),
            row("main", "main", "s5", &ts(-1), "live"),
        ],
    );
    let (i, n) = check(&fx);
    assert!(i.is_empty());
    assert_eq!(n.len(), 2, "{n:?}");
    assert!(n[0].contains("'project/old' idle 8d") && n[0].contains("af session end project/old --reason other"));
    assert!(n[1].contains("'worker/stale-live' idle 10d"));
}

fn remotes(fx: &Fx, rows: &[(&str, &str)]) {
    let mut s = String::from("[core]\n\tbare = false\n");
    for (name, url) in rows {
        s.push_str(&format!("[remote \"{name}\"]\n\turl = {url}\n\tfetch = +refs/heads/*:refs/remotes/{name}/*\n"));
    }
    fx.write("workspace/.git/config", &s);
}

#[test]
fn the_private_repo_keeps_no_remote_or_one_folder_hub() {
    let fx = Fx::new();
    remotes(&fx, &[]);
    assert_eq!(check(&fx), (vec![], vec![]), "no remote: silent");

    for url in [r"\\laptop\agentframe-hub\workspace.git", "//laptop/agentframe-hub/workspace.git", "D:/agentframe-hub/workspace.git"] {
        remotes(&fx, &[("hub", url)]);
        assert_eq!(check(&fx), (vec![], vec![]), "a folder hub is the supported remote: {url}");
    }

    remotes(&fx, &[("origin", "https://github.com/someone/life.git")]);
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("remote 'origin' (https://github.com/someone/life.git)"), "{}", i[0]);
    assert!(i[0].contains("git -C workspace remote remove origin") && i[0].contains("af sync init"), "the message names the fix: {}", i[0]);

    for url in ["git@github.com:someone/life.git", "https://github.com/someone/life.git", "ssh://laptop/srv/workspace.git"] {
        remotes(&fx, &[("hub", url)]);
        let (i, _) = check(&fx);
        assert_eq!(i.len(), 1, "a web or SSH hub is an issue: {url}");
        assert!(i[0].contains("not a folder path") && i[0].contains("af sync init"), "{}", i[0]);
    }

    // A folder url with a push address elsewhere sends the private layer there.
    fx.write(
        "workspace/.git/config",
        "[core]\n\tbare = false\n[remote \"hub\"]\n\turl = D:/agentframe-hub/workspace.git\n\tpushurl = git@github.com:someone/life.git\n",
    );
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("git@github.com:someone/life.git") && i[0].contains("pushurl"), "{}", i[0]);

    remotes(&fx, &[("hub", "D:/agentframe-hub/workspace.git"), ("backup", "E:/elsewhere/workspace.git")]);
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1, "only the hub is allowed: {i:?}");
    assert!(i[0].contains("remote 'backup'"), "{}", i[0]);

    remotes(&fx, &[("origin", "https://github.com/someone/life.git")]);
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("remote 'origin'"));
}

#[test]
fn a_handoff_older_than_its_folder_is_a_note() {
    let fx = Fx::new();
    let h = "system/sessions/system/cut/handoff.md";
    fx.write(h, &handoff(&ts(60), &[]));
    fx.write("system/sessions/system/cut/report.md", "result");
    assert_eq!(check(&fx), (vec![], vec![]));

    fx.write(h, &handoff("2026-01-01T00:00", &[]));
    let (_, n) = check(&fx);
    assert_eq!(n.len(), 1);
    assert!(n[0].starts_with("system/sessions/system/cut/handoff.md: written 2026-01-01T00:00 is older"), "{}", n[0]);

    fx.write(h, "no frontmatter\n");
    assert!(any_contains(&check(&fx).1, "no parseable 'written' stamp"));
}

fn order(fx: &Fx, name: &str, id: &str) {
    fx.write(&format!("workspace/life/sources/orders/{name}"), &format!("---\nid: {id}\ntype: order\n---\nverbatim\n"));
}

fn index(fx: &Fx, rows: &[(&str, &str)]) {
    let body: String = rows.iter().map(|(id, f)| format!("| `{id}` | [`{f}`]({f}) | 2026-10-25 | one line |\n")).collect();
    fx.write("workspace/life/sources/INDEX.md", &format!("# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n{body}"));
}

#[test]
fn a_clean_sources_tree_is_quiet() {
    let fx = Fx::new();
    order(&fx, "order-2026-10-25-01-dump.md", "src-2026-10-25-01");
    fx.write("workspace/life/sources/document-2026-10-25-snapshot.pdf", "%PDF");
    fx.write("workspace/life/sources/research/2026-10-25-x.md", "---\nid: research-2026-10-25-x\n---\nnotes\n");
    fx.write("workspace/life/sources/inbox/order-2026-10-25-later.md", "no frontmatter yet\n");
    index(
        &fx,
        &[
            ("src-2026-10-25-01", "orders/order-2026-10-25-01-dump.md"),
            ("src-2026-10-25-02", "document-2026-10-25-snapshot.pdf"),
        ],
    );
    assert_eq!(check(&fx), (vec![], vec![]));
}

/// The 2026-10-25 collision: the filer took -02 for a PDF, a side chat filed an order as
/// -02 too, and another order named -02- carried id -04.
#[test]
fn duplicate_source_ids_and_order_name_mismatches_are_issues() {
    let fx = Fx::new();
    fx.write("workspace/life/sources/document-2026-10-25-snapshot.pdf", "%PDF");
    order(&fx, "order-2026-10-25-02-gmail.md", "src-2026-10-25-02");
    order(&fx, "order-2026-10-25-02-fix.md", "src-2026-10-25-04");
    order(&fx, "order-2026-10-25-dump.md", "src-2026-10-25-01");
    index(
        &fx,
        &[
            ("src-2026-10-25-01", "orders/order-2026-10-25-dump.md"),
            ("src-2026-10-25-02", "document-2026-10-25-snapshot.pdf"),
            ("src-2026-10-25-04", "orders/order-2026-10-25-02-fix.md"),
            ("src-2026-10-25-04", "orders/order-2026-10-25-02-fix.md"),
        ],
    );
    let (i, n) = check(&fx);
    assert_eq!(i.len(), 4, "{i:?}");
    assert!(
        any_contains(
            &i,
            "workspace/life/sources: id src-2026-10-25-02 names 2 files: document-2026-10-25-snapshot.pdf (INDEX.md:6), orders/order-2026-10-25-02-gmail.md (frontmatter)"
        ),
        "{i:?}"
    );
    assert!(any_contains(&i, "workspace/life/sources/INDEX.md: id src-2026-10-25-04 has 2 rows (lines 7, 8)"), "{i:?}");
    assert!(
        any_contains(&i, "workspace/life/sources/orders/order-2026-10-25-02-fix.md: id src-2026-10-25-04 but the name carries 2026-10-25-02"),
        "{i:?}"
    );
    assert!(
        any_contains(&i, "workspace/life/sources/orders/order-2026-10-25-dump.md: id src-2026-10-25-01 but the name carries no date-NN"),
        "{i:?}"
    );
    assert!(n.is_empty(), "{n:?}");

    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("id src-2026-10-25-02 names 2 files"), "{out}");
}

#[test]
fn a_source_id_missing_from_the_index_is_a_note() {
    let fx = Fx::new();
    order(&fx, "order-2026-10-25-01-dump.md", "src-2026-10-25-01");
    order(&fx, "order-2026-10-25-02-more.md", "src-2026-10-25-02");
    index(&fx, &[("src-2026-10-25-01", "orders/order-2026-10-25-01-dump.md")]);
    let (i, n) = check(&fx);
    assert!(i.is_empty(), "{i:?}");
    assert_eq!(
        n,
        vec!["workspace/life/sources/orders/order-2026-10-25-02-more.md: id src-2026-10-25-02 has no INDEX.md row; add one".to_string()]
    );
}
