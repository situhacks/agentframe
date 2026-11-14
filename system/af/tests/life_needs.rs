//! `af needs` with old-style items (a detail file each): marked loops in open.md, their detail
//! files, the generated view, and the doctor, session-start, and brief surfaces. The new form,
//! with the fields on the line, is in `life_needs_inline.rs`.

mod common;
mod life_fx;
use common::*;
use life_fx::*;

const OPEN: &str = "workspace/life/open.md";
const VIEW: &str = "workspace/life/needs.md";
const NOW_HEAD: &str = "| Decision | Recommendation | More |\n|---|---|---|\n";
const DO_HEAD: &str = "| To do | Due | Waiting since |\n|---|---|---|\n";
const LATER_HEAD: &str = "| When | Decision | Recommendation | More |\n|---|---|---|---|\n";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn detail(fx: &Fx, slug: &str, rec: &str) {
    fx.write(
        &format!("workspace/life/needs/{slug}.md"),
        &format!("---\nprivate: true\nrecommendation: {rec}\n---\n# Question\n\nFull notes.\n"),
    );
}

/// An item with its recommendation on the line and no detail file.
fn with_inline(q: &str, rec: &str) -> Fx {
    let fx = Fx::new();
    fx.ok(&["needs", "add", q, "--rec", rec]);
    fx
}

fn with_item(slug: &str, q: &str, rec: &str) -> Fx {
    let fx = Fx::new();
    detail(&fx, slug, rec);
    fx.ok(&["needs", "add", q, "--detail", &format!("needs/{slug}.md")]);
    fx
}

/// Stdout with the platform's line ends normalized.
fn lf(s: String) -> String {
    s.replace("\r\n", "\n")
}

/// A refused button: its output, with the nonzero exit asserted.
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

#[test]
fn add_marks_a_loop_in_its_own_section_and_writes_the_view() {
    let fx = with_item("plan-choice", "Which plan to take?", "the cheaper plan");
    let line = format!("- [ ] {} · Which plan to take? · need now · detail needs/plan-choice.md · from: main", today());
    assert_eq!(fx.read(OPEN), format!("## Decisions needed\n\n{line}\n"));
    let view = fx.read(VIEW);
    assert!(view.contains(&format!("## Now (1)\n\n{NOW_HEAD}| Which plan to take? | the cheaper plan | [open](needs/plan-choice.md) |\n")), "{view}");
    assert!(view.contains("## Later (0)\n\nNothing.\n"), "{view}");
}

#[test]
fn add_accepts_the_vault_path_and_refuses_what_would_break_the_list() {
    let fx = Fx::new();
    detail(&fx, "a", "yes");
    fx.ok(&["needs", "add", "First?", "--detail", "workspace/life/needs/a.md", "--from", "src-1"]);
    assert!(fx.read(OPEN).contains("· detail needs/a.md · from: src-1"));

    let e = fail(&fx, &["needs", "add", "Again?", "--detail", "needs/a.md"]);
    assert!(e.contains("already belongs to open item #1"), "{e}");
    let e = fail(&fx, &["needs", "add", "Absent?", "--detail", "needs/none.md"]);
    assert!(e.contains("does not exist; write the detail file first"), "{e}");
    let e = fail(&fx, &["needs", "add", "Elsewhere?", "--detail", "workspace/life/a.md"]);
    assert!(e.contains("--detail takes needs/<kebab-slug>.md"), "{e}");

    fx.write("workspace/life/needs/bare.md", "# No frontmatter\n");
    let e = fail(&fx, &["needs", "add", "Bare?", "--detail", "needs/bare.md"]);
    assert!(e.contains("no 'recommendation:'"), "{e}");

    detail(&fx, "b", "yes");
    let e = fail(&fx, &["needs", "add", "Bad · nudge 2030-01-01 token", "--detail", "needs/b.md"]);
    assert!(e.contains("breaks the open.md grammar"), "{e}");
    let e = fail(&fx, &["needs", "add", "Bad?", "--detail", "needs/b.md", "--on", "soon"]);
    assert!(e.contains("--on takes YYYY-MM-DD or +Nd"), "{e}");
    let e = fail(&fx, &["needs", "add", "Bad?", "--detail", "needs/b.md", "--on", "+3d", "--when", "x"]);
    assert!(e.contains("cannot be used with"), "{e}");
}

