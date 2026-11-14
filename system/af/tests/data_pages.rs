//! Pages filled from queries: `af data render`, `af data page`, and the re-render after a write.
//! Only what sits between a block's markers changes; a failing block keeps its old content.

mod common;
use common::*;

const PAGE: &str = "workspace/life/systems/fitness/log.md";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ]);
}

fn add_set(fx: &Fx, exercise: &str, set: u32, weight: &str) {
    fx.ok(&["data", "add", "sets", "date=2026-11-14", &format!("exercise={exercise}"), &format!("set={set}"), &format!("weight={weight}"), "source=log"]);
}

const BEFORE: &str = "# Log\n\nIntro line.\n\n";
const MIDDLE: &str = "\n\nMiddle text with `code` and a | pipe.\n\n";
const AFTER: &str = "\n\nTail, no newline at the end";

fn page_text(eol: &str) -> String {
    let t = format!(
        "{BEFORE}<!-- af-data: select exercise, weight from sets order by \"set\" -->\nold one\n<!-- /af-data -->{MIDDLE}<!-- af-data:\n  select count(*) as n\n  from sets\n-->\nold two\nand more\n<!-- /af-data -->{AFTER}"
    );
    t.replace('\n', eol)
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

#[test]
fn render_fills_two_blocks_and_leaves_the_text_around_them_byte_for_byte() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    add_set(&fx, "Bench", 1, "70.5");
    fx.write(PAGE, &page_text("\n"));
    let out = fx.ok(&["data", "render", "systems/fitness/log.md"]);
    assert!(out.contains("workspace/life/systems/fitness/log.md: 2 blocks, updated"), "{out}");
    let want = format!(
        "{BEFORE}<!-- af-data: select exercise, weight from sets order by \"set\" -->\n| exercise | weight |\n|---|---|\n| Squat | 100 |\n| Bench | 70.5 |\n<!-- /af-data -->{MIDDLE}<!-- af-data:\n  select count(*) as n\n  from sets\n-->\n| n |\n|---|\n| 2 |\n<!-- /af-data -->{AFTER}"
    );
    assert_eq!(fx.read(PAGE), want);
    // A second render changes nothing.
    let before = bytes(&fx, PAGE);
    let out = fx.ok(&["data", "render", "systems/fitness/log.md"]);
    assert!(out.contains("2 blocks, unchanged"), "{out}");
    assert_eq!(bytes(&fx, PAGE), before);
    assert!(!fx.exists(&format!("{PAGE}.lock")));
}

#[test]
fn a_page_with_crlf_stays_crlf_in_every_line() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    fx.write_bytes(PAGE, page_text("\r\n").as_bytes());
    fx.ok(&["data", "render", "systems/fitness/log.md"]);
    let text = String::from_utf8(bytes(&fx, PAGE)).unwrap();
    assert!(text.starts_with(&format!("{}<!-- af-data: select exercise, weight from sets order by \"set\" -->\r\n| exercise | weight |\r\n|---|---|\r\n| Squat | 100 |\r\n<!-- /af-data -->", BEFORE.replace('\n', "\r\n"))), "{text:?}");
    assert!(text.ends_with(&AFTER.replace('\n', "\r\n")), "{text:?}");
    assert!(!text.replace("\r\n", "").contains('\n'), "no bare LF: {text:?}");
    assert!(text.contains("<!-- af-data:\r\n  select count(*) as n\r\n  from sets\r\n-->\r\n| n |\r\n|---|\r\n| 1 |\r\n<!-- /af-data -->"), "{text:?}");
}

