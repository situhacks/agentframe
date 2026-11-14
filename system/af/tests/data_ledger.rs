//! `af data new`, `column`, `add`, `import`, `list`: the ledger registry and the write verbs.
//! A refusal exits 1 and writes nothing; an append leaves the bytes already there as they were.

mod common;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";
const REGISTRY: &str = "workspace/life/ledgers.yaml";
const HEADER: &str = "date,exercise,set,weight,source\n";

fn new_sets(fx: &Fx) -> String {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ])
}

fn add_set(fx: &Fx, exercise: &str, set: &str, weight: &str) -> (i32, String, String) {
    fx.run(&["data", "add", "sets", "date=2026-11-14", &format!("exercise={exercise}"), &format!("set={set}"), &format!("weight={weight}"), "source=log"])
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

fn files_under(fx: &Fx, rel: &str) -> Vec<String> {
    let mut out: Vec<String> = walkdir::WalkDir::new(fx.path(rel))
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(fx.path(rel)).unwrap().to_string_lossy().replace('\\', "/"))
        .collect();
    out.sort();
    out
}

#[test]
fn new_writes_the_registry_and_a_csv_with_its_header_and_list_shows_it() {
    let fx = Fx::new();
    let out = new_sets(&fx);
    assert!(out.contains("created") && out.contains("'sets'"), "{out}");
    assert_eq!(fx.read(SETS), HEADER);
    let reg = fx.read(REGISTRY);
    assert!(reg.contains("ledgers:\n  - name: sets\n    file: systems/fitness/sets.csv\n    key: [source, exercise, set]\n    columns:\n"), "{reg}");
    assert!(reg.contains("      - {name: date, type: date, required: true}\n"), "{reg}");
    assert!(reg.contains("      - {name: weight, type: number}\n"), "{reg}");
    assert!(reg.contains("pages: []"), "{reg}");
    let list = fx.ok(&["data", "list"]);
    assert!(list.contains("sets: workspace/life/systems/fitness/sets.csv, 0 rows"), "{list}");
    assert!(list.contains("columns: date:date!, exercise:text!, set:integer!, weight:number, source:text!"), "{list}");
    assert!(!fx.exists(&format!("{SETS}.lock")) && !fx.exists(&format!("{REGISTRY}.lock")), "locks are released");
}

#[test]
fn a_value_with_a_comma_a_quote_and_non_ascii_text_is_quoted_only_where_needed_and_kept_whole() {
    let fx = Fx::new();
    new_sets(&fx);
    let ex = "Press, \"strict\" \u{e9}t\u{e9} \u{1f3cb}";
    let (code, _, err) = add_set(&fx, ex, "1", "62.5");
    assert_eq!(code, 0, "{err}");
    add_set(&fx, "Squat", "1", "100");
    let text = fx.read(SETS);
    assert!(text.contains("2026-11-14,\"Press, \"\"strict\"\" \u{e9}t\u{e9} \u{1f3cb}\",1,62.5,log\n"), "{text}");
    assert!(text.contains("2026-11-14,Squat,1,100,log\n"), "unquoted when it needs no quotes: {text}");
}

#[test]
fn a_ledger_and_its_columns_may_be_named_like_sql_keywords_except_a_ledger_that_breaks_a_bare_select() {
    let fx = Fx::new();
    let out = fx.ok(&["data", "new", "sets", "--file", "a.csv", "--column", "set:integer!", "--column", "group:text", "--column", "order:text", "--key", "set"]);
    assert!(out.contains("created"), "{out}");
    for bad in ["group", "select", "order", "sqlite_x"] {
        let err = fx.refused(&["data", "new", bad, "--file", &format!("{bad}.csv"), "--column", "a:text!", "--key", "a"]);
        assert!(err.contains("keyword") || err.contains("reserved"), "{bad}: {err}");
        assert!(!fx.exists(&format!("workspace/life/{bad}.csv")), "{bad}");
    }
}