#[test]
fn later_groups_by_date_then_trigger_and_a_passed_date_is_now() {
    let fx = Fx::new();
    for s in ["soon", "far", "event", "past"] {
        detail(&fx, s, &format!("rec {s}"));
    }
    fx.ok(&["needs", "add", "Event?", "--detail", "needs/event.md", "--when", "the quote arrives"]);
    fx.ok(&["needs", "add", "Far?", "--detail", "needs/far.md", "--on", "+30d"]);
    fx.ok(&["needs", "add", "Soon?", "--detail", "needs/soon.md", "--on", "+2d"]);
    fx.ok(&["needs", "add", "Past?", "--detail", "needs/past.md", "--on", &days_ago(1)]);
    let view = fx.read(VIEW);
    let later = view.split("## Later (3)").nth(1).expect("three later items");
    let (s, f, e) = (later.find("Soon?").unwrap(), later.find("Far?").unwrap(), later.find("Event?").unwrap());
    assert!(s < f && f < e, "{later}");
    assert!(later.starts_with(&format!("\n\n{LATER_HEAD}| {} | Soon? | rec soon | [open](needs/soon.md) |\n", days_ahead(2))), "{later}");
    assert!(later.contains("| the quote arrives | Event? | rec event | [open](needs/event.md) |\n"), "{later}");
    assert!(view.contains(&format!("## Now (1)\n\n{NOW_HEAD}| Past? | rec past | [open](needs/past.md) |\n")), "{view}");
}

#[test]
fn the_view_moves_a_passed_date_to_now_on_render_without_touching_open_md() {
    let fx = Fx::new();
    detail(&fx, "d", "rec");
    fx.write(OPEN, &format!("# Open loops\n\n- [ ] {} · Dated? · need on {} · detail needs/d.md · from: main\n", days_ago(9), days_ago(1)));
    let before = fx.read(OPEN);
    fx.ok(&["needs", "render"]);
    assert!(fx.read(VIEW).contains(&format!("## Now (1)\n\n{NOW_HEAD}| Dated? | rec | [open](needs/d.md) |\n")), "{}", fx.read(VIEW));
    assert_eq!(fx.read(OPEN), before, "the state lives on the line; only the view changes");
}

#[test]
fn a_wikilink_in_a_question_stays_as_written_and_the_link_is_only_in_notes() {
    let fx = with_item("q", "Ask [[a-friend]] or not?", "ask");
    assert!(fx.read(VIEW).contains("| Ask [[a-friend]] or not? | ask | [open](needs/q.md) |\n"), "{}", fx.read(VIEW));
}

#[test]
fn an_aliased_wikilink_in_the_view_is_no_false_doctor_issue() {
    // A table needs the pipe escaped, and `[[slug\|alias]]` is how Obsidian reads one there.
    let fx = with_item("q", "Ask [[a-friend|the friend]] or not?", "ask");
    assert!(fx.read(VIEW).contains(r"| Ask [[a-friend\|the friend]] or not? | ask |"), "{}", fx.read(VIEW));
    fx.write("workspace/life/people/a-friend.md", "# A friend\n");
    let dangling = afcore::tags::dangling_link_issues(&fx.ctx()).unwrap();
    assert_eq!(dangling, Vec::<String>::new());
}

#[test]
fn an_aliased_wikilink_with_no_page_is_still_flagged_by_its_real_slug() {
    let fx = with_item("q", "Ask [[no-such-page|the friend]] or not?", "ask");
    let dangling = afcore::tags::dangling_link_issues(&fx.ctx()).unwrap();
    assert!(dangling.iter().any(|i| i.contains("needs.md: [[no-such-page]] has no page")), "{dangling:?}");
    assert!(!dangling.iter().any(|i| i.contains('\\')), "{dangling:?}");
}

