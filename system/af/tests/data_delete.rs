//! `af data delete`: remove the rows that match every given column (typed comparison), keep every
//! other line's bytes, re-render the registered pages, and write nothing when no row matches.

mod common;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--key", "exercise,set",
    ]);
}

fn add(fx: &Fx, date: &str, exercise: &str, set: u32, weight: &str) {
    fx.ok(&["data", "add", "sets", &format!("date={date}"), &format!("exercise={exercise}"), &format!("set={set}"), &format!("weight={weight}")]);
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

fn seeded(fx: &Fx) {
    new_sets(fx);
    add(fx, "2026-11-01", "Squat", 1, "100");
    add(fx, "2026-11-01", "Squat", 2, "105");
    add(fx, "2026-11-02", "Bench", 1, "70");
    add(fx, "2026-11-02", "Bench", 2, "5");
}

#[test]
fn delete_by_one_column_removes_every_matching_row() {
    let fx = Fx::new();
    seeded(&fx);
    let out = fx.ok(&["data", "delete", "sets", "exercise=Squat"]);
    assert_eq!(out.trim(), "deleted 2");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight\n2026-11-02,Bench,1,70\n2026-11-02,Bench,2,5\n");
    assert!(!fx.exists(&format!("{SETS}.lock")) && !fx.exists(&format!("{SETS}.tmp")), "no lock or temp file is left");
    let out = fx.ok(&["data", "delete", "sets", "date=2026-11-02"]);
    assert_eq!(out.trim(), "deleted 2");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight\n", "the header stays");
    fx.ok(&["data", "add", "sets", "date=2026-11-03", "exercise=Row", "set=1"]);
}

#[test]
fn delete_by_two_columns_removes_only_the_rows_matching_both() {
    let fx = Fx::new();
    seeded(&fx);
    let out = fx.ok(&["data", "delete", "sets", "exercise=Squat", "date=2026-11-01", "set=2"]);
    assert_eq!(out.trim(), "deleted 1");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight\n2026-11-01,Squat,1,100\n2026-11-02,Bench,1,70\n2026-11-02,Bench,2,5\n");
}

#[test]
fn no_match_is_exit_one_and_writes_nothing() {
    let fx = Fx::new();
    seeded(&fx);
    let before = bytes(&fx, SETS);
    let (code, out, err) = fx.run(&["data", "delete", "sets", "exercise=Deadlift"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("no row matches"), "{err}");
    assert!(out.is_empty());
    let (code, _, err) = fx.run(&["data", "delete", "sets", "exercise=Squat", "set=9"]);
    assert_eq!(code, 1);
    assert!(err.contains("no row matches"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
    assert!(!fx.exists(&format!("{SETS}.lock")));
    // Even a dry run says so with exit 1.
    assert_eq!(fx.run(&["data", "delete", "sets", "exercise=Deadlift", "--dry-run"]).0, 1);
}

#[test]
fn an_unknown_column_a_missing_condition_a_bad_value_and_a_repeated_column_are_refused() {
    let fx = Fx::new();
    seeded(&fx);
    let before = bytes(&fx, SETS);
    let err = fx.refused(&["data", "delete", "sets", "colour=red"]);
    assert!(err.contains("no column 'colour'"), "{err}");
    let err = fx.refused(&["data", "delete", "sets"]);
    assert!(err.contains("at least one"), "{err}");
    let err = fx.refused(&["data", "delete", "sets", "set=abc"]);
    assert!(err.contains("set:"), "{err}");
    let err = fx.refused(&["data", "delete", "sets", "exercise=Squat", "exercise=Bench"]);
    assert!(err.contains("twice"), "{err}");
    let err = fx.refused(&["data", "delete", "sets", "nonsense"]);
    assert!(err.contains("column=value"), "{err}");
    let err = fx.refused(&["data", "delete", "ghost", "a=1"]);
    assert!(err.contains("no ledger named 'ghost'"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
}

#[test]
fn a_match_is_by_typed_value() {
    let fx = Fx::new();
    seeded(&fx);
    assert_eq!(fx.ok(&["data", "delete", "sets", "exercise=Squat", "set=01"]).trim(), "deleted 1", "01 is the 1 stored");
    assert_eq!(fx.ok(&["data", "delete", "sets", "weight=5.0"]).trim(), "deleted 1", "5.0 is the 5 stored");
    assert_eq!(fx.ok(&["data", "delete", "sets", "exercise= Bench "]).trim(), "deleted 1", "spaces around text do not count");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight\n2026-11-01,Squat,2,105\n");
    // An empty value matches the rows where the column is empty.
    add(&fx, "2026-11-05", "Row", 1, "");
    assert_eq!(fx.ok(&["data", "delete", "sets", "weight="]).trim(), "deleted 1");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight\n2026-11-01,Squat,2,105\n");
}

#[test]
fn every_other_line_keeps_its_bytes_and_the_line_endings() {
    let fx = Fx::new();
    new_sets(&fx);
    let text = "\u{feff}\"date\",exercise,set,weight\r\n\"2026-11-01\",\"Squat\",1,100\r\n\r\n2026-11-01,Squat,2,105\r\n\r\n2026-11-02,\"Bench, flat\",1,70\r\n2026-11-02,Bench,2,5";
    fx.write_bytes(SETS, text.as_bytes());
    fx.ok(&["data", "delete", "sets", "exercise=Squat", "set=2"]);
    let want = "\u{feff}\"date\",exercise,set,weight\r\n\"2026-11-01\",\"Squat\",1,100\r\n\r\n\r\n2026-11-02,\"Bench, flat\",1,70\r\n2026-11-02,Bench,2,5";
    assert_eq!(bytes(&fx, SETS), want.as_bytes(), "a deleted line is cut out; blank lines, quotes, the BOM and the missing final newline stay");
    fx.ok(&["data", "delete", "sets", "exercise=Bench", "set=2"]);
    assert_eq!(bytes(&fx, SETS), "\u{feff}\"date\",exercise,set,weight\r\n\"2026-11-01\",\"Squat\",1,100\r\n\r\n\r\n2026-11-02,\"Bench, flat\",1,70\r\n".as_bytes());
    fx.ok(&["data", "add", "sets", "date=2026-11-09", "exercise=Row", "set=1"]);
    assert!(bytes(&fx, SETS).ends_with(b"\r\n2026-11-09,Row,1,\r\n"), "an add after still uses the file's line ending");
}

#[test]
fn dry_run_says_how_many_and_writes_nothing() {
    let fx = Fx::new();
    seeded(&fx);
    fx.write("workspace/life/p.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "p.md"]);
    let page = fx.read("workspace/life/p.md");
    let before = bytes(&fx, SETS);
    let out = fx.ok(&["data", "delete", "sets", "exercise=Squat", "--dry-run"]);
    assert_eq!(out.trim(), "would delete 2");
    assert_eq!(bytes(&fx, SETS), before);
    assert_eq!(fx.read("workspace/life/p.md"), page);
    assert!(!fx.exists(&format!("{SETS}.lock")));
}

#[test]
fn delete_re_renders_the_registered_pages() {
    let fx = Fx::new();
    seeded(&fx);
    fx.write("workspace/life/p.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "p.md"]);
    assert!(fx.read("workspace/life/p.md").contains("| 4 |"));
    fx.ok(&["data", "delete", "sets", "exercise=Squat"]);
    assert!(fx.read("workspace/life/p.md").contains("| n |\n|---|\n| 2 |\n"), "{}", fx.read("workspace/life/p.md"));
}

#[test]
fn a_ledger_whose_file_fails_its_checks_is_not_touched() {
    let fx = Fx::new();
    seeded(&fx);
    fx.write(SETS, "date,exercise,set,weight\n2026-13-01,Squat,1,100\n2026-11-02,Bench,1,70\n");
    let before = bytes(&fx, SETS);
    let err = fx.refused(&["data", "delete", "sets", "exercise=Bench"]);
    assert!(err.contains("sets.csv:2: date:") && err.contains("no row was deleted"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
}

#[test]
fn delete_is_logged_as_data_delete() {
    let fx = Fx::new();
    seeded(&fx);
    fx.ok(&["data", "delete", "sets", "exercise=Squat"]);
    let verbs: Vec<String> = fx.run_log_rows().iter().map(|r| r["verb"].as_str().unwrap().to_string()).collect();
    assert_eq!(verbs.last().unwrap(), "data delete");
}

#[test]
fn eight_processes_deleting_different_keys_at_once_leave_the_right_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    let mut csv = String::from("date,exercise,set\n");
    for i in 0..16 {
        csv.push_str(&format!("2026-11-14,Squat,{i}\n"));
    }
    fx.ok(&["data", "import", "sets", "--csv", &fx.write("in/many.csv", &csv)]);
    fx.write("workspace/life/p.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "p.md"]);
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..8)
            .map(|i| {
                let (fx, start) = (&fx, &start);
                scope.spawn(move || {
                    start.wait();
                    fx.run(&["data", "delete", "sets", "exercise=Squat", &format!("set={}", i * 2)])
                })
            })
            .collect();
        for r in runs {
            let (code, out, err) = r.join().unwrap();
            assert_eq!(code, 0, "{err}");
            assert_eq!(out.trim(), "deleted 1");
        }
    });
    let text = fx.read(SETS);
    let mut left: Vec<i32> = text.lines().skip(1).map(|l| l.split(',').nth(2).unwrap().parse().unwrap()).collect();
    left.sort();
    assert_eq!(left, [1, 3, 5, 7, 9, 11, 13, 15]);
    assert!(fx.read("workspace/life/p.md").contains("| 8 |"), "{}", fx.read("workspace/life/p.md"));
    assert!(!fx.exists(&format!("{SETS}.lock")));
}

#[test]
fn a_fault_names_the_physical_line_even_after_a_blank_line_in_a_crlf_file() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write_bytes(SETS, b"date,exercise,set,weight\r\n2026-11-01,Squat,1,100\r\n\r\n2026-11-01,Squat,2,105\r\nnot-a-date,Squat,3,1\r\n");
    let err = fx.refused(&["data", "sql", "select 1"]);
    assert!(err.contains("sets.csv:5: date:"), "{err}");
    fx.write_bytes(SETS, b"date,exercise,set,weight\n2026-11-01,Squat,1,100\n\n\n2026-11-01,Squat,2,105\nnot-a-date,Squat,3,1\n");
    assert!(fx.refused(&["data", "sql", "select 1"]).contains("sets.csv:6: date:"));
}