#[test]
fn a_failing_block_keeps_its_old_content_while_the_other_is_rendered_and_the_exit_is_one() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    let page = "top\n<!-- af-data: select nope from sets -->\nkept one\n<!-- /af-data -->\nmid\n<!-- af-data: select count(*) as n from sets -->\nstale\n<!-- /af-data -->\n<!-- af-data: delete from sets -->\nkept three\n<!-- /af-data -->\nend\n";
    fx.write(PAGE, page);
    let (code, out, err) = fx.run(&["data", "render", "systems/fitness/log.md"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("workspace/life/systems/fitness/log.md:2: SQL error: no such column: nope"), "{err}");
    assert!(err.contains("workspace/life/systems/fitness/log.md:9: only a single select"), "{err}");
    assert!(out.contains("3 blocks, updated"), "{out}");
    assert_eq!(
        fx.read(PAGE),
        "top\n<!-- af-data: select nope from sets -->\nkept one\n<!-- /af-data -->\nmid\n<!-- af-data: select count(*) as n from sets -->\n| n |\n|---|\n| 1 |\n<!-- /af-data -->\n<!-- af-data: delete from sets -->\nkept three\n<!-- /af-data -->\nend\n"
    );
    assert_eq!(fx.read("workspace/life/systems/fitness/sets.csv").lines().count(), 2, "a refused delete deleted nothing");
}

#[test]
fn an_opening_marker_with_no_closing_marker_is_a_fault_and_that_page_is_not_written_but_others_are() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    let broken = "a\n<!-- af-data: select 1 as one -->\nold\nb\n<!-- af-data: select 2 as two -->\nold2\n<!-- /af-data -->\n";
    let tail = "a\n<!-- af-data: select 1 as one -->\nold\n";
    fx.write("workspace/life/broken.md", broken);
    fx.write("workspace/life/tail.md", tail);
    fx.write("workspace/life/good.md", "<!-- af-data: select 3 as three -->\n<!-- /af-data -->\n");
    let (code, _, err) = fx.run(&["data", "render", "broken.md", "tail.md", "good.md"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("workspace/life/broken.md:2: this opening marker has no closing marker before the next opening marker"), "{err}");
    assert!(err.contains("workspace/life/tail.md:2: this opening marker has no closing marker"), "{err}");
    assert_eq!(fx.read("workspace/life/broken.md"), broken);
    assert_eq!(fx.read("workspace/life/tail.md"), tail);
    assert_eq!(fx.read("workspace/life/good.md"), "<!-- af-data: select 3 as three -->\n| three |\n|---|\n| 3 |\n<!-- /af-data -->\n");
}

#[test]
fn a_query_with_no_rows_renders_the_header_and_no_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select exercise, weight from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "render", "systems/fitness/log.md"]);
    assert_eq!(fx.read(PAGE), "<!-- af-data: select exercise, weight from sets -->\n| exercise | weight |\n|---|---|\n\n(no rows)\n<!-- /af-data -->\n");
}

#[test]
fn add_import_and_column_re_render_the_registered_pages() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n<!-- af-data: select * from sets limit 0 -->\n<!-- /af-data -->\n");
    let out = fx.ok(&["data", "page", "systems/fitness/log.md"]);
    assert!(out.contains("2 blocks, updated"), "{out}");
    assert!(fx.read("workspace/life/ledgers.yaml").contains("pages:\n  - systems/fitness/log.md\n"));
    add_set(&fx, "Squat", 1, "100");
    assert!(fx.read(PAGE).contains("| n |\n|---|\n| 1 |\n"), "{}", fx.read(PAGE));
    let input = fx.write("in/more.csv", "date,exercise,set,source\n2026-11-14,Squat,2,log\n2026-11-14,Squat,3,log\n");
    fx.ok(&["data", "import", "sets", "--csv", &input]);
    assert!(fx.read(PAGE).contains("| 3 |"), "{}", fx.read(PAGE));
    fx.ok(&["data", "column", "sets", "rpe:number"]);
    assert!(fx.read(PAGE).contains("| date | exercise | set | weight | source | rpe |"), "{}", fx.read(PAGE));
    // A replace re-renders too: the count stays, the page is current.
    fx.ok(&["data", "add", "sets", "--replace", "date=2026-11-14", "exercise=Squat", "set=1", "source=log", "weight=111"]);
    assert!(fx.read(PAGE).contains("| 3 |"), "{}", fx.read(PAGE));
}