#[test]
fn new_refuses_bad_names_files_columns_and_keys_and_writes_nothing() {
    let fx = Fx::new();
    let base = |name: &'static str, file: &'static str, cols: &[&'static str], key: &'static str| {
        let mut a = vec!["data", "new", name, "--file", file];
        for c in cols {
            a.push("--column");
            a.push(c);
        }
        a.push("--key");
        a.push(key);
        fx.run(&a)
    };
    let cases: Vec<(&str, &str, Vec<&str>, &str)> = vec![
        ("Sets", "a.csv", vec!["a:text!"], "a"),
        ("1sets", "a.csv", vec!["a:text!"], "a"),
        ("a-b", "a.csv", vec!["a:text!"], "a"),
        ("x", "../a.csv", vec!["a:text!"], "a"),
        ("x", "sub/../../a.csv", vec!["a:text!"], "a"),
        ("x", "C:/a.csv", vec!["a:text!"], "a"),
        ("x", "a.txt", vec!["a:text!"], "a"),
        ("x", ".csv", vec!["a:text!"], "a"),
        ("x", "a.csv", vec!["A:text!"], "a"),
        ("x", "a.csv", vec!["a:string!"], "a"),
        ("x", "a.csv", vec!["a"], "a"),
        ("x", "a.csv", vec!["a:text!", "a:text"], "a"),
        ("x", "a.csv", vec!["a:text"], "a"),
        ("x", "a.csv", vec!["a:text!"], "b"),
        ("x", "a.csv", vec!["a:text!", "b:text!"], "a,a"),
    ];
    for (name, file, cols, key) in cases {
        let (code, _, err) = base(name, file, &cols, key);
        assert_eq!(code, 1, "{name} {file} {cols:?} {key}: {err}");
    }
    let abs = fx.p("elsewhere/a.csv");
    let err = fx.refused(&["data", "new", "x", "--file", &abs, "--column", "a:text!", "--key", "a"]);
    assert!(err.contains("inside workspace/life"), "{err}");
    let (code, ..) = fx.run(&["data", "new", "x", "--file", "a.csv", "--key", "a"]);
    assert_eq!(code, 1, "no columns");
    assert!(!fx.exists("workspace"), "nothing was written: no registry, no folder");
    assert!(!fx.exists("elsewhere"));
}

#[test]
fn new_refuses_a_name_or_a_file_already_registered() {
    let fx = Fx::new();
    new_sets(&fx);
    let reg = fx.read(REGISTRY);
    let err = fx.refused(&["data", "new", "sets", "--file", "other.csv", "--column", "a:text!", "--key", "a"]);
    assert!(err.contains("already registered"), "{err}");
    let err = fx.refused(&["data", "new", "other", "--file", "systems/fitness/SETS.csv", "--column", "a:text!", "--key", "a"]);
    assert!(err.contains("already the file"), "{err}");
    assert_eq!(fx.read(REGISTRY), reg);
    assert!(!fx.exists("workspace/life/other.csv"));
}

#[test]
fn a_good_existing_csv_is_adopted_unchanged() {
    let fx = Fx::new();
    let body = "date,exercise,set,weight,source\r\n2026-11-01,\"Squat\",1,100,log\r\n2026-11-01,Squat,2,,log\r\n";
    fx.write_bytes(SETS, body.as_bytes());
    let out = new_sets(&fx);
    assert!(out.contains("adopted") && out.contains("(2 rows)"), "{out}");
    assert_eq!(bytes(&fx, SETS), body.as_bytes(), "the file was not touched");
    assert!(fx.read(REGISTRY).contains("name: sets"));
}

#[test]
fn an_existing_csv_with_a_bad_row_is_refused_with_five_faults_and_no_registry_entry() {
    let fx = Fx::new();
    let mut body = String::from(HEADER);
    for i in 0..8 {
        body.push_str(&format!("2026-02-30,Squat,{i},100,log\n"));
    }
    body.push_str("2026-11-01,Squat,9,1,200,log\n");
    fx.write(SETS, &body);
    let (code, _, err) = fx.run(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!", "--column",
        "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ]);
    assert_eq!(code, 1, "{err}");
    let faults: Vec<&str> = err.lines().filter(|l| l.starts_with("workspace/life/systems/fitness/sets.csv:")).collect();
    assert_eq!(faults.len(), 5, "{err}");
    assert!(faults[0].starts_with("workspace/life/systems/fitness/sets.csv:2: date: \"2026-02-30\" is not a real date"), "{err}");
    assert!(err.contains("more"), "a count of the rest: {err}");
    assert!(!fx.exists(REGISTRY), "no registry entry");
    assert_eq!(fx.read(SETS), body);

    fx.write(SETS, "date,exercise,set,weight\n");
    let err = fx.refused(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!", "--column",
        "weight:number", "--column", "source:text!", "--key", "source,exercise,set",
    ]);
    assert!(err.contains("sets.csv:1: the header is date,exercise,set,weight but the registry has"), "{err}");
    assert!(!fx.exists(REGISTRY));
}

#[test]
fn each_type_refuses_what_it_should_and_a_refusal_writes_nothing() {
    let fx = Fx::new();
    new_sets(&fx);
    let before = bytes(&fx, SETS);
    let reg = fx.read(REGISTRY);
    let attempt = |field: &str, value: &str| {
        let mut vals: Vec<String> = vec!["date=2026-11-14".into(), "exercise=Squat".into(), "set=1".into(), "source=log".into()];
        vals.retain(|v| !v.starts_with(&format!("{field}=")));
        vals.push(format!("{field}={value}"));
        let mut a: Vec<&str> = vec!["data", "add", "sets"];
        a.extend(vals.iter().map(String::as_str));
        fx.run(&a)
    };
    for (field, value) in [
        ("date", "2026-02-30"),
        ("date", "2026-2-3"),
        ("date", "14/11/2026"),
        ("date", "2026-11-14 "),
        ("weight", "1,200"),
        ("weight", "1e3"),
        ("weight", "+5"),
        ("weight", ".5"),
        ("weight", "5."),
        ("weight", "abc"),
        ("weight", "1 000"),
        ("set", "3.0"),
        ("set", "1e3"),
        ("set", "1,000"),
        ("set", "99999999999999999999"),
        ("set", "-"),
        ("exercise", "two\nlines"),
        ("exercise", "carriage\rreturn"),
    ] {
        let (code, _, err) = attempt(field, value);
        assert_eq!(code, 1, "{field}={value:?}: {err}");
    }
    assert_eq!(bytes(&fx, SETS), before);
    assert_eq!(fx.read(REGISTRY), reg);
    for (n, (field, value)) in [("weight", "-12.5"), ("weight", "0.25"), ("set", "-2"), ("date", "2024-02-29")].into_iter().enumerate() {
        // A distinct exercise per row, so none of these repeats a key.
        let ex = format!("exercise=Lift{n}");
        let kv = format!("{field}={value}");
        let mut a: Vec<&str> = vec!["data", "add", "sets", "source=log", "set=1", "date=2026-11-14", &ex];
        a.retain(|v| !v.starts_with(&format!("{field}=")));
        a.push(&kv);
        let (code, _, err) = fx.run(&a);
        assert_eq!(code, 0, "{field}={value:?}: {err}");
    }
}

#[test]
fn add_refuses_an_unknown_column_a_missing_required_value_and_a_key_that_exists() {
    let fx = Fx::new();
    new_sets(&fx);
    assert_eq!(add_set(&fx, "Squat", "1", "100").0, 0);
    let before = bytes(&fx, SETS);
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=2", "source=log", "colour=red"]);
    assert!(err.contains("no column 'colour'"), "{err}");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=2"]);
    assert!(err.contains("source is required"), "{err}");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=2", "source="]);
    assert!(err.contains("source is required"), "{err}");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "exercise=Bench", "set=2", "source=log"]);
    assert!(err.contains("twice"), "{err}");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=1", "source=log", "weight=105"]);
    assert!(err.contains("exists at line 2") && err.contains("(source=log, exercise=Squat, set=1)"), "{err}");
    let err = fx.refused(&["data", "add", "sets", "nonsense"]);
    assert!(err.contains("column=value"), "{err}");
    let err = fx.refused(&["data", "add", "ghost", "a=1"]);
    assert!(err.contains("no ledger named 'ghost'") && err.contains("sets"), "{err}");
    let (code, ..) = fx.run(&["data", "add", "sets"]);
    assert_eq!(code, 1);
    assert_eq!(bytes(&fx, SETS), before);
}