#[test]
fn a_pipe_in_any_cell_is_escaped_so_it_cannot_split_the_row() {
    let fx = Fx::new();
    detail(&fx, "a", "take a | b");
    detail(&fx, "b", "wait");
    fx.ok(&["needs", "add", "Plan a | plan b?", "--detail", "needs/a.md"]);
    fx.ok(&["needs", "add", "Later | one?", "--detail", "needs/b.md", "--when", "price | stock moves"]);
    fx.ok(&["open", "add", "Do x | y", "--nudge", &today()]);
    let view = fx.read(VIEW);
    assert!(view.contains(r"| Plan a \| plan b? | take a \| b | [open](needs/a.md) |"), "{view}");
    assert!(view.contains(r"| price \| stock moves | Later \| one? | wait | [open](needs/b.md) |"), "{view}");
    assert!(view.contains(&format!(r"| Do x \| y | {t} | {t} |", t = today())), "{view}");
}

#[test]
fn a_cell_stays_on_one_line_and_a_backslash_cannot_undo_the_escape() {
    use afcore::needs::cell;
    assert_eq!(cell("two\nlines\r\nhere"), "two lines here");
    assert_eq!(cell("para\u{2028}sep"), "para sep");
    assert_eq!(cell(r"ends with \| pipe"), r"ends with \\\| pipe");
    assert_eq!(cell(r"C:\temp"), r"C:\\temp");
}

#[test]
fn a_missing_detail_file_says_so_in_notes_with_no_dead_link() {
    let fx = Fx::new();
    fx.write(OPEN, &format!("- [ ] {} · Gone? · need now · detail needs/gone.md · from: main\n", today()));
    fx.ok(&["needs", "render"]);
    let view = fx.read(VIEW);
    assert!(view.contains("| Gone? | no recommendation written | detail file missing: needs/gone.md |\n"), "{view}");
    assert!(!view.contains("[open]"), "{view}");
}

#[test]
fn needs_close_logs_the_outcome_keeps_the_notes_and_rewrites_the_view() {
    let fx = with_item("plan-choice", "Which plan to take?", "the cheaper plan");
    fx.ok(&["needs", "close", "plan-choice", "--outcome", "took the cheaper plan"]);
    assert!(!fx.read(OPEN).contains("Which plan"), "the line leaves open.md");
    let log = fx.read(&log_rel());
    assert!(log.contains("closed: Which plan to take? → took the cheaper plan (detail: needs/closed/plan-choice.md)"), "{log}");
    assert!(!fx.path("workspace/life/needs/plan-choice.md").exists());
    assert!(fx.read("workspace/life/needs/closed/plan-choice.md").contains("Full notes."));
    assert!(fx.read(VIEW).contains("## Now (0)"), "{}", fx.read(VIEW));
    assert_eq!(issues(&fx), Vec::<String>::new());
}

#[test]
fn a_closed_detail_never_overwrites_an_earlier_one() {
    let fx = with_item("q", "First time?", "yes");
    fx.ok(&["needs", "close", "q", "--outcome", "done"]);
    detail(&fx, "q", "again");
    fx.ok(&["needs", "add", "Second time?", "--detail", "needs/q.md"]);
    fx.ok(&["needs", "close", "q", "--outcome", "done again"]);
    assert!(fx.read("workspace/life/needs/closed/q.md").contains("recommendation: yes"));
    assert!(fx.read("workspace/life/needs/closed/q-2.md").contains("recommendation: again"));
}

#[test]
fn open_close_on_a_marked_loop_does_the_same_and_an_unmarked_loop_leaves_the_view_alone() {
    let fx = with_item("q", "Decide?", "yes");
    fx.ok(&["open", "add", "plain loop"]);
    let view = fx.read(VIEW);
    fx.ok(&["open", "close", "plain", "--outcome", "done"]);
    assert_eq!(fx.read(VIEW), view);
    fx.ok(&["open", "close", "Decide", "--outcome", "decided"]);
    assert!(fx.read(VIEW).contains("## Now (0)"));
    assert!(fx.path("workspace/life/needs/closed/q.md").exists());
    assert_eq!(issues(&fx), Vec::<String>::new());
}

