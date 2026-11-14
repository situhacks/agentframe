//! `af data sql`: every ledger loaded into an in-memory SQLite, one read-only statement, three
//! output formats. A refused statement leaves no file created or changed anywhere.

mod common;
use common::*;
use std::collections::BTreeMap;
use std::path::Path;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ]);
}

fn add_set(fx: &Fx, exercise: &str, set: &str, weight: &str) {
    fx.ok(&["data", "add", "sets", "date=2026-11-14", &format!("exercise={exercise}"), &format!("set={set}"), &format!("weight={weight}"), "source=log"]);
}

/// Every file under a folder with its bytes.
fn snapshot(dir: &Path, skip_log: bool) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for e in walkdir::WalkDir::new(dir).into_iter().flatten().filter(|e| e.file_type().is_file()) {
        let rel = e.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
        if skip_log && rel == "system/logs/af-runs.jsonl" {
            continue;
        }
        out.insert(rel, std::fs::read(e.path()).unwrap());
    }
    out
}

#[test]
fn new_then_add_then_sql_returns_a_value_with_a_comma_a_quote_and_non_ascii_text_intact() {
    let fx = Fx::new();
    new_sets(&fx);
    let ex = "Press, \"strict\" \u{e9}t\u{e9} \u{1f3cb} a|b";
    add_set(&fx, ex, "1", "62.5");
    add_set(&fx, "Squat", "1", "100");
    let md = fx.ok(&["data", "sql", "select exercise, \"set\", weight from sets order by exercise"]);
    let lines: Vec<&str> = md.lines().collect();
    assert_eq!(lines[0], "| exercise | set | weight |");
    assert_eq!(lines[1], "|---|---|---|");
    assert_eq!(lines[2], "| Press, \"strict\" \u{e9}t\u{e9} \u{1f3cb} a\\|b | 1 | 62.5 |");
    assert_eq!(lines[3], "| Squat | 1 | 100 |");
    let csv = fx.ok(&["data", "sql", "select exercise, weight from sets order by exercise", "--csv"]);
    assert_eq!(csv.replace("\r\n", "\n"), "exercise,weight\n\"Press, \"\"strict\"\" \u{e9}t\u{e9} \u{1f3cb} a|b\",62.5\nSquat,100\n");
    let json = fx.ok(&["data", "sql", "select exercise, \"set\", weight from sets order by exercise", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v[0]["exercise"], ex);
    assert_eq!(v[0]["weight"], 62.5);
    assert_eq!(v[1]["set"], 1);
    assert!(json.find("\"exercise\"").unwrap() < json.find("\"set\"").unwrap(), "keys stay in column order: {json}");
}

#[test]
fn a_ledger_named_sets_and_columns_named_set_group_and_order_work_when_quoted() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "sets", "--file", "s.csv", "--column", "set:integer!", "--column", "group:text", "--column", "order:integer", "--key", "set"]);
    fx.ok(&["data", "add", "sets", "set=1", "group=legs", "order=2"]);
    fx.ok(&["data", "add", "sets", "set=2", "group=legs", "order=1"]);
    fx.ok(&["data", "add", "sets", "set=3"]);
    let out = fx.ok(&["data", "sql", "select \"group\", sum(\"order\") as total, count(\"set\") as n from sets group by \"group\" order by \"group\""]);
    assert!(out.contains("| legs | 3 | 2 |") && out.contains("|  | "), "{out}");
    assert!(out.contains("| group | total | n |"), "{out}");
    // Unquoted `group` is SQLite's keyword: it fails like any SQL error, listing the columns.
    let err = fx.refused(&["data", "sql", "select group from sets"]);
    assert!(err.contains("SQL error") && err.contains("sets (set integer, group text, order integer)"), "{err}");
    let out = fx.ok(&["data", "sql", "select * from sets order by \"set\" desc limit 1", "--json"]);
    assert!(out.contains("\"group\": null") && out.contains("\"order\": null"), "{out}");
}