#[test]
fn add_with_replace_replaces_the_row_with_that_key_and_keeps_the_rest() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    add_set(&fx, "Squat", "2", "100");
    let out = fx.ok(&["data", "add", "sets", "--replace", "date=2026-11-15", "exercise=Squat", "set=1", "weight=105", "source=log"]);
    assert!(out.contains("replaced 1 row"), "{out}");
    assert_eq!(fx.read(SETS), format!("{HEADER}2026-11-15,Squat,1,105,log\n2026-11-14,Squat,2,100,log\n"));
    let out = fx.ok(&["data", "add", "sets", "--replace", "date=2026-11-15", "exercise=Squat", "set=1", "weight=105", "source=log"]);
    assert!(out.contains("nothing changed"), "{out}");
    let out = fx.ok(&["data", "add", "sets", "--replace", "date=2026-11-16", "exercise=Squat", "set=3", "source=log"]);
    assert!(out.contains("added 1 row"), "a new key is still appended: {out}");
    assert_eq!(files_under(&fx, "workspace/life/systems/fitness"), ["sets.csv"], "no temp or lock file is left");
}

#[test]
fn appending_leaves_the_earlier_bytes_identical_with_crlf_and_needless_quotes() {
    let fx = Fx::new();
    let old = "\"date\",\"exercise\",\"set\",\"weight\",\"source\"\r\n\"2026-11-01\",\"Squat\",\"1\",\"100\",\"log\"\r\n2026-11-01,Bench,1,,log";
    fx.write_bytes(SETS, old.as_bytes());
    new_sets(&fx);
    // The header has quotes but equals the columns; the last line has no final newline.
    add_set(&fx, "Dead, lift", "1", "140");
    let now = bytes(&fx, SETS);
    assert!(now.starts_with(old.as_bytes()), "the old bytes are the start of the file");
    assert_eq!(&now[old.len()..], "\r\n2026-11-14,\"Dead, lift\",1,140,log\r\n".as_bytes());
    add_set(&fx, "Row", "1", "");
    let again = bytes(&fx, SETS);
    assert!(again.starts_with(&now));
    assert_eq!(&again[now.len()..], "2026-11-14,Row,1,,log\r\n".as_bytes());
}