#[test]
fn a_render_fault_after_a_write_is_a_warning_and_the_write_stands_with_exit_zero() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select nope from sets -->\nkept\n<!-- /af-data -->\n");
    let (code, _, err) = fx.run(&["data", "page", "systems/fitness/log.md"]);
    assert_eq!(code, 1, "page itself exits 1 on a fault: {err}");
    assert!(fx.read("workspace/life/ledgers.yaml").contains("systems/fitness/log.md"), "yet the page is registered");
    let (code, out, err) = fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1", "source=log"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("added 1 row"), "{out}");
    assert!(err.contains("af: warning: workspace/life/systems/fitness/log.md:1: SQL error: no such column: nope"), "{err}");
    assert!(fx.read("workspace/life/systems/fitness/sets.csv").contains("Squat,1,,log"));
    assert_eq!(fx.read(PAGE), "<!-- af-data: select nope from sets -->\nkept\n<!-- /af-data -->\n");
    // A registered page that went missing is a warning too.
    fx.remove(PAGE);
    let (code, _, err) = fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=2", "source=log"]);
    assert_eq!(code, 0);
    assert!(err.contains("log.md: the page is missing"), "{err}");
}

#[test]
fn render_with_no_file_renders_every_registered_page_and_page_adds_one_once() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    fx.write("workspace/life/a.md", "<!-- af-data: select 1 as a -->\n<!-- /af-data -->\n");
    fx.write("workspace/life/sub/b.md", "<!-- af-data: select 2 as b -->\n<!-- /af-data -->\n");
    assert!(fx.ok(&["data", "render"]).contains("no pages"), "nothing registered yet");
    fx.ok(&["data", "page", "a.md"]);
    fx.ok(&["data", "page", "sub/b.md"]);
    fx.ok(&["data", "page", "workspace/life/a.md"]);
    assert_eq!(fx.read("workspace/life/ledgers.yaml").matches("a.md").count(), 1, "a page is listed once");
    fx.write("workspace/life/a.md", "<!-- af-data: select 1 as a -->\nold\n<!-- /af-data -->\n");
    fx.write("workspace/life/sub/b.md", "<!-- af-data: select 2 as b -->\nold\n<!-- /af-data -->\n");
    let out = fx.ok(&["data", "render"]);
    assert!(out.contains("workspace/life/a.md: 1 block, updated") && out.contains("workspace/life/sub/b.md: 1 block, updated"), "{out}");
    assert!(fx.read("workspace/life/a.md").contains("| 1 |") && fx.read("workspace/life/sub/b.md").contains("| 2 |"));
    // An absolute path inside the folder works too.
    fx.write("workspace/life/a.md", "<!-- af-data: select 1 as a -->\nold\n<!-- /af-data -->\n");
    fx.ok(&["data", "render", &fx.p("workspace/life/a.md")]);
    assert!(fx.read("workspace/life/a.md").contains("| 1 |"));
}