#[test]
fn null_is_empty_in_a_table_and_in_csv_and_null_in_json() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "");
    assert!(fx.ok(&["data", "sql", "select exercise, weight from sets"]).contains("| Squat |  |"));
    assert_eq!(fx.ok(&["data", "sql", "select exercise, weight from sets", "--csv"]).replace("\r\n", "\n"), "exercise,weight\nSquat,\n");
    assert!(fx.ok(&["data", "sql", "select exercise, weight from sets", "--json"]).contains("\"weight\": null"));
    assert!(fx.ok(&["data", "sql", "select count(*) as n from sets where weight is null", "--csv"]).replace("\r\n", "\n").contains("\n1\n"));
}

#[test]
fn real_values_print_with_no_trailing_zeros_and_no_binary_noise() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "prices", "--file", "p.csv", "--column", "item:text!", "--column", "price:number", "--key", "item"]);
    fx.ok(&["data", "add", "prices", "item=a", "price=0.1"]);
    fx.ok(&["data", "add", "prices", "item=b", "price=0.2"]);
    fx.ok(&["data", "add", "prices", "item=c", "price=2.50"]);
    fx.ok(&["data", "add", "prices", "item=d", "price=-0.00001"]);
    assert!(fx.ok(&["data", "sql", "select sum(price) as t from prices where item in ('a', 'b')"]).contains("| 0.3 |"));
    let out = fx.ok(&["data", "sql", "select price from prices order by item", "--csv"]).replace("\r\n", "\n");
    assert_eq!(out, "price\n0.1\n0.2\n2.5\n-0.00001\n");
    assert!(fx.ok(&["data", "sql", "select 1.0/3 as x, 2.0 as y, 1e20 as z", "--json"]).contains("\"x\": 0.333333333333, \"y\": 2, \"z\": 100000000000000000000"));
    assert!(fx.ok(&["data", "sql", "select 7 as n, 7.0 as r", "--csv"]).replace("\r\n", "\n").contains("7,7\n"));
}

#[test]
fn integer_columns_stay_integers_and_dates_and_text_are_text() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "10", "100");
    let out = fx.ok(&["data", "sql", "select typeof(date), typeof(exercise), typeof(\"set\"), typeof(weight) from sets", "--csv"]).replace("\r\n", "\n");
    assert_eq!(out, "typeof(date),typeof(exercise),\"typeof(\"\"set\"\")\",typeof(weight)\ntext,text,integer,real\n");
    // Integers sort as numbers, so 10 comes after 9.
    add_set(&fx, "Squat", "9", "100");
    let out = fx.ok(&["data", "sql", "select \"set\" from sets order by \"set\"", "--csv"]).replace("\r\n", "\n");
    assert_eq!(out, "set\n9\n10\n");
}

#[test]
fn each_refused_statement_exits_one_and_leaves_no_file_created_or_changed_anywhere() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    let scratch = tempfile::tempdir().unwrap();
    let sp = scratch.path().to_string_lossy().replace('\\', "/");
    let statements: Vec<String> = vec![
        "insert into sets (date, exercise, set, source) values ('2026-01-01', 'x', 1, 's')".into(),
        "update sets set weight = 1".into(),
        "delete from sets".into(),
        "drop table sets".into(),
        "create table t (x)".into(),
        "create table t as select 1".into(),
        format!("attach database '{sp}/attached.db' as other"),
        "attach database 'relative.db' as other".into(),
        "pragma user_version = 3".into(),
        "pragma table_info(sets)".into(),
        format!("vacuum into '{sp}/vacuumed.db'"),
        "vacuum into 'relative-vacuum.db'".into(),
        "with x as (select 1 as a) insert into sets (date, exercise, set, source) select '2026-01-01', 'x', a, 's' from x".into(),
        "with x as (select 1) delete from sets".into(),
        "select 1; select 2".into(),
        "select 1; drop table sets".into(),
        "select 1; insert into sets (date, exercise, set, source) values ('2026-01-01', 'x', 2, 's')".into(),
        "select load_extension('nothing')".into(),
        "SELECT LOAD_EXTENSION('nothing', 'entry')".into(),
        "-- a comment\n insert into sets (date, exercise, set, source) values ('2026-01-01', 'x', 3, 's')".into(),
        "/* a comment */ delete from sets".into(),
        "explain select 1".into(),
        "replace into sets (date, exercise, set, source) values ('2026-01-01', 'x', 4, 's')".into(),
        "begin".into(),
        "".into(),
        "-- only a comment".into(),
        "selectx 1".into(),
    ];
    let root_before = snapshot(fx.dir.path(), true);
    let scratch_before = snapshot(scratch.path(), false);
    for sql in &statements {
        let out = fx.af().current_dir(scratch.path()).args(["data", "sql", sql]).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{sql:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert!(out.stdout.is_empty(), "{sql:?} printed {:?}", String::from_utf8_lossy(&out.stdout));
    }
    assert_eq!(snapshot(fx.dir.path(), true), root_before, "nothing in the vault changed (the run log aside)");
    assert_eq!(snapshot(scratch.path(), false), scratch_before, "nothing appeared where the statements pointed");
    assert!(scratch.path().read_dir().unwrap().next().is_none());
    // And the ledger still answers.
    assert!(fx.ok(&["data", "sql", "select count(*) as n from sets"]).contains("| 1 |"));
}

