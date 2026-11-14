//! Review fixes for `af data`: `--replace` merges, keys compare typed values, a half-finished
//! `column` finishes on a re-run, an import refuses its own repeated keys, and the smaller points.

mod common;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";
const REGISTRY: &str = "workspace/life/ledgers.yaml";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "date:date!", "--column", "exercise:text!", "--column", "set:integer!",
        "--column", "weight:number", "--column", "note:text", "--key", "exercise,set",
    ]);
}

fn new_tx(fx: &Fx) {
    fx.ok(&["data", "new", "tx", "--file", "tx.csv", "--column", "id:text!", "--column", "amt:number!", "--key", "id"]);
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

fn add_ok(fx: &Fx, args: &[&str]) -> String {
    let mut a = vec!["data", "add", "sets"];
    a.extend_from_slice(args);
    fx.ok(&a)
}

// ------------------------------------------------------------------ 4: --replace merges

#[test]
fn add_replace_keeps_the_columns_it_is_not_given_and_an_explicit_empty_empties_one() {
    let fx = Fx::new();
    new_sets(&fx);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=2", "weight=50", "note=keep"]);
    let out = add_ok(&fx, &["--replace", "date=2026-11-14", "exercise=Squat", "set=2", "weight=55"]);
    assert!(out.contains("replaced 1 row"), "{out}");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,note\n2026-11-14,Squat,2,55,keep\n");
    add_ok(&fx, &["--replace", "date=2026-11-14", "exercise=Squat", "set=2", "note="]);
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,note\n2026-11-14,Squat,2,55,\n");
    // Given values equal to the row's: nothing to do, whatever else the row holds.
    add_ok(&fx, &["date=2026-11-15", "exercise=Bench", "set=1", "weight=70", "note=n"]);
    let out = add_ok(&fx, &["--replace", "exercise=Bench", "set=1", "weight=70"]);
    assert!(out.contains("nothing changed"), "{out}");
    assert!(fx.read(SETS).ends_with("2026-11-15,Bench,1,70,n\n"));
    // An explicit empty on a required column is refused and writes nothing.
    let before = bytes(&fx, SETS);
    let err = fx.refused(&["data", "add", "sets", "--replace", "exercise=Bench", "set=1", "date="]);
    assert!(err.contains("date is required"), "{err}");
    assert_eq!(bytes(&fx, SETS), before);
    // A new key under --replace is an ordinary new row and needs its required columns.
    let err = fx.refused(&["data", "add", "sets", "--replace", "exercise=Row", "set=1"]);
    assert!(err.contains("date is required"), "{err}");
}

#[test]
fn import_replace_sets_the_columns_in_the_header_and_keeps_the_ones_absent() {
    let fx = Fx::new();
    new_sets(&fx);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=1", "weight=100", "note=first"]);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=2", "weight=100", "note=second"]);
    let f = fx.write("in/a.csv", "exercise,set,date,weight\nSquat,1,2026-11-14,105\nSquat,2,2026-11-14,100\nSquat,3,2026-11-15,90\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f]);
    assert!(err.contains("a.csv:2: the key (exercise=Squat, set=1) is at line 2 of the ledger with different values"), "{err}");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--replace"]);
    assert_eq!(out.trim(), "added 1, unchanged 1, replaced 1", "{out}");
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,note\n2026-11-14,Squat,1,105,first\n2026-11-14,Squat,2,100,second\n2026-11-15,Squat,3,90,\n");
    // A column in the header, even empty, is set.
    let f = fx.write("in/b.csv", "exercise,set,note\nSquat,1,\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--replace"]);
    assert_eq!(out.trim(), "added 0, unchanged 0, replaced 1");
    assert!(fx.read(SETS).contains("2026-11-14,Squat,1,105,\n"), "{}", fx.read(SETS));
    // Without --replace, only the columns the input carries decide "unchanged".
    let f = fx.write("in/c.csv", "exercise,set,weight\nSquat,2,100\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim(), "added 0, unchanged 1, replaced 0");
}

// ------------------------------------------------------------------ 5: typed values