#[test]
fn page_needs_an_existing_page_with_a_block_inside_workspace_life_and_a_registry() {
    let fx = Fx::new();
    let err = fx.refused(&["data", "page", "a.md"]);
    assert!(err.contains("does not exist"), "{err}");
    fx.write("workspace/life/plain.md", "no blocks here\n");
    let err = fx.refused(&["data", "page", "plain.md"]);
    assert!(err.contains("holds no <!-- af-data"), "{err}");
    fx.write("workspace/life/open.md", "<!-- af-data: select 1 -->\nnever closed\n");
    let err = fx.refused(&["data", "page", "open.md"]);
    assert!(err.contains("open.md:1:") && err.contains("no closing marker"), "{err}");
    fx.write("workspace/life/ok.md", "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n");
    let err = fx.refused(&["data", "page", "ok.md"]);
    assert!(err.contains("no ledgers are registered"), "{err}");
    assert!(!fx.exists("workspace/life/ledgers.yaml"), "a refusal registers nothing");
    assert_eq!(fx.read("workspace/life/ok.md"), "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n");
    new_sets(&fx);
    fx.write("workspace/outside.md", "<!-- af-data: select 1 as one -->\nold\n<!-- /af-data -->\n");
    fx.write("elsewhere.md", "<!-- af-data: select 1 as one -->\nold\n<!-- /af-data -->\n");
    let reg = fx.read("workspace/life/ledgers.yaml");
    for arg in ["../outside.md", "../../elsewhere.md", "..\\outside.md", &fx.p("workspace/outside.md"), &fx.p("elsewhere.md"), "/etc/x.md", "C:/x.md"] {
        let (code, _, err) = fx.run(&["data", "page", arg]);
        assert_eq!(code, 1, "{arg}: {err}");
        let (code, _, err) = fx.run(&["data", "render", "ok.md", arg]);
        assert_eq!(code, 1, "{arg}: {err}");
    }
    assert_eq!(fx.read("workspace/life/ledgers.yaml"), reg);
    assert!(fx.read("workspace/outside.md").contains("old") && fx.read("elsewhere.md").contains("old"));
    assert_eq!(fx.read("workspace/life/ok.md"), "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n", "a bad argument stops the render before any page");
}

#[test]
fn a_ledger_file_that_fails_its_checks_leaves_every_page_as_it_was() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", 1, "100");
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->\nold\n<!-- /af-data -->\n");
    fx.write("workspace/life/systems/fitness/sets.csv", "date,exercise,set,weight,source\n2026-13-45,Squat,1,100,log\n");
    let (code, _, err) = fx.run(&["data", "render", "systems/fitness/log.md"]);
    assert_eq!(code, 1);
    assert!(err.contains("sets.csv:2: date:"), "{err}");
    assert_eq!(fx.read(PAGE), "<!-- af-data: select count(*) as n from sets -->\nold\n<!-- /af-data -->\n");
}

#[test]
fn a_marker_with_space_variants_and_an_indented_closing_marker_render_and_keep_the_indent() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "- item\n  <!--af-data:select 1 as one-->\n  old\n  <!--  /af-data  -->\n- next\n");
    fx.ok(&["data", "render", "systems/fitness/log.md"]);
    assert_eq!(fx.read(PAGE), "- item\n  <!--af-data:select 1 as one-->\n| one |\n|---|\n| 1 |\n  <!--  /af-data  -->\n- next\n");
}

#[test]
fn eight_adds_at_once_with_a_registered_page_leave_the_page_showing_eight_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "systems/fitness/log.md"]);
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..8)
            .map(|i| {
                let (fx, start) = (&fx, &start);
                scope.spawn(move || {
                    start.wait();
                    fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", &format!("set={i}"), "source=log"])
                })
            })
            .collect();
        for r in runs {
            let (code, _, err) = r.join().unwrap();
            assert_eq!(code, 0, "{err}");
        }
    });
    assert!(fx.read(PAGE).contains("| n |\n|---|\n| 8 |\n"), "{}", fx.read(PAGE));
}

#[test]
fn an_import_that_adds_nothing_still_re_renders_the_registered_pages() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->
<!-- /af-data -->
");
    fx.ok(&["data", "page", "systems/fitness/log.md"]);
    let input = fx.write("in/one.csv", "date,exercise,set,source
2026-11-14,Squat,1,log
");
    fx.ok(&["data", "import", "sets", "--csv", &input]);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->
edited by hand
<!-- /af-data -->
");
    let out = fx.ok(&["data", "import", "sets", "--csv", &input]);
    assert_eq!(out.trim(), "added 0, unchanged 1, replaced 0");
    assert!(fx.read(PAGE).contains("| n |
|---|
| 1 |
") && !fx.read(PAGE).contains("edited by hand"), "{}", fx.read(PAGE));
}