#[test]
fn a_file_with_lf_endings_keeps_them_and_a_new_file_uses_lf() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    assert!(!fx.read(SETS).replace("\r\n", "").is_empty() && !bytes(&fx, SETS).contains(&b'\r'));
}

#[test]
fn eight_adds_at_once_end_with_eight_rows() {
    let fx = Fx::new();
    new_sets(&fx);
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
    let text = fx.read(SETS);
    assert_eq!(text.lines().count(), 9, "{text}");
    let mut sets: Vec<i32> = text.lines().skip(1).map(|l| l.split(',').nth(2).unwrap().parse().unwrap()).collect();
    sets.sort();
    assert_eq!(sets, (0..8).collect::<Vec<_>>());
    assert_eq!(files_under(&fx, "workspace/life/systems/fitness"), ["sets.csv"]);
}

#[test]
fn column_adds_the_column_to_the_header_the_rows_and_the_registry() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    add_set(&fx, "Squat", "2", "");
    let out = fx.ok(&["data", "column", "sets", "rpe:number"]);
    assert!(out.contains("rpe:number") && out.contains("2 rows"), "{out}");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,source,rpe\n2026-11-14,Squat,1,100,log,\n2026-11-14,Squat,2,,log,\n");
    assert!(fx.read(REGISTRY).contains("      - {name: rpe, type: number}\n"));
    assert!(fx.read(REGISTRY).contains("pages: []"));
    // A row added after takes the new column.
    fx.ok(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=3", "source=log", "rpe=8.5"]);
    assert!(fx.read(SETS).ends_with("2026-11-14,Squat,3,,log,8.5\n"));
    // Never required, never twice, never a bad name or type.
    for spec in ["rpe2:number!", "rpe:number", "RPE:number", "x:float", "x"] {
        let (code, ..) = fx.run(&["data", "column", "sets", spec]);
        assert_eq!(code, 1, "{spec}");
    }
    let (code, ..) = fx.run(&["data", "column", "ghost", "x:text"]);
    assert_eq!(code, 1);
    assert_eq!(files_under(&fx, "workspace/life/systems/fitness"), ["sets.csv"]);
}