#[test]
fn needs_close_takes_only_a_needs_item() {
    let fx = with_item("q", "Decide?", "yes");
    fx.ok(&["open", "add", "Decide the plain one"]);
    // #2 is the plain loop; the prefix resolves among needs items alone.
    let e = fail(&fx, &["needs", "close", "2", "--outcome", "x"]);
    assert!(e.contains("no needs item #2"), "{e}");
    fx.ok(&["needs", "close", "Decide", "--outcome", "decided"]);
    assert!(fx.read(OPEN).contains("Decide the plain one"));
}

#[test]
fn move_changes_the_state_in_place_and_logs_it() {
    let fx = Fx::new();
    detail(&fx, "q", "yes");
    fx.ok(&["needs", "add", "Decide?", "--detail", "needs/q.md", "--when", "the quote arrives", "--from", "src-7"]);
    assert!(fx.read(VIEW).contains("## Now (0)"));
    fx.ok(&["needs", "move", "q", "--now"]);
    assert!(fx.read(OPEN).contains("· Decide? · need now · detail needs/q.md · from: src-7"));
    assert!(fx.read(VIEW).contains("## Now (1)"));
    assert!(fx.read(&log_rel()).contains("redated: Decide? → need now (was need when the quote arrives)"));
    let e = fail(&fx, &["needs", "move", "q"]);
    assert!(e.contains("--now, --on <date>, or --when <trigger>"), "{e}");
}

#[test]
fn open_redate_refuses_a_marked_loop_and_an_undated_one_is_never_due() {
    let fx = with_item("q", "Decide?", "yes");
    let e = fail(&fx, &["open", "redate", "Decide", "--nudge", "+3d"]);
    assert!(e.contains("use af needs move"), "{e}");
    assert_eq!(fx.ok(&["open", "due"]), "");
}

#[test]
fn unmarked_loops_parse_as_before_and_a_malformed_mark_is_reported() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\n\
- [ ] {t} · plain · nudge {t} · every weekly · from: main\n\
- [ ] {t} · words like need now and detail stay text · from: main\n\
- [ ] {t} · bad state · need soon · detail needs/a.md · from: main\n\
- [ ] {t} · bad date · need on 2030-13-45 · detail needs/a.md · from: main\n\
- [ ] {t} · bad slug · need now · detail needs/A B.md · from: main\n\
- [ ] {t} · no detail · need now · from: main\n\
- [ ] {t} · both · nudge {t} · need now · detail needs/a.md · from: main\n",
            t = today()
        ),
    );
    let doc = afcore::open::OpenDoc::load(&fx.ctx()).unwrap().unwrap();
    assert_eq!(doc.loops.len(), 2, "{:?}", doc.loops);
    assert_eq!(doc.bad.len(), 5, "{:?}", doc.bad);
}

#[test]
fn doctor_flags_a_missing_detail_an_orphan_a_shared_detail_and_no_recommendation() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "- [ ] {t} · Gone? · need now · detail needs/gone.md · from: main\n\
- [ ] {t} · Shared one? · need now · detail needs/shared.md · from: main\n\
- [ ] {t} · Shared two? · need now · detail needs/shared.md · from: main\n\
- [ ] {t} · Bare? · need now · detail needs/bare.md · from: main\n",
            t = today()
        ),
    );
    detail(&fx, "shared", "yes");
    detail(&fx, "orphan", "yes");
    fx.write("workspace/life/needs/bare.md", "# nothing\n");
    fx.write("workspace/life/needs/closed/old.md", "# closed items are not orphans\n");
    let i = issues(&fx);
    assert_eq!(i.len(), 4, "{i:?}");
    assert!(i[0].contains("needs item 'Gone?' names workspace/life/needs/gone.md, which does not exist"), "{}", i[0]);
    assert!(i[1].contains("two needs items share"), "{}", i[1]);
    assert!(i[2].contains("bare.md: no 'recommendation:'"), "{}", i[2]);
    assert!(i[3].contains("needs/orphan.md: no open needs item uses this detail file"), "{}", i[3]);
}

#[test]
fn doctor_reports_through_the_verb_and_stays_quiet_on_a_clean_list() {
    let fx = with_inline("Decide?", "yes");
    assert_eq!(issues(&fx), Vec::<String>::new());
    assert_eq!(notes(&fx), Vec::<String>::new());
    let fx = with_item("q", "Decide?", "yes");
    assert_eq!(issues(&fx), Vec::<String>::new());
    std::fs::remove_file(fx.path("workspace/life/needs/q.md")).unwrap();
    let (_, stdout, _) = fx.run(&["doctor"]);
    assert!(stdout.contains("needs item 'Decide?'"), "{stdout}");
}