#[test]
fn a_trailing_semicolon_a_leading_comment_and_a_with_select_are_fine() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    for sql in [
        "select count(*) as n from sets;",
        "select count(*) as n from sets; -- done",
        "-- how many\n  select count(*) as n from sets",
        "/* how many */ SELECT count(*) AS n FROM sets",
        "with s as (select * from sets) select count(*) as n from s",
        "WITH s AS (SELECT 1 AS x) SELECT count(*) AS n FROM s",
    ] {
        let (code, out, err) = fx.run(&["data", "sql", sql]);
        assert_eq!(code, 0, "{sql:?}: {err}");
        assert!(out.contains("| n |"), "{sql:?}: {out}");
    }
}

#[test]
fn rows_past_the_limit_are_cut_with_a_last_line_saying_how_many_of_how_many() {
    let fx = Fx::new();
    new_sets(&fx);
    let mut csv = String::from("date,exercise,set,source\n");
    for i in 1..=205 {
        csv.push_str(&format!("2026-11-14,Squat,{i},log\n"));
    }
    let input = fx.write("in/many.csv", &csv);
    fx.ok(&["data", "import", "sets", "--csv", &input]);
    let out = fx.ok(&["data", "sql", "select \"set\" from sets order by \"set\""]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.last().unwrap(), &"showing 200 of 205 rows; --limit shows more");
    assert_eq!(lines.iter().filter(|l| l.starts_with("| ") && l[2..].chars().next().unwrap().is_ascii_digit()).count(), 200);
    let out = fx.ok(&["data", "sql", "select \"set\" from sets order by \"set\"", "--limit", "3"]);
    assert_eq!(out.lines().last().unwrap(), "showing 3 of 205 rows; --limit shows more");
    let out = fx.ok(&["data", "sql", "select \"set\" from sets order by \"set\"", "--limit", "300"]);
    assert!(!out.contains("showing"), "{out}");
    // A machine format keeps stdout to the data; the note goes to stderr.
    let (code, out, err) = fx.run(&["data", "sql", "select \"set\" from sets order by \"set\"", "--csv", "--limit", "2"]);
    assert_eq!(code, 0);
    assert_eq!(out.replace("\r\n", "\n"), "set\n1\n2\n");
    assert!(err.contains("showing 2 of 205 rows"), "{err}");
    let (_, out, err) = fx.run(&["data", "sql", "select \"set\" from sets order by \"set\"", "--json", "--limit", "2"]);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap().as_array().unwrap().len(), 2);
    assert!(err.contains("showing 2 of 205"), "{err}");
    let (code, ..) = fx.run(&["data", "sql", "select 1", "--limit", "0"]);
    assert_ne!(code, 0);
}

#[test]
fn a_query_with_no_rows_prints_the_header_and_no_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    let out = fx.ok(&["data", "sql", "select date, weight from sets"]);
    assert_eq!(out.replace("\r\n", "\n"), "| date | weight |\n|---|---|\n\n(no rows)\n");
    assert_eq!(fx.ok(&["data", "sql", "select date from sets", "--json"]).trim(), "[]");
}