#[test]
fn column_keeps_crlf_and_refuses_a_ledger_whose_file_fails_its_checks() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.write_bytes(SETS, format!("{}2026-11-14,Squat,1,100,log\r\n", HEADER.replace('\n', "\r\n")).as_bytes());
    fx.ok(&["data", "column", "sets", "note:text"]);
    assert_eq!(bytes(&fx, SETS), b"date,exercise,set,weight,source,note\r\n2026-11-14,Squat,1,100,log,\r\n");
    fx.write(SETS, "date,exercise,set,weight,source,note\n2026-13-14,Squat,1,100,log,\n");
    let reg = fx.read(REGISTRY);
    let err = fx.refused(&["data", "column", "sets", "more:text"]);
    assert!(err.contains("sets.csv:2: date:"), "{err}");
    assert_eq!(fx.read(REGISTRY), reg);
}

fn csv_in(fx: &Fx, name: &str, text: &str) -> String {
    fx.write(&format!("in/{name}"), text)
}

#[test]
fn import_twice_adds_nothing_the_second_time() {
    let fx = Fx::new();
    new_sets(&fx);
    let f = csv_in(&fx, "a.csv", "source,set,exercise,date,weight\nlog,1,Squat,2026-11-14,100\nlog,2,Squat,2026-11-14,\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim(), "added 2, unchanged 0, replaced 0");
    assert_eq!(fx.read(SETS), format!("{HEADER}2026-11-14,Squat,1,100,log\n2026-11-14,Squat,2,,log\n"));
    let before = bytes(&fx, SETS);
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim(), "added 0, unchanged 2, replaced 0");
    assert_eq!(bytes(&fx, SETS), before);
}

#[test]
fn import_refuses_a_changed_row_without_replace_and_replaces_it_with_replace() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    add_set(&fx, "Squat", "2", "100");
    let f = csv_in(&fx, "a.csv", "date,exercise,set,weight,source\n2026-11-14,Squat,2,110,log\n2026-11-14,Squat,3,120,log\n");
    let before = bytes(&fx, SETS);
    let err = fx.refused(&["data", "import", "sets", "--csv", &f]);
    assert!(err.contains("a.csv:2: the key (source=log, exercise=Squat, set=2) is at line 3 of the ledger with different values"), "{err}");
    assert_eq!(bytes(&fx, SETS), before, "the new row was not appended either");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--replace"]);
    assert_eq!(out.trim(), "added 1, unchanged 0, replaced 1");
    assert_eq!(fx.read(SETS), format!("{HEADER}2026-11-14,Squat,1,100,log\n2026-11-14,Squat,2,110,log\n2026-11-14,Squat,3,120,log\n"));
}