#[test]
fn any_loop_read_overwrites_a_hand_edit_of_the_view() {
    let fx = with_inline("Decide?", "yes");
    fx.write(VIEW, "# hand edit\n");
    fx.ok(&["open", "due"]);
    assert!(fx.read(VIEW).contains("## Now (1)"), "{}", fx.read(VIEW));
    assert_eq!(notes(&fx), Vec::<String>::new());
}

#[test]
fn a_view_af_cannot_write_is_noted_and_never_stops_a_loop_verb() {
    let fx = with_inline("Decide?", "yes");
    std::fs::remove_file(fx.path(VIEW)).unwrap();
    std::fs::create_dir_all(fx.path(VIEW)).unwrap();
    fx.ok(&["open", "due"]);
    fx.ok(&["open", "add", "still works"]);
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("af could not rewrite it"), "{}", n[0]);
    fail(&fx, &["needs", "render"]);
}

#[test]
fn a_non_utf8_view_does_not_crash_doctor_and_is_replaced() {
    let fx = with_item("q", "Decide?", "yes");
    fx.write_bytes(VIEW, &[0xFF, 0xFE, 0x00, 0x80]);
    let (code, out, err) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(fx.read(VIEW).contains("## Now (1)"));
}

#[test]
fn a_block_scalar_recommendation_reads_as_its_text() {
    for (raw, want) in [
        ("recommendation: >\n  the cheaper\n  plan\n", "the cheaper plan"),
        ("recommendation: >-\n  the cheaper plan\n", "the cheaper plan"),
        ("recommendation: |\n  the cheaper\n  plan\n", "the cheaper plan"),
        ("recommendation: \"quoted: with a colon\"\n", "quoted: with a colon"),
        ("recommendation: plain: with a colon\n", "plain: with a colon"),
        ("recommendation: yes\n", "yes"),
    ] {
        let fx = Fx::new();
        fx.write("workspace/life/needs/q.md", &format!("---\nprivate: true\n{raw}---\n# Q\n"));
        fx.ok(&["needs", "add", "Decide?", "--detail", "needs/q.md"]);
        assert!(fx.read(VIEW).contains(&format!("| Decide? | {want} | [open](needs/q.md) |\n")), "{raw}: {}", fx.read(VIEW));
    }
    let fx = Fx::new();
    fx.write("workspace/life/needs/q.md", "---\nrecommendation: >\n---\n# Q\n");
    let e = fail(&fx, &["needs", "add", "Decide?", "--detail", "needs/q.md"]);
    assert!(e.contains("no 'recommendation:'"), "an empty block is no recommendation: {e}");
}

#[test]
fn an_empty_list_says_none_and_its_link_resolves() {
    let fx = Fx::new();
    fx.write(OPEN, &format!("- [ ] {} · plain · from: main\n", today()));
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    let out = lf(fx.ok(&["needs", "list"]));
    assert_eq!(out, "needs you: none now, 0 later (workspace/life/needs.md)\n");
    assert!(fx.read(VIEW).contains("## Now (0)\n\nNothing.\n"), "{}", fx.read(VIEW));
    let bundle = fx.ok(&["session", "bundle", "--session", "s-main"]);
    assert!(bundle.contains("needs you: none now, 0 later (workspace/life/needs.md)"), "{bundle}");
}

#[test]
fn no_open_md_means_no_view() {
    let fx = Fx::new();
    fx.ok(&["needs", "render"]);
    assert!(!fx.path(VIEW).exists());
}

