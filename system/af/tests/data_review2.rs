//! Second review of `af data`: one time limit per command, a clamped limit, what delete prints,
//! pages entries that cannot be pages, and the smaller points.

mod common;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";
const REGISTRY: &str = "workspace/life/ledgers.yaml";
const COUNTING: &str = "with recursive r(n) as (select 1 union all select n+1 from r) select count(*) from r";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "note:text", "--key", "exercise,set",
    ]);
}

fn timed(fx: &Fx, ms: &str, args: &[&str]) -> (i32, String, String, std::time::Duration) {
    let t = std::time::Instant::now();
    let out = fx.af().env("AF_DATA_SQL_TIMEOUT_MS", ms).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned(), t.elapsed())
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

fn slow_page(blocks: usize) -> String {
    let mut p = String::from("# Slow\n");
    for _ in 0..blocks {
        p.push_str(&format!("<!-- af-data: {COUNTING} -->\nkept\n<!-- /af-data -->\n"));
    }
    p
}

fn register_by_hand(fx: &Fx, pages: &[&str]) {
    let reg = fx.read(REGISTRY);
    let list: String = pages.iter().map(|p| format!("\n  - {p}")).collect();
    fx.write(REGISTRY, &reg.replace("pages: []", &format!("pages:{list}")));
}

// ------------------------------------------------------------------ 1: one deadline per command

#[test]
fn many_slow_blocks_share_one_time_limit_and_the_later_ones_are_not_run() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write("workspace/life/slow.md", &slow_page(10));
    fx.write("workspace/life/slow2.md", &slow_page(3));
    let before = fx.read("workspace/life/slow.md");
    let (code, _, err, took) = timed(&fx, "500", &["data", "render", "slow.md", "slow2.md"]);
    assert_eq!(code, 1, "{err}");
    assert!(took.as_millis() >= 400 && took.as_millis() < 3500, "about one limit for the whole command, not one per block: {took:?}");
    assert!(err.contains("workspace/life/slow.md:2: the query was stopped after"), "{err}");
    assert_eq!(err.matches("not run: the time limit for this command was spent").count(), 12, "{err}");
    assert!(err.contains("workspace/life/slow.md:5: not run:") && err.contains("workspace/life/slow2.md:2: not run:"), "{err}");
    assert_eq!(fx.read("workspace/life/slow.md"), before, "every block keeps its old content");
}

