//! `af doctor` and ledgers: issues for a broken ledger file, notes for a page that is missing, has a
//! failing block, or shows stale rows. No registry file means silence.

mod common;
use afcore::data::doctor_checks;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";
const PAGE: &str = "workspace/life/systems/fitness/log.md";
const HEADER: &str = "date,exercise,set,weight,source\n";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ]);
}

fn check(fx: &Fx) -> (Vec<String>, Vec<String>) {
    doctor_checks(&fx.ctx()).unwrap()
}

#[test]
fn no_registry_is_silence_and_a_good_ledger_is_clean() {
    let fx = Fx::new();
    assert_eq!(check(&fx), (vec![], vec![]));
    new_sets(&fx);
    fx.ok(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1", "source=log", "weight=100"]);
    assert_eq!(check(&fx), (vec![], vec![]));
    let out = fx.ok(&["doctor"]);
    assert!(!out.contains("sets.csv"), "{out}");
}

#[test]
fn a_missing_file_a_wrong_header_a_bad_row_and_a_repeated_key_are_issues_with_file_and_line() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.remove(SETS);
    let (i, n) = check(&fx);
    assert_eq!(i, ["workspace/life/systems/fitness/sets.csv: the file is missing"]);
    assert!(n.is_empty());

    fx.write(SETS, "date,exercise,set,source\n");
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].starts_with("workspace/life/systems/fitness/sets.csv:1: the header is date,exercise,set,source but the registry has date,exercise,set,weight,source"), "{i:?}");

    fx.write(
        SETS,
        &format!("{HEADER}2026-02-30,Squat,1,100,log\n2026-11-14,Squat,2,1e3,log\n2026-11-14,,3,1,log\n2026-11-14,Squat,4,1\n2026-11-14,Squat,5,1,log\n2026-11-15,Squat,5,2,log\n"),
    );
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 5, "{i:?}");
    assert!(i[0].contains("sets.csv:2: date: \"2026-02-30\" is not a real date"), "{i:?}");
    assert!(i[1].contains("sets.csv:3: weight: \"1e3\" is not a number"), "{i:?}");
    assert!(i[2].contains("sets.csv:4: exercise is required and empty"), "{i:?}");
    assert!(i[3].contains("sets.csv:5: has 4 fields, the header has 5"), "{i:?}");
    assert!(i[4].contains("sets.csv:7: the key (source=log, exercise=Squat, set=5) repeats line 6"), "{i:?}");
}

#[test]
fn at_most_five_lines_per_ledger_then_a_count_and_each_ledger_has_its_own_five() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.ok(&["data", "new", "body", "--file", "body.csv", "--column", "date:date!", "--column", "kg:number", "--key", "date"]);
    let mut text = String::from(HEADER);
    for n in 1..=9 {
        text.push_str(&format!("not-a-date,Squat,{n},1,log\n"));
    }
    fx.write(SETS, &text);
    fx.write("workspace/life/body.csv", "date,kg\nbad,1\n");
    let (i, _) = check(&fx);
    let sets: Vec<&String> = i.iter().filter(|l| l.contains("sets.csv")).collect();
    assert_eq!(sets.len(), 6, "{i:?}");
    assert!(sets[5].contains("... and 4 more faults in ledger 'sets'"), "{sets:?}");
    assert_eq!(i.iter().filter(|l| l.contains("body.csv:2:")).count(), 1, "{i:?}");
}

#[test]
fn a_page_missing_a_failing_block_and_stale_content_are_notes_and_render_clears_them() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "systems/fitness/log.md"]);
    assert_eq!(check(&fx), (vec![], vec![]), "rendered and current");

    fx.write("workspace/life/systems/fitness/sets.csv", &format!("{HEADER}2026-11-14,Squat,1,100,log\n"));
    let (i, n) = check(&fx);
    assert!(i.is_empty(), "{i:?}");
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].starts_with("workspace/life/systems/fitness/log.md: its rendered content differs") && n[0].contains("run `af data render`"), "{n:?}");
    fx.ok(&["data", "render"]);
    assert_eq!(check(&fx), (vec![], vec![]));

    fx.write(PAGE, "x\n<!-- af-data: select nope from sets -->\nkept\n<!-- /af-data -->\n");
    let (i, n) = check(&fx);
    assert!(i.is_empty());
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("log.md:2: the block fails: SQL error: no such column: nope"), "{n:?}");

    fx.write(PAGE, "<!-- af-data: select 1 as one -->\nnever closed\n");
    let (_, n) = check(&fx);
    assert!(n[0].contains("log.md:1:") && n[0].contains("no closing marker"), "{n:?}");

    fx.remove(PAGE);
    let (i, n) = check(&fx);
    assert!(i.is_empty());
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].starts_with("workspace/life/systems/fitness/log.md: the registered page is missing"), "{n:?}");
}

#[test]
fn a_ledger_issue_stops_page_notes_for_that_run_and_af_doctor_exits_one() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write(PAGE, "<!-- af-data: select count(*) as n from sets -->\nstale\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "systems/fitness/log.md"]);
    fx.write(SETS, &format!("{HEADER}nope,Squat,1,100,log\n"));
    let (i, n) = check(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(n.is_empty(), "the ledger fault is the one thing to fix: {n:?}");
    let (code, out, err) = fx.run(&["doctor"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(format!("{out}{err}").contains("sets.csv:2: date:"), "{out}{err}");
}

#[test]
fn a_registry_that_does_not_parse_is_one_issue() {
    let fx = Fx::new();
    fx.write("workspace/life/ledgers.yaml", "ledgers:\n  - name: Bad Name\n    file: a.csv\n");
    let (i, _) = check(&fx);
    assert_eq!(i.len(), 1, "{i:?}");
    assert!(i[0].contains("ledgers.yaml") && i[0].contains("ledger name \"Bad Name\" must be lower-case letters"), "the reason is in the issue: {i:?}");
}