#[test]
fn list_prints_the_counts_then_each_item_with_the_number_open_close_takes() {
    let fx = Fx::new();
    fx.ok(&["open", "add", "plain"]);
    detail(&fx, "a", "yes");
    detail(&fx, "b", "wait");
    fx.ok(&["needs", "add", "Now one?", "--detail", "needs/a.md"]);
    fx.ok(&["needs", "add", "Later one?", "--detail", "needs/b.md", "--when", "the quote arrives"]);
    let out = lf(fx.ok(&["needs", "list"]));
    let want = "needs you: 1 now, 0 to do, 1 later (workspace/life/needs.md)\n  \
#2 Now one? · recommend: yes · workspace/life/needs/a.md\n  \
#3 when the quote arrives: Later one? · recommend: wait · workspace/life/needs/b.md\n";
    assert_eq!(out, want);
}

#[test]
fn session_start_prints_the_now_items_to_main_and_refreshes_the_view() {
    let fx = Fx::new();
    detail(&fx, "a", "take it");
    detail(&fx, "b", "wait");
    detail(&fx, "c", "dated");
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    fx.ok(&["session", "register", "project/acme", "--session", "s-acme"]);
    fx.ok(&["needs", "add", "Now one?", "--detail", "needs/a.md"]);
    fx.ok(&["needs", "add", "Later one?", "--detail", "needs/b.md", "--when", "the quote arrives"]);
    fx.ok(&["needs", "add", "Dated one?", "--detail", "needs/c.md", "--on", "+5d"]);

    let out = lf(fx.ok(&["session", "bundle", "--session", "s-main"]));
    assert!(out.contains("needs you: 1 now, 0 to do, 2 later (workspace/life/needs.md)\n- Now one? · recommend: take it · workspace/life/needs/a.md"), "{out}");
    assert!(!out.contains("Later one?") && !out.contains("Dated one?"), "{out}");

    let side = fx.ok(&["session", "bundle", "--session", "s-acme"]);
    assert!(!side.contains("needs you"), "{side}");

    // A date that passed since the last render: the hook rewrites the view before it prints.
    let line = format!("need on {}", days_ahead(5));
    let open = fx.read(OPEN).replace(&line, &format!("need on {}", days_ago(1)));
    fx.write(OPEN, &open);
    let hook = afcore::sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-main","source":"startup"}"#);
    assert!(hook.stdout.contains("needs you: 2 now, 0 to do, 1 later"), "{}", hook.stdout);
    assert!(fx.read(VIEW).contains("## Now (2)"), "{}", fx.read(VIEW));
}

#[test]
fn brief_carries_the_list_only_when_it_has_items() {
    let fx = Fx::new();
    let out = fx.ok(&["brief"]);
    assert!(!out.contains("Needs you"), "{out}");
    detail(&fx, "a", "take it");
    fx.ok(&["needs", "add", "Now one?", "--detail", "needs/a.md"]);
    let out = lf(fx.ok(&["brief"]));
    assert!(out.contains("## Needs you (1 now, 0 to do, 0 later)\n- Now one? · recommend: take it · workspace/life/needs/a.md"), "{out}");
}

// ------------------------------------------------------------------ a date passing

/// A view written while the date was ahead, then the date passes with no af call between.
fn date_just_passed() -> Fx {
    let fx = Fx::new();
    detail(&fx, "d", "rec");
    fx.ok(&["needs", "add", "Dated?", "--detail", "needs/d.md", "--on", "+5d"]);
    let open = fx.read(OPEN).replace(&format!("need on {}", days_ahead(5)), &format!("need on {}", days_ago(1)));
    fx.write(OPEN, &open);
    assert!(fx.read(VIEW).contains("## Later (1)"), "{}", fx.read(VIEW));
    fx
}