#[test]
fn the_re_render_after_a_write_and_the_doctor_each_share_one_limit_too() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write("workspace/life/slow.md", &slow_page(10));
    register_by_hand(&fx, &["slow.md"]);
    let (code, out, err, took) = timed(&fx, "500", &["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("added 1 row") && took.as_millis() < 3500, "{took:?}");
    assert!(err.contains("af: warning: workspace/life/slow.md:2: the query was stopped after") && err.contains("slow.md:5: not run:"), "{err}");
    let (_, out, err, took) = timed(&fx, "500", &["doctor"]);
    assert!(took.as_millis() < 3500, "{took:?}");
    let all = format!("{out}{err}");
    assert!(all.contains("slow.md:2: the block fails: the query was stopped after") && all.contains("slow.md:5: the block fails: not run: the time limit"), "{all}");
}

// ------------------------------------------------------------------ 2: the limit cannot be switched off

#[test]
fn the_limit_from_the_environment_is_clamped_to_1_to_60000_and_a_bad_value_is_the_default() {
    use afcore::data::query::{parse_timeout_ms, DEFAULT_TIMEOUT_MS};
    assert_eq!(parse_timeout_ms(None), DEFAULT_TIMEOUT_MS);
    assert_eq!(parse_timeout_ms(Some("250")), 250);
    assert_eq!(parse_timeout_ms(Some(" 250 ")), 250);
    assert_eq!(parse_timeout_ms(Some("60000")), 60_000);
    assert_eq!(parse_timeout_ms(Some("60001")), 60_000);
    assert_eq!(parse_timeout_ms(Some("999999999999")), 60_000);
    assert_eq!(parse_timeout_ms(Some("18446744073709551615")), 60_000);
    assert_eq!(parse_timeout_ms(Some("0")), 1);
    assert_eq!(parse_timeout_ms(Some("abc")), DEFAULT_TIMEOUT_MS);
    assert_eq!(parse_timeout_ms(Some("-5")), DEFAULT_TIMEOUT_MS);
    assert_eq!(parse_timeout_ms(Some("")), DEFAULT_TIMEOUT_MS);
    assert_eq!(parse_timeout_ms(Some("1.5")), DEFAULT_TIMEOUT_MS);
    let fx = Fx::new();
    let (code, out, _, _) = timed(&fx, "99999999999999999999999", &["data", "sql", "select 1 as one"]);
    assert_eq!(code, 0);
    assert!(out.contains("| 1 |"));
    let (code, _, err, took) = timed(&fx, "0", &["data", "sql", COUNTING]);
    assert_eq!(code, 1, "{err}");
    assert!(took.as_secs() < 5 && err.contains("stopped after"), "{took:?} {err}");
}

// ------------------------------------------------------------------ 4: delete shows what it removed

#[test]
fn delete_prints_the_removed_lines_as_they_stood_in_both_modes_and_says_how_many_more() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write_bytes(SETS, b"date,exercise,set,weight,note\r\n2026-11-01,\"Squat\",1,100,\"a, b\"\r\n2026-11-01,Bench,1,70,\r\n");
    let out = fx.ok(&["data", "delete", "sets", "exercise=Squat", "--dry-run"]).replace("\r\n", "\n");
    assert_eq!(out, "would delete 1\n2026-11-01,\"Squat\",1,100,\"a, b\"\n");
    let out = fx.ok(&["data", "delete", "sets", "exercise=Squat"]).replace("\r\n", "\n");
    assert_eq!(out, "deleted 1\n2026-11-01,\"Squat\",1,100,\"a, b\"\n", "the line as it stood: quotes kept, line ending not printed");
    // An empty condition matches the empty values, and the rows it removes are shown.
    let out = fx.ok(&["data", "delete", "sets", "note="]).replace("\r\n", "\n");
    assert_eq!(out, "deleted 1\n2026-11-01,Bench,1,70,\n");
    // Many rows: 20 lines, then a count of the rest.
    let mut csv = String::from("date,exercise,set\n");
    for i in 1..=25 {
        csv.push_str(&format!("2026-11-14,Row,{i}\n"));
    }
    fx.ok(&["data", "import", "sets", "--csv", &fx.write("in/many.csv", &csv)]);
    for args in [vec!["--dry-run"], vec![]] {
        let mut a = vec!["data", "delete", "sets", "exercise=Row"];
        a.extend(args.clone());
        let out = fx.ok(&a).replace("\r\n", "\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1 + 20 + 1, "{out}");
        assert!(lines[0] == "would delete 25" || lines[0] == "deleted 25", "{out}");
        assert_eq!(lines[1], "2026-11-14,Row,1,,");
        assert_eq!(lines[20], "2026-11-14,Row,20,,");
        assert_eq!(lines[21], "... and 5 more");
    }
}

// ------------------------------------------------------------------ 5: pages entries that cannot be pages

#[test]
fn a_registry_page_that_is_not_a_markdown_page_inside_the_folder_is_an_issue_and_writes_skip_it() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write("workspace/life/ok.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "ok.md"]);
    let reg = fx.read(REGISTRY).replace("  - ok.md\n", "  - ok.md\n  - sets.csv\n  - ../outside.md\n  - notes.txt\n");
    fx.write(REGISTRY, &reg);
    // A lock held on the csv must not make a write wait: the entry is skipped, not opened.
    let ledger = fx.path("workspace/life/systems/fitness/sets.csv");
    let _ = ledger;
    std::fs::write(fx.path("workspace/life/sets.csv.lock"), "pid 1\n").unwrap();
    let start = std::time::Instant::now();
    let (code, out, err) = fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1"]);
    assert_eq!(code, 0, "{err}");
    assert!(start.elapsed().as_millis() < 3000, "{:?}", start.elapsed());
    assert!(out.contains("added 1 row"), "{out}");
    for bad in ["sets.csv", "../outside.md", "notes.txt"] {
        assert!(err.contains(&format!("af: warning: workspace/life/ledgers.yaml: pages entry \"{bad}\"")), "{bad}: {err}");
    }
    assert!(fx.read("workspace/life/ok.md").contains("| 1 |"), "the good page was still rendered");
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1, "{out}");
    for bad in ["sets.csv", "../outside.md", "notes.txt"] {
        assert!(out.contains(&format!("pages entry \"{bad}\"")), "{bad}: {out}");
    }
    // The entries are still in the file after a registry save (the doctor goes on flagging them).
    fx.ok(&["data", "column", "sets", "rpe:number"]);
    let reg = fx.read(REGISTRY);
    assert!(reg.contains("sets.csv\n") && reg.contains("../outside.md") && reg.contains("notes.txt"), "{reg}");
    // render with no argument skips them too.
    let (_, _, err) = fx.run(&["data", "render"]);
    assert!(err.contains("pages entry \"notes.txt\""), "{err}");
}

// ------------------------------------------------------------------ 6: delete checks which rows remain