#[test]
fn a_ledger_file_that_fails_its_checks_stops_the_query_with_the_fault() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    fx.write(SETS, "date,exercise,set,weight,source\n2026-11-14,Squat,1,100,log\n2026-11-31,Squat,2,100,log\n2026-11-14,Squat,1,105,log\n");
    let (code, out, err) = fx.run(&["data", "sql", "select count(*) from sets"]);
    assert_eq!(code, 1);
    assert!(out.is_empty(), "no number is printed: {out}");
    assert!(err.contains("workspace/life/systems/fitness/sets.csv:3: date: \"2026-11-31\" is not a real date"), "{err}");
    // A repeated key is a fault too.
    fx.write(SETS, "date,exercise,set,weight,source\n2026-11-14,Squat,1,100,log\n2026-11-14,Squat,1,105,log\n");
    let err = fx.refused(&["data", "sql", "select 1"]);
    assert!(err.contains("sets.csv:3: the key (source=log, exercise=Squat, set=1) repeats line 2"), "{err}");
    fx.remove(SETS);
    let err = fx.refused(&["data", "sql", "select 1"]);
    assert!(err.contains("sets.csv: the file is missing"), "{err}");
}

#[test]
fn a_sql_error_prints_sqlites_message_and_the_tables_and_columns() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.ok(&["data", "new", "body", "--file", "body.csv", "--column", "date:date!", "--column", "kg:number", "--key", "date"]);
    let err = fx.refused(&["data", "sql", "select nope from sets"]);
    assert!(err.contains("SQL error: no such column: nope"), "{err}");
    assert!(err.contains("tables:") && err.contains("  sets (date date, exercise text, set integer, weight number, source text)") && err.contains("  body (date date, kg number)"), "{err}");
    let err = fx.refused(&["data", "sql", "select * from ghost"]);
    assert!(err.contains("no such table: ghost"), "{err}");
    let err = fx.refused(&["data", "sql", "select from"]);
    assert!(err.contains("SQL error"), "{err}");
}

#[test]
fn sql_works_with_no_registry_and_says_so_when_a_table_is_missing() {
    let fx = Fx::new();
    assert!(fx.ok(&["data", "sql", "select 1 as one"]).contains("| 1 |"));
    let err = fx.refused(&["data", "sql", "select * from sets"]);
    assert!(err.contains("no such table") && err.contains("no ledgers are registered"), "{err}");
    assert!(!fx.exists("workspace"), "a query creates nothing");
}

#[test]
fn a_column_added_later_is_null_for_the_old_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    fx.ok(&["data", "column", "sets", "rpe:number"]);
    let out = fx.ok(&["data", "sql", "select rpe, rpe is null as unset from sets", "--csv"]).replace("\r\n", "\n");
    assert_eq!(out, "rpe,unset\n,1\n");
}

#[test]
fn json_and_csv_cannot_be_asked_together_and_a_join_across_ledgers_works() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.ok(&["data", "new", "body", "--file", "body.csv", "--column", "date:date!", "--column", "kg:number", "--key", "date"]);
    add_set(&fx, "Squat", "1", "100");
    fx.ok(&["data", "add", "body", "date=2026-11-14", "kg=80.5"]);
    let out = fx.ok(&["data", "sql", "select s.weight / b.kg as ratio from sets s join body b on b.date = s.date"]);
    assert!(out.contains("| 1.24223602484 |"), "{out}");
    let (code, ..) = fx.run(&["data", "sql", "select 1", "--json", "--csv"]);
    assert_eq!(code, 2);
}

#[test]
fn sql_is_logged_as_data_sql() {
    let fx = Fx::new();
    fx.ok(&["data", "sql", "select 1"]);
    let rows = fx.run_log_rows();
    assert_eq!(rows[0]["verb"], "data sql");
}

// ------------------------------------------------------------------ review: blocker 3

const ENDLESS: &str = "with recursive r(n) as (select 1 union all select n+1 from r) select n from r";

fn timed(fx: &Fx, ms: &str, args: &[&str]) -> (i32, String, String, std::time::Duration) {
    let t = std::time::Instant::now();
    let out = fx.af().env("AF_DATA_SQL_TIMEOUT_MS", ms).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned(), t.elapsed())
}

#[test]
fn the_default_time_limit_is_ten_seconds() {
    assert_eq!(afcore::data::query::DEFAULT_TIMEOUT_MS, 10_000);
}