#[test]
fn an_integer_key_is_one_key_however_it_is_written_and_is_written_in_canonical_form() {
    let fx = Fx::new();
    new_sets(&fx);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=1"]);
    for dup in ["01", "001"] {
        let (code, _, err) = fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", &format!("set={dup}")]);
        assert_eq!(code, 1, "{dup}: {err}");
        assert!(err.contains("exists at line 2"), "{dup}: {err}");
    }
    assert_eq!(fx.run(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=+1"]).0, 1, "+1 is not an integer");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Squat", "set=01"]);
    assert!(err.contains("the key (exercise=Squat, set=1) exists at line 2"), "{err}");
    add_ok(&fx, &["date=2026-11-14", "exercise=Bench", "set=007", "weight=1"]);
    add_ok(&fx, &["date=2026-11-14", "exercise=Zero", "set=-0"]);
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,note\n2026-11-14,Squat,1,,\n2026-11-14,Bench,7,1,\n2026-11-14,Zero,0,,\n");
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Zero", "set=0"]);
    assert!(err.contains("exists at line 4"), "{err}");
    // Through import: 01 is the existing 1, unchanged; a new 08 is written as 8.
    let f = fx.write("in/a.csv", "date,exercise,set\n2026-11-14,Squat,01\n2026-11-14,Squat,08\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim(), "added 1, unchanged 1, replaced 0");
    assert!(fx.read(SETS).ends_with("2026-11-14,Squat,8,,\n"), "{}", fx.read(SETS));
}

#[test]
fn text_is_trimmed_when_written_and_a_key_that_differs_only_in_spaces_is_the_same_key() {
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id= abc ", "amt=5"]);
    assert_eq!(fx.read("workspace/life/tx.csv"), "id,amt\nabc,5\n");
    let err = fx.refused(&["data", "add", "tx", "id=abc  ", "amt=6"]);
    assert!(err.contains("exists at line 2"), "{err}");
    fx.ok(&["data", "add", "tx", "id=ABC", "amt=7"]);
    let f = fx.write("in/a.csv", "id,amt\n abc ,5\n");
    assert_eq!(fx.ok(&["data", "import", "tx", "--csv", &f]).trim(), "added 0, unchanged 1, replaced 0");
}

#[test]
fn a_number_is_written_as_given_but_compared_by_value() {
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id=a", "amt=5"]);
    let f = fx.write("in/a.csv", "id,amt\na,5.0\nb,2.50\n");
    assert_eq!(fx.ok(&["data", "import", "tx", "--csv", &f]).trim(), "added 1, unchanged 1, replaced 0", "5.0 is the 5 already there");
    assert_eq!(fx.read("workspace/life/tx.csv"), "id,amt\na,5\nb,2.50\n", "written as given");
    let out = fx.ok(&["data", "add", "tx", "--replace", "id=b", "amt=2.5"]);
    assert!(out.contains("nothing changed"), "{out}");
    // A number key compares by value too.
    fx.ok(&["data", "new", "px", "--file", "px.csv", "--column", "p:number!", "--column", "n:text", "--key", "p"]);
    fx.ok(&["data", "add", "px", "p=5"]);
    let err = fx.refused(&["data", "add", "px", "p=5.0"]);
    assert!(err.contains("exists"), "{err}");
    fx.ok(&["data", "add", "px", "p=-0.0", "n=x"]);
    assert_eq!(fx.read("workspace/life/px.csv"), "p,n\n5,\n-0.0,x\n", "written as given");
    assert!(fx.refused(&["data", "add", "px", "p=0"]).contains("exists at line 3"), "-0.0 is 0");
    assert!(fx.refused(&["data", "add", "px", "p=-0"]).contains("exists at line 3"));
}

#[test]
fn a_number_with_more_than_fifteen_significant_digits_is_refused() {
    let fx = Fx::new();
    new_tx(&fx);
    for bad in ["12345678901234567890", "1.2345678901234567", "0.1234567890123456", "123456789.1234567", "1234567890123", "1.23456789012345", "123456789012345"] {
        let (code, _, err) = fx.run(&["data", "add", "tx", "id=x", &format!("amt={bad}")]);
        assert_eq!(code, 1, "{bad}");
        assert!(err.contains("significant digits"), "{bad}: {err}");
    }
    for good in ["123456789012", "1.23456789012", "0.00001", "1000000000000000000", "1200.50000000000000000", "-0.000000000001", "0.123456789012"] {
        let (code, _, err) = fx.run(&["data", "add", "tx", &format!("id=n{good}"), &format!("amt={good}")]);
        assert_eq!(code, 0, "{good}: {err}");
    }
    let f = fx.write("in/a.csv", "id,amt\nz,12345678901234567890\n");
    assert!(fx.refused(&["data", "import", "tx", "--csv", &f]).contains("significant digits"));
}

#[test]
fn the_doctor_finds_keys_that_differ_only_in_how_they_are_written() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "ints", "--file", "s.csv", "--column", "set:integer!", "--column", "tag:text", "--key", "set"]);
    fx.ok(&["data", "new", "txt", "--file", "t.csv", "--column", "tag:text!", "--column", "p:number", "--key", "tag"]);
    fx.ok(&["data", "new", "nums", "--file", "n.csv", "--column", "p:number!", "--column", "q:text", "--key", "p"]);
    fx.write("workspace/life/s.csv", "set,tag
1,a
2,b
01,c
");
    fx.write("workspace/life/t.csv", "tag,p
abc,1
x,2
abc ,1
");
    fx.write("workspace/life/n.csv", "p,q
5,a
5.0,b
6,c
-0,d
0,e
");
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("workspace/life/s.csv:4: the key (set=01) repeats line 2"), "{out}");
    assert!(out.contains("workspace/life/t.csv:4: the key (tag=abc ) repeats line 2"), "{out}");
    assert!(out.contains("workspace/life/n.csv:3: the key (p=5.0) repeats line 2"), "{out}");
    assert!(out.contains("workspace/life/n.csv:6: the key (p=0) repeats line 5"), "{out}");
}

#[test]
fn a_file_with_keys_that_are_one_key_cannot_be_adopted() {
    let fx = Fx::new();
    fx.write("workspace/life/s.csv", "set,tag
1,a
01,b
");
    let err = fx.refused(&["data", "new", "ints", "--file", "s.csv", "--column", "set:integer!", "--column", "tag:text", "--key", "set"]);
    assert!(err.contains("s.csv:3: the key (set=01) repeats line 2"), "{err}");
    assert!(!fx.exists(REGISTRY));
}

// ------------------------------------------------------------------ 6: column that did not finish

fn half_state_file_ahead(fx: &Fx) {
    new_sets(fx);
    add_ok(fx, &["date=2026-11-14", "exercise=Squat", "set=1", "weight=100"]);
    fx.write(SETS, "date,exercise,set,weight,note,rpe\n2026-11-14,Squat,1,100,,\n");
}

#[test]
fn a_column_whose_header_is_written_but_not_its_registry_entry_is_finished_by_running_it_again() {
    let fx = Fx::new();
    half_state_file_ahead(&fx);
    let reg = fx.read(REGISTRY);
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Bench", "set=1"]);
    assert!(err.contains("af data column sets rpe:"), "the message names the command that finishes it: {err}");
    let err = fx.refused(&["data", "import", "sets", "--csv", &fx.write("in/a.csv", "date,exercise,set\n2026-11-14,Bench,1\n")]);
    assert!(err.contains("af data column sets rpe:"), "{err}");
    assert_eq!(fx.read(REGISTRY), reg);
    let file = bytes(&fx, SETS);
    let out = fx.ok(&["data", "column", "sets", "rpe:number"]);
    assert!(out.contains("rpe:number"), "{out}");
    assert_eq!(bytes(&fx, SETS), file, "the file already had it and is left alone");
    assert!(fx.read(REGISTRY).contains("{name: rpe, type: number}"));
    add_ok(&fx, &["date=2026-11-14", "exercise=Bench", "set=1", "rpe=8"]);
    assert!(fx.read(SETS).ends_with("2026-11-14,Bench,1,,,8\n"), "{}", fx.read(SETS));
    // Run again on the finished ledger: the plain refusal.
    assert!(fx.refused(&["data", "column", "sets", "rpe:number"]).contains("already has a column"));
}

#[test]
fn a_column_in_the_registry_but_not_the_header_is_finished_by_running_it_again() {
    let fx = Fx::new();
    new_sets(&fx);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=1", "weight=100"]);
    let reg = fx.read(REGISTRY).replace("      - {name: note, type: text}\n", "      - {name: note, type: text}\n      - {name: rpe, type: number}\n");
    fx.write(REGISTRY, &reg);
    let err = fx.refused(&["data", "add", "sets", "date=2026-11-14", "exercise=Bench", "set=1"]);
    assert!(err.contains("af data column sets rpe:number"), "{err}");
    let err = fx.refused(&["data", "column", "sets", "rpe:text"]);
    assert!(err.contains("number"), "a different type does not finish it: {err}");
    let err = fx.refused(&["data", "column", "sets", "other:text"]);
    assert!(err.contains("rpe"), "{err}");
    fx.ok(&["data", "column", "sets", "rpe:number"]);
    assert_eq!(fx.read(SETS), "date,exercise,set,weight,note,rpe\n2026-11-14,Squat,1,100,,\n");
    assert_eq!(fx.read(REGISTRY).matches("name: rpe").count(), 1);
    add_ok(&fx, &["date=2026-11-14", "exercise=Bench", "set=1", "rpe=8"]);
}

#[test]
fn a_half_state_that_does_not_match_the_column_asked_for_is_refused_and_writes_nothing() {
    let fx = Fx::new();
    half_state_file_ahead(&fx);
    fx.write(SETS, "date,exercise,set,weight,note,rpe\n2026-11-14,Squat,1,100,,x\n");
    let reg = fx.read(REGISTRY);
    let file = bytes(&fx, SETS);
    let err = fx.refused(&["data", "column", "sets", "rpe:number"]);
    assert!(err.contains("sets.csv:2: rpe:"), "the rows must fit the new column: {err}");
    let err = fx.refused(&["data", "column", "sets", "other:text"]);
    assert!(err.contains("header"), "{err}");
    assert_eq!((bytes(&fx, SETS), fx.read(REGISTRY)), (file, reg));
}

#[cfg(windows)]
#[test]
fn a_failed_registry_save_in_column_puts_the_file_back_as_it_was() {
    let fx = Fx::new();
    new_sets(&fx);
    add_ok(&fx, &["date=2026-11-14", "exercise=Squat", "set=1", "weight=100"]);
    let file = bytes(&fx, SETS);
    let reg = bytes(&fx, REGISTRY);
    // A read-only registry cannot be replaced by the rename that saves it.
    let mut perm = std::fs::metadata(fx.path(REGISTRY)).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(fx.path(REGISTRY), perm.clone()).unwrap();
    let (code, _, err) = fx.run(&["data", "column", "sets", "rpe:number"]);
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(fx.path(REGISTRY), perm).unwrap();
    assert_ne!(code, 0, "{err}");
    assert_eq!(bytes(&fx, SETS), file, "the file is back as it was");
    assert_eq!(bytes(&fx, REGISTRY), reg);
    fx.ok(&["data", "add", "sets", "date=2026-11-14", "exercise=Bench", "set=1"]);
}

// ------------------------------------------------------------------ 7: an import that repeats a key

#[test]
fn an_import_whose_own_rows_repeat_a_key_is_refused_with_or_without_replace_naming_both_lines() {
    let fx = Fx::new();
    new_tx(&fx);
    let f = fx.write("in/a.csv", "id,amt\nnew1,1\nok,3\nnew1,2\nOK,4\nnew1,1\n");
    let before = bytes(&fx, "workspace/life/tx.csv");
    for extra in [vec![], vec!["--replace"]] {
        let mut a = vec!["data", "import", "tx", "--csv", &f];
        a.extend(extra);
        let (code, out, err) = fx.run(&a);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.is_empty());
        assert!(err.contains("a.csv:4: the key (id=new1) repeats line 2 of the input"), "{err}");
        assert!(err.contains("a.csv:6: the key (id=new1) repeats line 2 of the input"), "{err}");
        assert!(!err.contains("OK"), "{err}");
        assert_eq!(bytes(&fx, "workspace/life/tx.csv"), before);
    }
    // The same row twice is a repeat too, and so is 5 against 5.0.
    let f = fx.write("in/b.csv", "id,amt\nx,5\nx,5.0\n");
    assert!(fx.refused(&["data", "import", "tx", "--csv", &f]).contains("b.csv:3: the key (id=x) repeats line 2 of the input"));
}

// ------------------------------------------------------------------ minors

#[test]
fn a_page_argument_must_end_in_md_so_a_table_never_goes_into_a_ledger() {
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id=a", "amt=1"]);
    let before = bytes(&fx, "workspace/life/tx.csv");
    fx.write("workspace/life/p.md", "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n");
    for args in [vec!["render", "tx.csv"], vec!["render", "p.md", "tx.csv"], vec!["page", "tx.csv"], vec!["page", "notes.txt"], vec!["render", "workspace/life/tx.csv"]] {
        let mut a = vec!["data"];
        a.extend(args.clone());
        let err = fx.refused(&a);
        assert!(err.contains(".md"), "{args:?}: {err}");
    }
    assert_eq!(bytes(&fx, "workspace/life/tx.csv"), before);
    assert_eq!(fx.read("workspace/life/p.md"), "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n", "a bad argument stops the render before any page");
    fx.write("workspace/life/Upper.MD", "<!-- af-data: select 1 as one -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "render", "Upper.MD"]);
}

#[test]
fn reals_print_with_twelve_significant_digits_no_trailing_zeros_and_never_in_exponent_form() {
    let fx = Fx::new();
    let one = |sql: &str| fx.ok(&["data", "sql", sql, "--csv"]).replace("\r\n", "\n").lines().nth(1).unwrap().to_string();
    assert_eq!(one("select 0.00001"), "0.00001");
    assert_eq!(one("select 0.1 + 0.2"), "0.3");
    assert_eq!(one("select 1.0/3"), "0.333333333333");
    assert_eq!(one("select 2.0/3"), "0.666666666667");
    assert_eq!(one("select 19.99 * 3"), "59.97");
    assert_eq!(one("select 123456789.01"), "123456789.01", "what is stored prints whole");
    assert_eq!(one("select 1234567.8901"), "1234567.8901");
    assert_eq!(one("select 0.123456789012"), "0.123456789012");
    for huge in ["select 1e300", "select 1e-300", "select 2.5e-7", "select -1.5e25", "select 6.02e23"] {
        let v = one(huge);
        assert!(!v.contains('e') && !v.contains('E'), "{huge} printed {v}");
    }
    assert_eq!(one("select 1e-7"), "0.0000001");
    assert_eq!(one("select -0.00001"), "-0.00001");
    assert_eq!(one("select 2.50"), "2.5");
    assert_eq!(one("select 1e20"), "100000000000000000000");
    assert_eq!(one("select 123456789012.0"), "123456789012", "the integer part is never rounded away");
    assert_eq!(one("select 100.0 / 80.5"), "1.24223602484");
    assert_eq!(one("select 0.0"), "0");
    assert_eq!(one("select -0.0"), "0");
    assert!(fx.ok(&["data", "sql", "select 0.00001 as x", "--json"]).contains("\"x\": 0.00001"));
}

#[test]
fn the_doctor_prints_why_the_registry_does_not_parse() {
    let fx = Fx::new();
    fx.write(REGISTRY, "ledgers:\n  - name: Bad Name\n    file: a.csv\n");
    let (_, out, _) = fx.run(&["doctor"]);
    assert!(out.contains("ledger name \"Bad Name\" must be lower-case letters"), "{out}");
    fx.write(REGISTRY, "ledgers: [oops\n");
    let (_, out, _) = fx.run(&["doctor"]);
    assert!(out.contains("ledgers.yaml does not parse"), "{out}");
}

#[test]
fn a_reader_waits_for_a_writer_holding_the_ledger_lock() {
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id=a", "amt=1"]);
    let lock = fx.path("workspace/life/tx.csv.lock");
    std::fs::write(&lock, "pid 1\n").unwrap();
    let release = lock.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(900));
        std::fs::remove_file(release).unwrap();
    });
    let start = std::time::Instant::now();
    let out = fx.ok(&["data", "sql", "select count(*) as n from tx"]);
    assert!(start.elapsed().as_millis() >= 700, "the query went on while the file was locked: {:?}", start.elapsed());
    assert!(out.contains("| 1 |"));
    t.join().unwrap();
    for args in [vec!["list"], vec!["list", "tx"]] {
        std::fs::write(&lock, "pid 1\n").unwrap();
        let release = lock.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(900));
            std::fs::remove_file(release).unwrap();
        });
        let start = std::time::Instant::now();
        let mut a = vec!["data"];
        a.extend(args);
        fx.ok(&a);
        assert!(start.elapsed().as_millis() >= 700);
        t.join().unwrap();
    }
}

