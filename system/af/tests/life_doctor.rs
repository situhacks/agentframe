//! The doctor additions: life.md cap, open.md grammar, stale registry rows, a remote on
//! the private repo, and stale handoffs.

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

#[test]
fn a_remote_on_the_private_repo_is_an_issue() {
    let fx = Fx::new();
    fx.write("workspace/.git/config", "[core]\n\tbare = false\n");
    assert_eq!(check(&fx), (vec![], vec![]), "no remote: silent");
    fx.write(
        "workspace/.git/config",
        "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://example.com/me/life.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
    );
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1);
    assert!(i[0].contains("remote 'origin' (https://example.com/me/life.git)"), "{}", i[0]);

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
