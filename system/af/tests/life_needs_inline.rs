//! `af needs` with the recommendation, options and reason on the line: the grammar, `add`
//! and `set`, the view's blocks, the surfaces, and doctor. Old-style items (a detail file
//! each) are in `life_needs.rs`.

mod common;
mod life_fx;
use afcore::needs::{format_line, split_need, Need, When};
use chrono::NaiveDate;
use common::*;
use life_fx::month;

const OPEN: &str = "workspace/life/open.md";
const VIEW: &str = "workspace/life/needs.md";

fn lf(s: String) -> String {
    s.replace("\r\n", "\n")
}

fn fail(fx: &Fx, args: &[&str]) -> String {
    let (code, out, err) = fx.run(args);
    assert_ne!(code, 0, "{args:?} should refuse: {out}");
    format!("{out}{err}")
}

fn issues(fx: &Fx) -> Vec<String> {
    afcore::needs::doctor_checks(&fx.ctx()).unwrap().0
}

fn notes(fx: &Fx) -> Vec<String> {
    afcore::needs::doctor_checks(&fx.ctx()).unwrap().1
}

/// `n` words.
fn words(n: usize) -> String {
    vec!["w"; n].join(" ")
}

fn old_detail(fx: &Fx, slug: &str, rec: &str) {
    fx.write(&format!("workspace/life/needs/{slug}.md"), &format!("---\nprivate: true\nrecommendation: {rec}\n---\n# Q\n\nFull notes.\n"));
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn full_need() -> Need {
    Need {
        when: When::Now,
        detail: None,
        rec: Some("the cheaper plan".to_string()),
        options: vec!["the cheaper plan, $12 a month".to_string(), "the bigger plan, $20 a month".to_string(), "stay".to_string()],
        why: Some("You use about 3 GB a month, and the cheaper plan covers 10.".to_string()),
        more: Some("knowledge/phone.md#plans".to_string()),
    }
}

const FULL: &str = "- [ ] 2026-11-05 · Which plan to take? · need now · rec: the cheaper plan · options: a) the cheaper plan, $12 a month; b) the bigger plan, $20 a month; c) stay · why: You use about 3 GB a month, and the cheaper plan covers 10. · more: knowledge/phone.md#plans · from: src-2026-11-05-01";

// ------------------------------------------------------------------ the line

#[test]
fn a_line_with_every_field_writes_and_reads_back_the_same() {
    let need = full_need();
    assert_eq!(format_line(d("2026-11-05"), "Which plan to take?", &need, "src-2026-11-05-01"), FULL);
    let (plain, parsed) = split_need(FULL);
    assert_eq!(parsed, Some(need));
    assert_eq!(plain, "- [ ] 2026-11-05 · Which plan to take? · from: src-2026-11-05-01");
}

#[test]
fn a_line_with_some_fields_or_only_a_rec_round_trips_with_from_last() {
    let mut need = Need::new(When::On(d("2026-12-01")));
    need.rec = Some("renew".to_string());
    let line = format_line(d("2026-11-05"), "Renew or move?", &need, "main");
    assert_eq!(line, "- [ ] 2026-11-05 · Renew or move? · need on 2026-12-01 · rec: renew · from: main");
    assert_eq!(split_need(&line).1, Some(need.clone()));

    need.when = When::Trigger("the quote arrives".to_string());
    need.why = Some("the quote decides it".to_string());
    let line = format_line(d("2026-11-05"), "Accept the quote?", &need, "src-9");
    assert_eq!(line, "- [ ] 2026-11-05 · Accept the quote? · need when the quote arrives · rec: renew · why: the quote decides it · from: src-9");
    assert_eq!(split_need(&line).1, Some(need.clone()));
    assert!(line.ends_with(" · from: src-9"));

    need.options = vec!["yes".to_string()];
    need.why = None;
    let line = format_line(d("2026-11-05"), "Accept?", &need, "main");
    assert!(line.contains(" · rec: renew · options: a) yes · from: main"), "{line}");
    assert_eq!(split_need(&line).1, Some(need));
}

#[test]
fn the_fields_must_come_in_order_and_a_line_with_no_rec_or_detail_is_no_need() {
    let wrong = "- [ ] 2026-11-05 · Q? · need now · why: y · rec: r · from: main";
    assert_eq!(split_need(wrong), (wrong.to_string(), None));
    let late_more = "- [ ] 2026-11-05 · Q? · need now · rec: r · more: a.md · why: y · from: main";
    assert_eq!(split_need(late_more), (late_more.to_string(), None));
    let bare = "- [ ] 2026-11-05 · Q? · need now · why: y · from: main";
    assert_eq!(split_need(bare), (bare.to_string(), None));
    let none = "- [ ] 2026-11-05 · Q? · need now · from: main";
    assert_eq!(split_need(none), (none.to_string(), None));
}

#[test]
fn an_old_style_line_still_reads_and_may_carry_a_rec_as_well() {
    let old = "- [ ] 2026-11-05 · Q? · need now · detail needs/q.md · from: main";
    let (plain, need) = split_need(old);
    let need = need.unwrap();
    assert_eq!(plain, "- [ ] 2026-11-05 · Q? · from: main");
    assert_eq!(need.detail.as_deref(), Some("needs/q.md"));
    assert_eq!(need.rec, None);
    assert_eq!(format_line(d("2026-11-05"), "Q?", &need, "main"), old);

    let both = "- [ ] 2026-11-05 · Q? · need now · detail needs/q.md · rec: r · from: main";
    let need = split_need(both).1.unwrap();
    assert_eq!((need.detail.as_deref(), need.rec.as_deref()), (Some("needs/q.md"), Some("r")));
    assert_eq!(format_line(d("2026-11-05"), "Q?", &need, "main"), both);
}

#[test]
fn the_open_md_reader_takes_a_new_line_and_reports_a_malformed_one() {
    let fx = Fx::new();
    fx.write(OPEN, &format!("# Open\n\n{FULL}\n- [ ] 2026-11-05 · Bad order · need now · why: y · rec: r · from: main\n"));
    let doc = afcore::open::OpenDoc::load(&fx.ctx()).unwrap().unwrap();
    assert_eq!(doc.loops.len(), 1);
    assert_eq!(doc.loops[0].need, Some(full_need()));
    assert_eq!(doc.bad.len(), 1, "{:?}", doc.bad);
}

// ------------------------------------------------------------------ add

#[test]
fn add_with_the_new_form_writes_the_line_and_no_file() {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n\n## Plans\n");
    let out = fx.ok(&[
        "needs", "add", "Which plan to take?", "--rec", "the cheaper plan", "--option", "the cheaper plan, $12 a month", "--option",
        "the bigger plan, $20 a month", "--option", "stay", "--why", "You use about 3 GB a month, and the cheaper plan covers 10.",
        "--more", "knowledge/phone.md#plans", "--from", "src-2026-11-05-01",
    ]);
    let line = FULL.replace("2026-11-05 · Which", &format!("{} · Which", today()));
    assert_eq!(lf(out), format!("{line}\n"));
    assert_eq!(lf(fx.read(OPEN)), format!("## Decisions needed\n\n{line}\n"));
    assert!(!fx.path("workspace/life/needs").exists(), "no file is made for a decision");
    assert_eq!(issues(&fx), Vec::<String>::new());
    assert_eq!(notes(&fx), Vec::<String>::new());
}

#[test]
fn add_needs_a_rec_and_the_old_form_still_works_with_a_note() {
    let fx = Fx::new();
    let e = fail(&fx, &["needs", "add", "No rec?"]);
    assert!(e.contains("needs --rec"), "{e}");
    assert!(!fx.path(OPEN).exists());

    old_detail(&fx, "q", "from the file");
    let (code, out, err) = fx.run(&["needs", "add", "Old form?", "--detail", "needs/q.md"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("--detail is the old form"), "{err}");
    assert!(fx.read(OPEN).contains("· Old form? · need now · detail needs/q.md · from: main"));
    assert!(fx.read(VIEW).contains("| Old form? | from the file | [open](needs/q.md) |\n"), "{}", fx.read(VIEW));
}

#[test]
fn each_cap_holds_at_its_boundary() {
    let fx = Fx::new();
    fx.ok(&["needs", "add", "Rec 12?", "--rec", &words(12)]);
    let e = fail(&fx, &["needs", "add", "Rec 13?", "--rec", &words(13)]);
    assert!(e.contains("--rec is 13 words; the cap is 12"), "{e}");

    fx.ok(&["needs", "add", "Option 30?", "--rec", "r", "--option", &words(30)]);
    let e = fail(&fx, &["needs", "add", "Option 31?", "--rec", "r", "--option", &words(31)]);
    assert!(e.contains("--option is 31 words; the cap is 30"), "{e}");

    let five: Vec<String> = (0..5).flat_map(|i| ["--option".to_string(), format!("opt {i}")]).collect();
    let mut args = vec!["needs", "add", "Five?", "--rec", "r"];
    args.extend(five.iter().map(String::as_str));
    fx.ok(&args);
    args[2] = "Six?";
    args.extend(["--option", "one more"]);
    let e = fail(&fx, &args);
    assert!(e.contains("6 options given; the cap is 5"), "{e}");

    fx.ok(&["needs", "add", "Why 60?", "--rec", "r", "--why", &words(60)]);
    let e = fail(&fx, &["needs", "add", "Why 61?", "--rec", "r", "--why", &words(61)]);
    assert!(e.contains("--why is 61 words; the cap is 60"), "{e}");
    assert_eq!(lf(fx.read(OPEN)).matches("- [ ]").count(), 4, "a refused add writes nothing");
}

#[test]
fn a_dot_separator_in_a_value_or_a_semicolon_in_an_option_is_refused_with_a_reword() {
    let fx = Fx::new();
    for args in [
        vec!["needs", "add", "Q?", "--rec", "a · b"],
        vec!["needs", "add", "Q?", "--rec", "r", "--option", "a · b"],
        vec!["needs", "add", "Q?", "--rec", "r", "--why", "a · b"],
    ] {
        let e = fail(&fx, &args);
        assert!(e.contains("may not contain ' · '") && e.contains("reword"), "{args:?}: {e}");
    }
    let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--option", "a; b"]);
    assert!(e.contains("may not contain ';'") && e.contains("reword"), "{e}");
    let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--why", "a · from: b"]);
    assert!(e.contains("--why may not contain ' · '"), "{e}");
    assert!(!fx.path(OPEN).exists());
}

#[test]
fn more_must_exist_and_may_not_sit_under_needs() {
    let fx = Fx::new();
    let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--more", "knowledge/none.md"]);
    assert!(e.contains("does not exist"), "{e}");
    fx.write("workspace/life/needs/x.md", "# x\n");
    for p in ["needs/x.md", "workspace/life/needs/x.md", "needs/closed/y.md"] {
        let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--more", p]);
        assert!(e.contains("may not point under workspace/life/needs/"), "{p}: {e}");
    }
    let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--more", "../outside.md"]);
    assert!(e.contains("a path inside the vault"), "{e}");

    // An anchor, a path under workspace/life/, and one from the vault root.
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.write("workspace/projects/p/project.md", "# P\n");
    fx.ok(&["needs", "add", "Life?", "--rec", "r", "--more", "knowledge/phone.md#plans"]);
    fx.ok(&["needs", "add", "Root?", "--rec", "r", "--more", "workspace/projects/p/project.md"]);
    let view = fx.read(VIEW);
    assert!(view.contains("| Life? | r | [more](knowledge/phone.md#plans) |\n"), "{view}");
    assert!(view.contains("| Root? | r | [more](../../workspace/projects/p/project.md) |\n"), "{view}");
}

// ------------------------------------------------------------------ set

fn set_fixture() -> (Fx, String) {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.write(OPEN, &format!("# Open loops\n\n- [ ] {t} · plain · from: main\n\n## Waiting on others\n\n- [ ] {t} · other · from: main\n", t = today()));
    fx.ok(&["needs", "add", "Which plan?", "--rec", "cheaper", "--option", "cheaper", "--option", "bigger", "--option", "stay", "--why", "usage is low"]);
    let before = lf(fx.read(OPEN));
    (fx, before)
}

#[test]
fn set_changes_only_the_named_fields_and_nothing_else_in_open_md() {
    let (fx, before) = set_fixture();
    let old_line = before.lines().find(|l| l.contains("Which plan?")).unwrap().to_string();
    fx.ok(&["needs", "set", "Which plan", "--why", "usage is about 3 GB", "--more", "knowledge/phone.md"]);
    let after = lf(fx.read(OPEN));
    let new_line = after.lines().find(|l| l.contains("Which plan?")).unwrap().to_string();
    assert_eq!(
        new_line,
        format!("- [ ] {} · Which plan? · need now · rec: cheaper · options: a) cheaper; b) bigger; c) stay · why: usage is about 3 GB · more: knowledge/phone.md · from: main", today())
    );
    assert_eq!(after, before.replace(&old_line, &new_line), "only the item's line changes");

    // The number names it too, and a rec change leaves the rest.
    let n = after.lines().filter(|l| l.starts_with("- [ ]")).position(|l| l.contains("Which plan?")).unwrap() + 1;
    fx.ok(&["needs", "set", &n.to_string(), "--rec", "stay"]);
    let line = lf(fx.read(OPEN));
    assert!(line.contains("· rec: stay · options: a) cheaper; b) bigger; c) stay · why: usage is about 3 GB · more: knowledge/phone.md · from: main"), "{line}");
    assert!(fx.read(VIEW).contains("**Which plan?** Recommended: stay."), "{}", fx.read(VIEW));
}

#[test]
fn set_option_replaces_the_whole_list_and_empty_values_clear() {
    let (fx, _) = set_fixture();
    fx.ok(&["needs", "set", "Which plan", "--option", "only this", "--option", "or that"]);
    let open = fx.read(OPEN);
    assert!(open.contains("· options: a) only this; b) or that · why: usage is low ·"), "{open}");
    fx.ok(&["needs", "set", "Which plan", "--option", "", "--why", ""]);
    let open = fx.read(OPEN);
    assert!(open.contains(" · need now · rec: cheaper · from: main"), "{open}");
}

#[test]
fn set_refuses_what_add_refuses_and_changes_nothing() {
    let (fx, before) = set_fixture();
    for args in [
        vec!["needs", "set", "Which plan"],
        vec!["needs", "set", "Which plan", "--rec", "a · b"],
        vec!["needs", "set", "Which plan", "--rec", &words(13)],
        vec!["needs", "set", "Which plan", "--why", &words(61)],
        vec!["needs", "set", "Which plan", "--option", "a; b"],
        vec!["needs", "set", "Which plan", "--more", "knowledge/none.md"],
        vec!["needs", "set", "Which plan", "--drop-detail", "--rec", "r"],
        vec!["needs", "set", "Absent item", "--rec", "r"],
    ] {
        fail(&fx, &args);
        assert_eq!(lf(fx.read(OPEN)), before, "{args:?}");
    }
    // The plain loop is not a needs item.
    let e = fail(&fx, &["needs", "set", "plain", "--rec", "r"]);
    assert!(e.contains("no needs item starts with"), "{e}");
}

#[test]
fn drop_detail_moves_the_file_and_refuses_without_a_rec_on_the_line() {
    let fx = Fx::new();
    old_detail(&fx, "q", "file rec");
    fx.ok(&["needs", "add", "Old one?", "--detail", "needs/q.md"]);
    let before = lf(fx.read(OPEN));

    // The file's recommendation is not the line's: it would be lost.
    let e = fail(&fx, &["needs", "set", "Old one", "--drop-detail"]);
    assert!(e.contains("needs a recommendation on the line"), "{e}");
    assert_eq!(lf(fx.read(OPEN)), before);
    assert!(fx.path("workspace/life/needs/q.md").exists());

    // A rec alone keeps the file, and the line now carries both.
    fx.ok(&["needs", "set", "q", "--rec", "line rec"]);
    assert!(fx.read(OPEN).contains("· detail needs/q.md · rec: line rec · from: main"), "{}", fx.read(OPEN));
    assert!(fx.read(VIEW).contains("| Old one? | line rec | [open](needs/q.md) |\n"), "the line's rec wins: {}", fx.read(VIEW));
    assert_eq!(notes(&fx).len(), 1);

    fx.ok(&["needs", "set", "Old one", "--drop-detail"]);
    assert!(fx.read(OPEN).contains(&format!("{} · Old one? · need now · rec: line rec · from: main", today())), "{}", fx.read(OPEN));
    assert!(!fx.path("workspace/life/needs/q.md").exists());
    assert!(fx.read("workspace/life/needs/closed/q.md").contains("Full notes."));
    assert_eq!(issues(&fx), Vec::<String>::new());
    assert_eq!(notes(&fx), Vec::<String>::new());

    // Given a rec in the same call, and on an item that has no file: refused.
    let e = fail(&fx, &["needs", "set", "Old one", "--drop-detail"]);
    assert!(e.contains("has no old detail file"), "{e}");
}

#[test]
fn drop_detail_with_a_rec_given_in_the_same_call() {
    let fx = Fx::new();
    old_detail(&fx, "q", "file rec");
    fx.ok(&["needs", "add", "Old one?", "--detail", "needs/q.md"]);
    fx.ok(&["needs", "set", "1", "--rec", "new rec", "--option", "yes", "--drop-detail"]);
    assert!(fx.read(OPEN).contains("· need now · rec: new rec · options: a) yes · from: main"), "{}", fx.read(OPEN));
    assert!(fx.path("workspace/life/needs/closed/q.md").exists());
}

// ------------------------------------------------------------------ the view

#[test]
fn the_view_prints_a_block_under_the_table_for_an_item_with_options_or_a_reason() {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.ok(&[
        "needs", "add", "Which plan to take?", "--rec", "the cheaper plan", "--option", "the cheaper plan, $12 a month", "--option",
        "the bigger plan, $20 a month", "--option", "stay", "--why", "You use about 3 GB a month, and the cheaper plan covers 10.", "--more",
        "knowledge/phone.md#plans",
    ]);
    fx.ok(&["needs", "add", "Bare one?", "--rec", "yes"]);
    fx.ok(&["needs", "add", "Reason only?", "--rec", "no.", "--why", "because"]);
    let view = lf(fx.read(VIEW));
    let want = "## Now (3)\n\n| Decision | Recommendation | More |\n|---|---|---|\n\
| Which plan to take? | the cheaper plan | [more](knowledge/phone.md#plans) |\n\
| Bare one? | yes |  |\n\
| Reason only? | no. |  |\n\
\n\
**Which plan to take?** Recommended: the cheaper plan.\n\
- a) the cheaper plan, $12 a month\n\
- b) the bigger plan, $20 a month\n\
- c) stay\n\
\n\
You use about 3 GB a month, and the cheaper plan covers 10.\n\
\n\
**Reason only?** Recommended: no.\n\
\n\
because\n\
\n## Do (0)";
    assert!(view.contains(want), "{view}");
    assert!(!view.contains("**Bare one?**"), "an item with neither options nor a reason has no block: {view}");
}

#[test]
fn later_has_its_own_blocks_in_table_order_and_an_empty_more_is_empty() {
    let fx = Fx::new();
    fx.ok(&["needs", "add", "Event?", "--rec", "wait", "--when", "the quote arrives", "--option", "wait", "--option", "go"]);
    fx.ok(&["needs", "add", "Soon?", "--rec", "go", "--on", "+2d", "--why", "it is cheap"]);
    fx.ok(&["needs", "add", "Quiet?", "--rec", "ok", "--on", "+9d"]);
    let view = lf(fx.read(VIEW));
    let later = view.split("## Later (3)").nth(1).unwrap();
    assert!(later.starts_with("\n\n| When | Decision | Recommendation | More |\n|---|---|---|---|\n"), "{later}");
    let (soon, event) = (later.find("**Soon?**").unwrap(), later.find("**Event?**").unwrap());
    assert!(soon < event && !later.contains("**Quiet?**"), "{later}");
    assert!(later.contains(&format!("| {} | Quiet? | ok |  |\n", days_ahead(9))), "{later}");
    assert!(view.contains("## Now (0)\n\nNothing.\n"), "{view}");
}

#[test]
fn a_pipe_in_a_new_style_cell_is_escaped_and_the_block_is_plain_text() {
    let fx = Fx::new();
    fx.ok(&["needs", "add", "A | B?", "--rec", "take a | b", "--option", "a | b", "--why", "x | y"]);
    let view = fx.read(VIEW);
    assert!(view.contains(r"| A \| B? | take a \| b |  |"), "{view}");
    assert!(lf(view.clone()).contains("**A | B?** Recommended: take a | b.\n- a) a | b\n\nx | y\n"), "{view}");
}

#[test]
fn a_more_document_that_is_gone_says_so_in_the_cell_and_doctor_reports_it() {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.ok(&["needs", "add", "Q?", "--rec", "r", "--more", "knowledge/phone.md"]);
    assert_eq!(issues(&fx), Vec::<String>::new());
    std::fs::remove_file(fx.path("workspace/life/knowledge/phone.md")).unwrap();
    fx.ok(&["needs", "render"]);
    let view = fx.read(VIEW);
    assert!(view.contains("| Q? | r | missing: knowledge/phone.md |\n") && !view.contains("[more]"), "{view}");
    let i = issues(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("'Q?'") && i[0].contains("more: knowledge/phone.md no longer exists") && i[0].contains("af needs set 1"), "{}", i[0]);
}

// ------------------------------------------------------------------ the surfaces

#[test]
fn list_session_start_and_brief_print_the_rec_from_the_line() {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    fx.ok(&["needs", "add", "Now one?", "--rec", "take it", "--option", "x", "--why", "y"]);
    fx.ok(&["needs", "add", "Linked one?", "--rec", "read it", "--more", "knowledge/phone.md"]);
    fx.ok(&["needs", "add", "Later one?", "--rec", "wait", "--when", "the quote arrives"]);
    let list = lf(fx.ok(&["needs", "list"]));
    let want = "needs you: 2 now, 0 to do, 1 later (workspace/life/needs.md)\n  \
#1 Now one? · recommend: take it\n  \
#2 Linked one? · recommend: read it · knowledge/phone.md\n  \
#3 when the quote arrives: Later one? · recommend: wait\n";
    assert_eq!(list, want);
    let bundle = lf(fx.ok(&["session", "bundle", "--session", "s-main"]));
    assert!(bundle.contains("needs you: 2 now, 0 to do, 1 later (workspace/life/needs.md)\n- Now one? · recommend: take it\n- Linked one? · recommend: read it · knowledge/phone.md"), "{bundle}");
    assert!(!bundle.contains("Later one?"), "{bundle}");
    let brief = lf(fx.ok(&["brief"]));
    assert!(brief.contains("## Needs you (2 now, 0 to do, 1 later)\n- Now one? · recommend: take it\n"), "{brief}");
}

#[test]
fn close_of_an_item_with_no_file_logs_and_has_nothing_to_move() {
    let fx = Fx::new();
    fx.ok(&["needs", "add", "Decide?", "--rec", "yes"]);
    let out = lf(fx.ok(&["needs", "close", "Decide", "--outcome", "took it"]));
    assert!(out.contains("af needs close: #1 Decide? → took it (moved to workspace/life/log/"), "{out}");
    assert!(!out.contains("detail:"), "{out}");
    assert!(!fx.path("workspace/life/needs").exists());
    assert!(!fx.read(OPEN).contains("Decide?"));
    assert!(fx.read(VIEW).contains("## Now (0)"));
    // The slug form is for old-style lines only.
    fx.ok(&["needs", "add", "Plan choice?", "--rec", "yes"]);
    let e = fail(&fx, &["needs", "close", "plan-choice", "--outcome", "x"]);
    assert!(e.contains("no needs item starts with 'plan-choice'"), "{e}");
    fx.ok(&["needs", "close", "Plan choice", "--outcome", "x"]);
}

// ------------------------------------------------------------------ doctor

#[test]
fn doctor_counts_the_old_detail_files_and_names_the_command_that_folds_one_in() {
    let fx = Fx::new();
    old_detail(&fx, "a", "yes");
    old_detail(&fx, "b", "yes");
    fx.ok(&["needs", "add", "A?", "--detail", "needs/a.md"]);
    fx.ok(&["needs", "add", "B?", "--detail", "needs/b.md"]);
    fx.ok(&["needs", "add", "C?", "--rec", "yes"]);
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("2 open needs items still keep an old detail file"), "{}", n[0]);
    assert!(n[0].contains("af needs set <n> --rec \"<a few words>\" --drop-detail"), "{}", n[0]);
    fx.ok(&["needs", "set", "A?", "--rec", "yes", "--drop-detail"]);
    assert!(notes(&fx)[0].contains("1 open needs item still keeps an old detail file"));
    fx.ok(&["needs", "set", "B?", "--rec", "yes", "--drop-detail"]);
    assert_eq!(notes(&fx), Vec::<String>::new());
}

#[test]
fn doctor_reports_a_new_style_line_over_a_cap() {
    let fx = Fx::new();
    let t = today();
    let opts: Vec<String> = (0..6).map(|i| format!("o{i}")).collect();
    let letters: Vec<String> = opts.iter().enumerate().map(|(i, o)| format!("{}) {o}", (b'a' + i as u8) as char)).collect();
    fx.write(
        OPEN,
        &format!(
            "## Decisions needed\n\n- [ ] {t} · Long rec? · need now · rec: {} · from: main\n- [ ] {t} · Long option? · need now · rec: r · options: a) {} · from: main\n- [ ] {t} · Many? · need now · rec: r · options: {} · from: main\n- [ ] {t} · Long why? · need now · rec: r · why: {} · from: main\n- [ ] {t} · Fine? · need now · rec: {} · why: {} · from: main\n",
            words(13),
            words(31),
            letters.join("; "),
            words(61),
            words(12),
            words(60)
        ),
    );
    let i = issues(&fx);
    assert_eq!(i.len(), 4, "{i:?}");
    assert!(i[0].contains("'Long rec?'") && i[0].contains("rec is 13 words; the cap is 12"), "{}", i[0]);
    assert!(i[1].contains("an option is 31 words; the cap is 30"), "{}", i[1]);
    assert!(i[2].contains("6 options; the cap is 5"), "{}", i[2]);
    assert!(i[3].contains("why is 61 words; the cap is 60"), "{}", i[3]);
}

#[test]
fn doctor_still_reports_an_orphan_file_and_an_old_item_with_no_recommendation() {
    let fx = Fx::new();
    old_detail(&fx, "orphan", "yes");
    fx.write("workspace/life/needs/bare.md", "# nothing\n");
    fx.write(OPEN, &format!("- [ ] {} · Bare? · need now · detail needs/bare.md · from: main\n- [ ] {} · Covered? · need now · detail needs/covered.md · rec: on the line · from: main\n", today(), today()));
    fx.write("workspace/life/needs/covered.md", "# the file has no recommendation, the line does\n");
    let i = issues(&fx);
    assert_eq!(i.len(), 2, "{i:?}");
    assert!(i[0].contains("bare.md: no 'recommendation:'"), "{}", i[0]);
    assert!(i[1].contains("needs/orphan.md: no open needs item names this file"), "{}", i[1]);
}

// ------------------------------------------------------------------ the review's fixes

#[test]
fn block_text_is_escaped_in_the_view_and_never_in_open_md() {
    let fx = Fx::new();
    fx.ok(&[
        "needs", "add", "Q <!-- x?", "--rec", "r > s", "--option=- not a list <b>", "--option", "1. first", "--option", "2) second", "--option", "> quote",
        "--option", "a | b", "--why", "# heading <!-- hide",
    ]);
    let view = lf(fx.read(VIEW));
    let want = "**Q \\<!-- x?** Recommended: r \\> s.\n\
- a) \\- not a list \\<b\\>\n\
- b) 1\\. first\n\
- c) 2\\) second\n\
- d) \\> quote\n\
- e) a | b\n\
\n\
\\# heading \\<!-- hide\n\
\n## Do (0)\n\nNothing.\n\n## Later (0)\n";
    assert!(view.contains(want), "{view}");
    assert!(view.contains(r"| Q \<!-- x? | r \> s |  |"), "the cell escapes too: {view}");
    // Only af's own header comment is left open-able; every other `<!--` has its backslash.
    assert_eq!(view.matches("<!--").count() - view.matches(r"\<!--").count(), 1, "no comment can open: {view}");
    let open = fx.read(OPEN);
    assert!(open.contains("· options: a) - not a list <b>; b) 1. first;") && open.contains("· why: # heading <!-- hide ·"), "the line is as written: {open}");
}

#[test]
fn a_leading_marker_is_escaped_only_at_the_start_and_a_pipe_does_not_break_a_block() {
    let fx = Fx::new();
    fx.ok(&["needs", "add", "Q?", "--rec", "r", "--option", "go - now", "--option", "#1 plan", "--option", "10 plans", "--why", "a | b # c"]);
    let view = lf(fx.read(VIEW));
    assert!(view.contains("- a) go - now\n- b) \\#1 plan\n- c) 10 plans\n\na | b # c\n"), "{view}");
    assert!(view.contains("## Do (0)") && view.contains("## Later (0)"));
}

#[test]
fn drop_detail_says_where_the_file_went_and_logs_it_even_after_a_name_clash() {
    let fx = Fx::new();
    old_detail(&fx, "q", "file rec");
    fx.write("workspace/life/needs/closed/q.md", "# an earlier one\n");
    fx.ok(&["needs", "add", "Old one?", "--detail", "needs/q.md"]);
    let out = lf(fx.ok(&["needs", "set", "Old one", "--rec", "line rec", "--drop-detail"]));
    assert!(out.contains("moved to workspace/life/needs/closed/q-2.md"), "{out}");
    assert!(out.contains("(logged in workspace/life/log/"), "{out}");
    assert!(fx.read("workspace/life/needs/closed/q-2.md").contains("Full notes."));
    assert!(fx.read("workspace/life/needs/closed/q.md").contains("an earlier one"));
    let log = fx.read(&log_rel());
    assert!(log.contains("folded: Old one? → old detail file folded into the line (detail: needs/closed/q-2.md)"), "{log}");
}

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

#[test]
fn a_failed_move_in_drop_detail_changes_nothing_and_the_message_is_true_for_set() {
    let fx = Fx::new();
    old_detail(&fx, "q", "file rec");
    fx.ok(&["needs", "add", "Old one?", "--detail", "needs/q.md"]);
    fx.write("workspace/life/needs/closed", "a file where the folder belongs\n");
    let before = lf(fx.read(OPEN));
    let e = fail(&fx, &["needs", "set", "Old one", "--rec", "line rec", "--drop-detail"]);
    assert!(e.contains("could not move") && e.contains("nothing was changed") && e.contains("still names it"), "{e}");
    assert!(!e.contains("closed and logged"), "{e}");
    assert_eq!(lf(fx.read(OPEN)), before, "the line keeps its detail reference, and its rec");
    assert!(fx.path("workspace/life/needs/q.md").exists());
    assert!(!fx.path(&log_rel()).exists(), "nothing is logged for a change that did not happen");
}

#[test]
fn doctor_says_what_to_do_with_a_file_no_open_item_names() {
    let fx = Fx::new();
    old_detail(&fx, "orphan", "yes");
    let i = issues(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("no open needs item names this file; move it to needs/closed/ or delete it"), "{}", i[0]);
    assert!(!i[0].contains("af needs set"), "there is no item to fold it into: {}", i[0]);
}

#[test]
fn more_refuses_an_absolute_path_and_reads_dot_segments_before_the_needs_check() {
    let fx = Fx::new();
    fx.write("workspace/life/needs/a.md", "# a\n");
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    for p in ["D:/x.md", "D:\\x.md", "c:x.md", "//server/share/x.md", "\\\\server\\share\\x.md", "/x.md"] {
        let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--more", p]);
        assert!(e.contains("a path inside the vault"), "{p}: {e}");
    }
    for p in ["workspace/life/./needs/a.md", "./needs/./a.md", "workspace/./life/needs/./closed/a.md"] {
        let e = fail(&fx, &["needs", "add", "Q?", "--rec", "r", "--more", p]);
        assert!(e.contains("may not point under workspace/life/needs/"), "{p}: {e}");
    }
    assert!(!fx.path(OPEN).exists());
    fx.ok(&["needs", "add", "Q?", "--rec", "r", "--more", "./knowledge/./phone.md#a"]);
    assert!(fx.read(OPEN).contains("· more: knowledge/phone.md#a · from: main"), "{}", fx.read(OPEN));
}

#[test]
fn a_one_character_value_is_not_swallowed_by_the_fields_after_it() {
    // A one-character rec, then other fields: the lazy forms keep each value to itself.
    let line = "- [ ] 2026-11-05 · Q? · need now · rec: b · options: a) x; b) y · why: z · from: main";
    let need = split_need(line).1.expect("a need");
    assert_eq!(need.rec.as_deref(), Some("b"));
    assert_eq!(need.options, vec!["x".to_string(), "y".to_string()]);
    assert_eq!(need.why.as_deref(), Some("z"));
    // A one-character trigger.
    let line = "- [ ] 2026-11-05 · Q? · need when x · rec: ab · from: main";
    let (plain, need) = split_need(line);
    let need = need.expect("a need");
    assert_eq!((need.when, need.rec.as_deref()), (When::Trigger("x".to_string()), Some("ab")));
    assert_eq!(plain, "- [ ] 2026-11-05 · Q? · from: main");
    let line = "- [ ] 2026-11-05 · Q? · need when x · detail needs/q.md · from: main";
    assert_eq!(split_need(line).1.unwrap().when, When::Trigger("x".to_string()));
}

#[test]
fn a_question_with_a_field_separator_is_refused_for_its_question() {
    let fx = Fx::new();
    for t in ["Q · rec: x?", "Q · options: x?", "Q · why: x?", "Q · more: x?", "Q · detail x?", "Q · need x?"] {
        let e = fail(&fx, &["needs", "add", t, "--rec", "r"]);
        assert!(e.contains("the question may not contain"), "{t}: {e}");
    }
    assert!(!fx.path(OPEN).exists());
}

#[test]
fn brief_json_gives_more_as_a_vault_relative_path() {
    let fx = Fx::new();
    fx.write("workspace/life/knowledge/phone.md", "# Phone\n");
    fx.write("workspace/projects/p/project.md", "# P\n");
    fx.ok(&["needs", "add", "Life?", "--rec", "r", "--more", "knowledge/phone.md#plans"]);
    fx.ok(&["needs", "add", "Root?", "--rec", "r", "--more", "workspace/projects/p/project.md"]);
    fx.ok(&["needs", "add", "None?", "--rec", "r"]);
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["brief", "--json"])).unwrap();
    let now = v["needs"]["now"].as_array().unwrap();
    assert_eq!(now[0]["more"], "workspace/life/knowledge/phone.md#plans");
    assert_eq!(now[1]["more"], "workspace/projects/p/project.md");
    assert!(now[2]["more"].is_null());
    // The line itself keeps what was given.
    assert!(fx.read(OPEN).contains("· more: knowledge/phone.md#plans ·"));
}

#[test]
fn the_hint_for_a_need_line_that_does_not_parse_names_both_forms_new_first() {
    let fx = Fx::new();
    fx.write(OPEN, "- [ ] 2026-11-05 · Bad order · need now · why: y · rec: r · from: main\n");
    let doc = afcore::open::OpenDoc::load(&fx.ctx()).unwrap().unwrap();
    let why = &doc.bad[0].2;
    let (new, old) = (why.find("· rec:").expect(why), why.find("· detail needs/<slug>.md").expect(why));
    assert!(new < old && why.contains("old form"), "{why}");
}