#[test]
fn a_passed_date_reaches_the_view_through_every_loop_read_and_write() {
    let cases: [&[&str]; 8] = [
        &["open", "due"],
        &["open", "due", "--json"],
        &["brief"],
        &["brief", "--json"],
        &["doctor"],
        &["needs", "list"],
        &["open", "add", "unrelated"],
        &["open", "redate", "plain", "--nudge", "+3d"],
    ];
    for args in cases {
        let fx = date_just_passed();
        let open = fx.read(OPEN);
        fx.write(OPEN, &format!("{open}- [ ] {} · plain · from: main\n", today()));
        let (code, out, err) = fx.run(args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        let view = fx.read(VIEW);
        assert!(view.contains(&format!("## Now (1)\n\n{NOW_HEAD}| Dated? | rec | [open](needs/d.md) |\n")), "{args:?}: {view}");
    }
    let fx = date_just_passed();
    let open = fx.read(OPEN);
    fx.write(OPEN, &format!("{open}- [ ] {} · plain · from: main\n", today()));
    fx.ok(&["open", "close", "plain", "--outcome", "done"]);
    assert!(fx.read(VIEW).contains("## Now (1)"), "{}", fx.read(VIEW));
}

#[test]
fn a_dated_item_is_due_from_its_date_and_the_rest_of_the_list_never_is() {
    let fx = Fx::new();
    for s in ["today", "past", "ahead", "event", "now"] {
        detail(&fx, s, "rec");
    }
    fx.ok(&["needs", "add", "Today?", "--detail", "needs/today.md", "--on", &today()]);
    fx.ok(&["needs", "add", "Past?", "--detail", "needs/past.md", "--on", &days_ago(2)]);
    fx.ok(&["needs", "add", "Ahead?", "--detail", "needs/ahead.md", "--on", "+2d"]);
    fx.ok(&["needs", "add", "Event?", "--detail", "needs/event.md", "--when", "it happens"]);
    fx.ok(&["needs", "add", "Now?", "--detail", "needs/now.md"]);
    let out = fx.ok(&["open", "due"]);
    assert!(out.contains("Today?") && out.contains("Past?"), "{out}");
    for absent in ["Ahead?", "Event?", "Now?"] {
        assert!(!out.contains(absent), "{absent}: {out}");
    }
    // The phone push prints `nudge` and needs to know which item this is.
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["open", "due", "--json"])).unwrap();
    let past = v.as_array().unwrap().iter().find(|l| l["text"] == "Past?").unwrap();
    assert_eq!(past["nudge"], days_ago(2));
    assert_eq!(past["detail"], "workspace/life/needs/past.md");
    let plain = fx.ok(&["open", "add", "plain", "--nudge", &today()]);
    assert!(plain.contains("plain"));
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["open", "due", "--json"])).unwrap();
    let plain = v.as_array().unwrap().iter().find(|l| l["text"] == "plain").unwrap();
    assert!(plain["detail"].is_null(), "{plain}");
}

// ------------------------------------------------------------------ Do

fn do_fixture() -> Fx {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\n## Waiting on the operator\n\n### Admin\n\n\
- [ ] {o} · Pay the bill · nudge {d} · from: main\n\
- [ ] {o} · Not yet · nudge {f} · from: main\n\
- [ ] {o} · No date · from: main\n\n\
## Bills\n\n\
- [ ] {o} · Pay the card · nudge {t} · every monthly · from: main\n\n\
## Waiting on others\n\n\
- [ ] {o} · Refund from the shop · nudge {d} · from: main\n\n\
### A subgroup\n\n\
- [ ] {o} · Reply from a friend · nudge {d} · from: main\n",
            o = days_ago(9),
            d = days_ago(2),
            f = days_ahead(4),
            t = today()
        ),
    );
    fx
}

#[test]
fn do_lists_plain_due_loops_that_are_his_and_leaves_out_what_others_owe() {
    let fx = do_fixture();
    fx.ok(&["needs", "render"]);
    let view = fx.read(VIEW);
    let (now, do_, later) = (view.find("## Now").unwrap(), view.find("## Do").unwrap(), view.find("## Later").unwrap());
    assert!(now < do_ && do_ < later, "{view}");
    assert!(
        view.contains(&format!(
            "## Do (2)\n\n{DO_HEAD}| Pay the bill | {d} | {o} |\n| Pay the card | {t} | {o} |\n",
            d = days_ago(2),
            t = today(),
            o = days_ago(9)
        )),
        "{view}"
    );
    for absent in ["Not yet", "No date", "Refund", "Reply from a friend"] {
        assert!(!view.contains(absent), "{absent}: {view}");
    }
    let out = lf(fx.ok(&["needs", "list"]));
    assert!(out.starts_with("needs you: 0 now, 2 to do, 0 later (workspace/life/needs.md)\n"), "{out}");
    assert!(out.contains("Pay the bill · due"), "{out}");
}