#[test]
fn an_endless_query_with_a_limit_prints_the_limit_and_says_more_than_after_the_time_limit() {
    let fx = Fx::new();
    let (code, out, err, took) = timed(&fx, "300", &["data", "sql", ENDLESS, "--limit", "3"]);
    assert_eq!(code, 0, "{err}");
    assert!(took.as_secs() < 5, "{took:?}");
    assert!(out.contains("| 3 |") && !out.contains("| 4 |"), "{out}");
    assert!(out.contains("showing 3 of more than "), "{out}");
    let (code, out, err, _) = timed(&fx, "300", &["data", "sql", ENDLESS, "--limit", "2", "--json"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap().as_array().unwrap().len(), 2);
    assert!(err.contains("showing 2 of more than "), "{err}");
}

#[test]
fn a_query_that_gives_no_row_in_time_is_stopped_with_an_error_naming_the_time() {
    let fx = Fx::new();
    let (code, out, err, took) = timed(&fx, "300", &["data", "sql", "with recursive r(n) as (select 1 union all select n+1 from r) select count(*) from r"]);
    assert_eq!(code, 1, "{out}");
    assert!(took.as_secs() < 5, "{took:?}");
    assert!(err.contains("the query was stopped after") && err.contains("second"), "{err}");
    assert!(out.is_empty());
    assert!(fx.ok(&["data", "sql", "select 1 as one"]).contains("| 1 |"));
}

#[test]
fn a_registered_page_with_an_endless_block_does_not_hang_add_render_or_the_doctor() {
    let fx = Fx::new();
    new_sets(&fx);
    let counting = "with recursive r(n) as (select 1 union all select n+1 from r) select count(*) from r";
    fx.write("workspace/life/p.md", &format!("<!-- af-data: {ENDLESS} -->\n<!-- /af-data -->\n<!-- af-data: {counting} -->\nkept\n<!-- /af-data -->\n"));
    let (code, _, err, took) = timed(&fx, "300", &["data", "page", "p.md"]);
    assert_eq!(code, 1, "{err}");
    assert!(took.as_secs() < 8, "{took:?}");
    assert!(err.contains("p.md:3:") && err.contains("stopped after"), "{err}");
    let page = fx.read("workspace/life/p.md");
    assert!(page.ends_with(&format!("<!-- af-data: {counting} -->\nkept\n<!-- /af-data -->\n")), "kept: {page}");
    assert_eq!(page.matches("\n| 500 |\n").count(), 1, "the endless block holds 500 rows");
    assert!(!page.contains("\n| 501 |\n") && page.contains("(showing 500 of more than 100000 rows)"), "{}", &page[page.len().saturating_sub(300)..]);
    let (code, out, err, took) = timed(&fx, "300", &["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1", "source=log"]);
    assert_eq!(code, 0, "{err}");
    assert!(took.as_secs() < 8 && out.contains("added 1 row") && err.contains("af: warning:"), "{took:?} {out} {err}");
    let (_, out, err, took) = timed(&fx, "300", &["doctor"]);
    assert!(took.as_secs() < 15, "{took:?}");
    assert!(format!("{out}{err}").contains("the block fails: the query was stopped after"), "{out}{err}");
}

#[test]
fn a_block_keeps_at_most_500_rows_and_says_how_many_of_how_many() {
    let fx = Fx::new();
    new_sets(&fx);
    let mut csv = String::from("date,exercise,set,source\n");
    for i in 1..=600 {
        csv.push_str(&format!("2026-11-14,Squat,{i},log\n"));
    }
    fx.ok(&["data", "import", "sets", "--csv", &fx.write("in/many.csv", &csv)]);
    fx.write("workspace/life/p.md", "<!-- af-data: select \"set\" from sets order by \"set\" -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "render", "p.md"]);
    let page = fx.read("workspace/life/p.md");
    assert!(page.contains("\n| 500 |\n") && !page.contains("\n| 501 |\n"), "{page}");
    assert!(page.contains("\n\n(showing 500 of 600 rows)\n<!-- /af-data -->"), "{}", &page[page.len().saturating_sub(120)..]);
    fx.ok(&["data", "render", "p.md"]);
    assert_eq!(fx.read("workspace/life/p.md"), page, "stable");
}