#[test]
fn a_reader_creates_no_folder_for_a_ledger_whose_file_is_missing() {
    let fx = Fx::new();
    fx.ok(&["data", "new", "tx", "--file", "deep/er/tx.csv", "--column", "id:text!", "--key", "id"]);
    fx.rmtree("workspace/life/deep");
    let err = fx.refused(&["data", "sql", "select 1"]);
    assert!(err.contains("the file is missing"), "{err}");
    assert!(!fx.exists("workspace/life/deep"), "reading made a folder");
}

#[test]
fn a_failed_append_is_cut_back_to_the_length_before_it() {
    use std::io::Write;
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id=a", "amt=1"]);
    let path = fx.p("workspace/life/tx.csv");
    let before = std::fs::read(&path).unwrap();
    let r = afcore::data::ledger::append_bytes(&path, b"b,2\nc,3\n", |f, data| {
        f.write_all(&data[..5])?;
        Err(std::io::Error::other("disk full"))
    });
    assert!(r.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before, "no half row is left");
    afcore::data::ledger::append_bytes(&path, b"b,2\n", |f, data| f.write_all(data)).unwrap();
    assert!(fx.read("workspace/life/tx.csv").ends_with("a,1\nb,2\n"));
}

#[test]
fn a_block_inside_a_fenced_code_block_is_an_example_and_not_a_block() {
    let fx = Fx::new();
    new_tx(&fx);
    fx.ok(&["data", "add", "tx", "id=a", "amt=1"]);
    let example = "How to write one:\n\n```\n<!-- af-data: select count(*) as n from tx -->\nold example\n<!-- /af-data -->\n```\n\n~~~md\n<!-- af-data: select 1 -->\n~~~\n\n";
    fx.write("workspace/life/doc.md", example);
    let err = fx.refused(&["data", "page", "doc.md"]);
    assert!(err.contains("holds no <!-- af-data"), "{err}");
    let page = format!("{example}<!-- af-data: select count(*) as n from tx -->\nstale\n<!-- /af-data -->\n");
    fx.write("workspace/life/doc.md", &page);
    let out = fx.ok(&["data", "render", "doc.md"]);
    assert!(out.contains("1 block, updated"), "{out}");
    let now = fx.read("workspace/life/doc.md");
    assert!(now.starts_with(example), "the example is untouched: {now}");
    assert!(now.ends_with("<!-- af-data: select count(*) as n from tx -->\n| n |\n|---|\n| 1 |\n<!-- /af-data -->\n"), "{now}");
    // A fence left open swallows the rest of the page, as in markdown.
    fx.write("workspace/life/open.md", "```\n<!-- af-data: select 1 -->\n");
    let err = fx.refused(&["data", "page", "open.md"]);
    assert!(err.contains("holds no <!-- af-data"), "{err}");
}