#[test]
fn a_do_item_leaves_when_it_is_closed_or_redated_and_a_marked_loop_is_never_one() {
    let fx = do_fixture();
    fx.ok(&["open", "close", "Pay the bill", "--outcome", "paid"]);
    assert!(!fx.read(VIEW).contains("Pay the bill"));
    assert!(fx.read(VIEW).contains("## Do (1)"), "{}", fx.read(VIEW));
    fx.ok(&["open", "close", "Pay the card", "--outcome", "paid"]);
    assert!(fx.read(VIEW).contains("## Do (0)\n\nNothing.\n"), "a recurring loop waits for its next date: {}", fx.read(VIEW));
    detail(&fx, "d", "rec");
    fx.ok(&["needs", "add", "Dated?", "--detail", "needs/d.md", "--on", &days_ago(1)]);
    let view = fx.read(VIEW);
    assert!(view.contains("## Now (1)") && view.contains("## Do (0)"), "{view}");
}

#[test]
fn the_summary_counts_do_so_an_action_item_is_never_reported_as_none() {
    let fx = do_fixture();
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    let out = fx.ok(&["session", "bundle", "--session", "s-main"]);
    assert!(out.contains("needs you: 0 now, 2 to do, 0 later"), "{out}");
    let brief = lf(fx.ok(&["brief"]));
    assert!(brief.contains("## Needs you (0 now, 2 to do, 0 later)"), "{brief}");
}

// ------------------------------------------------------------------ a close that fails

#[test]
fn a_failed_log_write_leaves_the_item_open_and_its_detail_file_in_place() {
    let fx = with_item("q", "Decide?", "yes");
    std::fs::create_dir_all(fx.path(&log_rel())).unwrap();
    fail(&fx, &["needs", "close", "q", "--outcome", "decided"]);
    assert!(fx.read(OPEN).contains("Decide?"), "the line stays");
    assert!(fx.path("workspace/life/needs/q.md").exists(), "the detail file stays");
    assert!(!fx.path("workspace/life/needs/closed/q.md").exists());
    assert_eq!(issues(&fx), Vec::<String>::new());
}

#[test]
fn a_failed_move_is_reported_after_the_close_stands_and_doctor_names_the_file() {
    let fx = with_item("q", "Decide?", "yes");
    fx.write("workspace/life/needs/closed", "a file where the folder belongs\n");
    let e = fail(&fx, &["needs", "close", "q", "--outcome", "decided"]);
    assert!(e.contains("needs/q.md"), "{e}");
    assert!(!fx.read(OPEN).contains("Decide?"), "the close stands");
    assert!(fx.read(&log_rel()).contains("closed: Decide? → decided"));
    assert!(fx.path("workspace/life/needs/q.md").exists(), "the notes are not lost");
    let i = issues(&fx);
    assert!(i.iter().any(|m| m.contains("needs/q.md: no open needs item")), "{i:?}");
}

#[test]
fn a_number_names_the_loop_even_when_a_slug_is_that_number() {
    let fx = with_item("first", "First question?", "yes");
    detail(&fx, "1", "no");
    fx.ok(&["needs", "add", "Second question?", "--detail", "needs/1.md"]);
    fx.ok(&["needs", "close", "1", "--outcome", "done"]);
    let open = fx.read(OPEN);
    assert!(!open.contains("First question?"), "{open}");
    assert!(open.contains("Second question?"), "{open}");
}

#[test]
fn a_null_or_a_bare_block_header_is_no_recommendation() {
    for rec in ["~", "null", ">2-"] {
        let fx = Fx::new();
        fx.write(
            "workspace/life/needs/q.md",
            &format!("---\nnote: an: unquoted colon\nrecommendation: {rec}\n---\n# Q\n"),
        );
        let (code, _, _) = fx.run(&["needs", "add", "Q?", "--detail", "needs/q.md"]);
        assert_ne!(code, 0, "{rec} should read as no recommendation");
    }
}

#[test]
fn doctor_never_advises_deleting_an_orphan_detail_file() {
    let fx = Fx::new();
    detail(&fx, "orphan", "yes");
    let found = issues(&fx).join("\n");
    assert!(found.contains("needs/closed/"), "{found}");
    assert!(!found.contains("delete"), "{found}");
}