#[test]
fn delete_lines_refuses_when_the_rows_that_would_remain_are_not_the_expected_ones() {
    use afcore::data::ledger::{delete_lines, Col, Ledger, Ty};
    let fx = Fx::new();
    let path = fx.write("t.csv", "id,amt\na,1\nb,2\nc,3\n");
    let l = Ledger {
        name: "t".into(),
        file: "t.csv".into(),
        key: vec!["id".into()],
        columns: vec![Col { name: "id".into(), ty: Ty::Text, required: true }, Col { name: "amt".into(), ty: Ty::Number, required: false }],
    };
    let row = |a: &str, b: &str| vec![a.to_string(), b.to_string()];
    // Line 2 is `a,1`; the caller expects `a` and `c` to remain, which is wrong: same count, other rows.
    let before = std::fs::read(&path).unwrap();
    let r = delete_lines(&path, &l, &[2], &[row("a", "1"), row("c", "3")]);
    assert!(r.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before, "the file is untouched");
    delete_lines(&path, &l, &[2], &[row("b", "2"), row("c", "3")]).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "id,amt\nb,2\nc,3\n");
}

// ------------------------------------------------------------------ 7: the schema page says what is written

#[test]
fn the_schema_page_says_an_integer_is_written_canonically_and_a_number_as_given_within_500_words() {
    let text = std::fs::read_to_string(repo_root().join("library/schema/ledger.md")).unwrap();
    assert!(text.contains("an `integer` is written canonically (`01` is `1`)"), "{text}");
    assert!(text.contains("a `number` as given") && text.contains("`5.50`"), "{text}");
    assert!(text.contains("at most 12 significant digits"), "{text}");
    assert!(text.split_whitespace().count() < 500, "{} words", text.split_whitespace().count());
}

#[test]
fn a_number_column_keeps_01_minus_0_and_5_50_as_given() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "nn", "--file", "nn.csv", "--column", "k:text!", "--column", "v:number", "--key", "k"]);
    fx.ok(&["data", "add", "nn", "k=a", "v=01"]);
    fx.ok(&["data", "add", "nn", "k=b", "v=-0"]);
    fx.ok(&["data", "add", "nn", "k=c", "v= 5.50 "]);
    assert_eq!(fx.read("workspace/life/nn.csv"), "k,v\na,01\nb,-0\nc,5.50\n");
}

// ------------------------------------------------------------------ 8: column says what it did

#[test]
fn column_says_what_happened_in_each_half_state() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.ok(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1", "weight=100"]);
    // The file is ahead: its rows keep their values.
    fx.write(SETS, "date,exercise,set,weight,note,rpe\n2026-11-14,Squat,1,100,,8\n");
    let out = fx.ok(&["data", "column", "sets", "rpe:number"]);
    assert!(out.contains("the file already had column rpe:number") && out.contains("registered it") && out.contains("1 rows keep their values"), "{out}");
    assert!(!out.contains("get an empty value"), "{out}");
    // The registry is ahead: the file's rows get an empty value.
    let reg = fx.read(REGISTRY).replace("      - {name: rpe, type: number}\n", "      - {name: rpe, type: number}\n      - {name: more, type: text}\n");
    fx.write(REGISTRY, &reg);
    fx.write(SETS, "date,exercise,set,weight,note,rpe\n2026-11-14,Squat,1,100,,8\n");
    let out = fx.ok(&["data", "column", "sets", "more:text"]);
    assert!(out.contains("the registry already had column more:text") && out.contains("1 rows get an empty value"), "{out}");
    // The ordinary case.
    let out = fx.ok(&["data", "column", "sets", "plain:text"]);
    assert_eq!(out.trim(), "added column plain:text to 'sets' (1 rows get an empty value)");
}

// ------------------------------------------------------------------ 9: a typo before a second block

#[test]
fn a_mistyped_opening_marker_before_a_second_block_is_reported_as_the_typo() {
    let fx = Fx::new();
    new_sets(&fx);
    let page = "A\n<!-- af-data: select 1 as one ->\nOLD1\n<!-- af-data: select 2 as two -->\nOLD2\n<!-- /af-data -->\nB\n";
    fx.write("workspace/life/typo2.md", page);
    let (code, _, err) = fx.run(&["data", "render", "typo2.md"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("workspace/life/typo2.md:2:") && err.contains("typo"), "{err}");
    assert!(!err.contains("SQL error"), "{err}");
    assert_eq!(fx.read("workspace/life/typo2.md"), page);
    // The same for the other typo, a `--` that lost its `>`.
    let page = "A\n<!-- af-data: select 1 as one --\nOLD1\n<!-- af-data: select 2 as two -->\nOLD2\n<!-- /af-data -->\n";
    fx.write("workspace/life/typo3.md", page);
    let err = fx.refused(&["data", "render", "typo3.md"]);
    assert!(err.contains("typo3.md:2:") && !err.contains("SQL error"), "{err}");
    let _ = bytes(&fx, SETS);
}
