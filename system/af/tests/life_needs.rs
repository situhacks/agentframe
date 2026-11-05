//! `af needs`: marked loops in open.md, their detail files, the generated view, and the
//! doctor, session-start, and brief surfaces.

mod common;
mod life_fx;
use common::*;
use life_fx::*;

const OPEN: &str = "workspace/life/open.md";
const VIEW: &str = "workspace/life/needs.md";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn detail(fx: &Fx, slug: &str, rec: &str) {
    fx.write(
        &format!("workspace/life/needs/{slug}.md"),
        &format!("---\nprivate: true\nrecommendation: {rec}\n---\n# Question\n\nFull notes.\n"),
    );
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
    assert!(view.contains("## Now (1)\n\n- [Which plan to take?](needs/plan-choice.md) · recommend: the cheaper plan\n"), "{view}");
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
    assert!(later.contains(&format!("- on {}: [Soon?](needs/soon.md) · recommend: rec soon", days_ahead(2))), "{later}");
    assert!(later.contains("- when the quote arrives: [Event?](needs/event.md) · recommend: rec event"), "{later}");
    assert!(view.contains("## Now (1)\n\n- [Past?](needs/past.md) · recommend: rec past\n"), "{view}");
}

#[test]
fn the_view_moves_a_passed_date_to_now_on_render_without_touching_open_md() {
    let fx = Fx::new();
    detail(&fx, "d", "rec");
    fx.write(OPEN, &format!("# Open loops\n\n- [ ] {} · Dated? · need on {} · detail needs/d.md · from: main\n", days_ago(9), days_ago(1)));
    let before = fx.read(OPEN);
    fx.ok(&["needs", "render"]);
    assert!(fx.read(VIEW).contains("## Now (1)\n\n- [Dated?](needs/d.md)"), "{}", fx.read(VIEW));
    assert_eq!(fx.read(OPEN), before, "the state lives on the line; only the view changes");
}

#[test]
fn brackets_in_a_question_do_not_break_its_link() {
    let fx = with_item("q", "Ask [[a-friend]] or not?", "ask");
    assert!(fx.read(VIEW).contains(r"- [Ask \[\[a-friend\]\] or not?](needs/q.md)"), "{}", fx.read(VIEW));
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
fn open_redate_refuses_a_marked_loop_and_a_marked_loop_is_never_due() {
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
    let fx = with_item("q", "Decide?", "yes");
    assert_eq!(issues(&fx), Vec::<String>::new());
    assert_eq!(notes(&fx), Vec::<String>::new());
    std::fs::remove_file(fx.path("workspace/life/needs/q.md")).unwrap();
    let (_, stdout, _) = fx.run(&["doctor"]);
    assert!(stdout.contains("needs item 'Decide?'"), "{stdout}");
}

#[test]
fn doctor_notes_a_hand_edited_view() {
    let fx = with_item("q", "Decide?", "yes");
    fx.write(VIEW, "# hand edit\n");
    assert_eq!(notes(&fx), vec![format!("{VIEW}: differs from open.md and the detail files; run af needs render")]);
    fx.ok(&["needs", "render"]);
    assert_eq!(notes(&fx), Vec::<String>::new());
}

#[test]
fn nothing_is_written_while_the_list_is_empty_and_has_never_been_used() {
    let fx = Fx::new();
    fx.write(OPEN, &format!("- [ ] {} · plain · from: main\n", today()));
    fx.ok(&["needs", "render"]);
    assert!(!fx.path(VIEW).exists());
    let out = fx.ok(&["needs", "list"]);
    assert!(out.starts_with("needs you: 0 now, 0 later"), "{out}");
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
    let want = "needs you: 1 now, 1 later (workspace/life/needs.md)\n  \
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
    assert!(out.contains("needs you: 1 now, 2 later (workspace/life/needs.md)\n- Now one? · recommend: take it · workspace/life/needs/a.md"), "{out}");
    assert!(!out.contains("Later one?") && !out.contains("Dated one?"), "{out}");

    let side = fx.ok(&["session", "bundle", "--session", "s-acme"]);
    assert!(!side.contains("needs you"), "{side}");

    // A date that passed since the last render: the hook rewrites the view before it prints.
    let line = format!("need on {}", days_ahead(5));
    let open = fx.read(OPEN).replace(&line, &format!("need on {}", days_ago(1)));
    fx.write(OPEN, &open);
    let hook = afcore::sessions::hook_session_start(&fx.ctx(), r#"{"session_id":"s-main","source":"startup"}"#);
    assert!(hook.stdout.contains("needs you: 2 now, 1 later"), "{}", hook.stdout);
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
    assert!(out.contains("## Needs you (1 now, 0 later)\n- Now one? · recommend: take it · workspace/life/needs/a.md"), "{out}");
}