#[test]
fn one_bad_row_in_an_import_writes_no_row() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    let before = bytes(&fx, SETS);
    let f = csv_in(&fx, "a.csv", "date,exercise,set,weight,source\n2026-11-14,Squat,5,100,log\n2026-11-14,Squat,6,1e3,log\n2026-11-14,Squat,7,100\n,Squat,8,1,log\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f]);
    assert!(err.contains("a.csv:3: weight:") && err.contains("a.csv:4: has 4 fields, the header has 5") && err.contains("a.csv:5: date is required"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
    // A key repeated inside the input with other values is refused too.
    let f = csv_in(&fx, "b.csv", "date,exercise,set,weight,source\n2026-11-14,Squat,5,100,log\n2026-11-14,Squat,5,101,log\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f]);
    assert!(err.contains("repeats an earlier row of the input"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
}

#[test]
fn import_header_may_leave_out_optional_columns_but_not_add_or_repeat_columns() {
    let fx = Fx::new();
    new_sets(&fx);
    let before = bytes(&fx, SETS);
    let f = csv_in(&fx, "ok.csv", "exercise,set,source,date\nSquat,1,log,2026-11-14\n");
    fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(fx.read(SETS), format!("{HEADER}2026-11-14,Squat,1,,log\n"));
    let after = bytes(&fx, SETS);
    for (name, text, want) in [
        ("unknown.csv", "date,exercise,set,source,colour\n2026-11-14,Squat,2,log,red\n", "colour"),
        ("missing.csv", "date,exercise,set\n2026-11-14,Squat,2\n", "requires"),
        ("twice.csv", "date,date,exercise,set,source\n2026-11-14,2026-11-14,Squat,2,log\n", "twice"),
        ("empty.csv", "", "empty"),
    ] {
        let f = csv_in(&fx, name, text);
        let err = fx.refused(&["data", "import", "sets", "--csv", &f]);
        assert!(err.contains(want), "{name}: {err}");
    }
    assert_eq!(bytes(&fx, SETS), after);
    assert_ne!(before, after);
    let err = fx.refused(&["data", "import", "sets", "--csv", &fx.p("in/none.csv")]);
    assert!(err.contains("cannot read"), "{err}");
}

#[test]
fn import_reads_quoted_commas_a_bom_and_crlf_and_the_csv_may_be_anywhere() {
    let fx = Fx::new();
    new_sets(&fx);
    let f = fx.write_bytes("anywhere/else/in.csv", "\u{feff}date,exercise,set,weight,source\r\n2026-11-14,\"Press, \"\"strict\"\"\",1,60,log\r\n".as_bytes());
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim(), "added 1, unchanged 0, replaced 0");
    assert_eq!(fx.read(SETS), format!("{HEADER}2026-11-14,\"Press, \"\"strict\"\"\",1,60,log\n"));
}

#[test]
fn a_path_that_leaves_workspace_life_is_refused_for_file_and_the_registry_stays_as_it_was() {
    let fx = Fx::new();
    new_sets(&fx);
    let reg = fx.read(REGISTRY);
    for file in ["../out.csv", "systems/../../out.csv", "..\\out.csv", "/abs/out.csv", "C:\\out.csv", "a//b.csv"] {
        let (code, _, err) = fx.run(&["data", "new", "z", "--file", file, "--column", "a:text!", "--key", "a"]);
        assert_eq!(code, 1, "{file}: {err}");
    }
    assert_eq!(fx.read(REGISTRY), reg);
    assert!(!fx.exists("workspace/out.csv") && !fx.exists("out.csv"));
    // A backslash path inside the folder is kept as a forward-slash path.
    fx.ok(&["data", "new", "z", "--file", "sub\\z.csv", "--column", "a:text!", "--key", "a"]);
    assert!(fx.isfile("workspace/life/sub/z.csv"));
    assert!(fx.read(REGISTRY).contains("file: sub/z.csv"));
}

#[test]
fn list_prints_the_date_range_and_with_a_name_the_last_five_rows() {
    let fx = Fx::new();
    new_sets(&fx);
    for i in 1..=7 {
        fx.ok(&["data", "add", "sets", &format!("date=2026-11-{:02}", i), "exercise=Squat", &format!("set={i}"), "source=log", "weight=100"]);
    }
    let list = fx.ok(&["data", "list"]);
    assert!(list.contains("7 rows, 2026-11-01 to 2026-11-07"), "{list}");
    let one = fx.ok(&["data", "list", "sets"]);
    assert!(one.contains("last 5 rows:") && one.contains("| 2026-11-07 | Squat | 7 | 100 | log |") && one.contains("| 2026-11-03 | Squat | 3 |"), "{one}");
    assert!(!one.contains("| 2026-11-02 |"), "{one}");
    let err = fx.refused(&["data", "list", "ghost"]);
    assert!(err.contains("no ledger named 'ghost'"), "{err}");
    let fx2 = Fx::new();
    assert!(fx2.ok(&["data", "list"]).contains("no ledgers"));
}

#[test]
fn list_reports_a_ledger_whose_file_is_missing_or_faulty_without_failing() {
    let fx = Fx::new();
    new_sets(&fx);
    fx.remove(SETS);
    let out = fx.ok(&["data", "list"]);
    assert!(out.contains("fault: workspace/life/systems/fitness/sets.csv: the file is missing"), "{out}");
    fx.write(SETS, "date,exercise,set,weight,source\nnot-a-date,Squat,1,1,log\n");
    let out = fx.ok(&["data", "list"]);
    assert!(out.contains("1 rows") && out.contains("fault: workspace/life/systems/fitness/sets.csv:2: date:"), "{out}");
}

#[test]
fn data_verbs_are_logged_with_their_subcommand() {
    let fx = Fx::new();
    new_sets(&fx);
    add_set(&fx, "Squat", "1", "100");
    let verbs: Vec<String> = fx.run_log_rows().iter().map(|r| r["verb"].as_str().unwrap().to_string()).collect();
    assert_eq!(verbs, ["data new", "data add"]);
}

#[test]
fn column_may_be_repeated_or_given_several_values_after_one_flag() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "a", "--file", "a.csv", "--column", "x:text!", "y:integer", "--column", "z:date", "--key", "x"]);
    assert_eq!(fx.read("workspace/life/a.csv"), "x,y,z\n");
}
