//! `af data import --scope col=value...` and `--whole`: the file's rows replace the rows in scope (or the
//! whole ledger) in one write; a fault leaves the ledger file byte-identical.

mod common;
use common::*;

const SETS: &str = "workspace/life/systems/fitness/sets.csv";

fn new_sets(fx: &Fx) {
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "workout:text!", "--column", "set_no:integer!", "--column", "exercise:text!",
        "--column", "weight:number", "--key", "workout,set_no,exercise",
    ]);
}

/// Two workouts, written by hand so every line's bytes are known (quotes and a blank line stay).
fn seeded(fx: &Fx) {
    new_sets(fx);
    fx.write_bytes(
        SETS,
        b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,2,Squat,105\n\"w2\",1,Bench,70\nw2,2,\"Bench\",75\n",
    );
}

fn bytes(fx: &Fx) -> Vec<u8> {
    std::fs::read(fx.path(SETS)).unwrap()
}

fn csv(fx: &Fx, text: &str) -> String {
    fx.write("in/rows.csv", text)
}

#[test]
fn scope_replaces_the_rows_in_scope_and_leaves_the_others_byte_for_byte() {
    let fx = Fx::new();
    seeded(&fx);
    // w1 had two sets; the file has one changed set and one new one. The w1 set 2 row is absent: it goes.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\nw1,3,Squat,115\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 2, added 2, unchanged 0");
    assert_eq!(
        bytes(&fx),
        b"workout,set_no,exercise,weight\n\"w2\",1,Bench,70\nw2,2,\"Bench\",75\nw1,1,Squat,110\nw1,3,Squat,115\n",
        "the w2 lines keep their quoting; the w1 rows are the file's"
    );
    assert!(!fx.exists(&format!("{SETS}.lock")) && !fx.exists(&format!("{SETS}.tmp")), "no lock or temp file is left");
}

#[test]
fn scope_takes_several_pairs_and_matches_by_typed_value() {
    let fx = Fx::new();
    seeded(&fx);
    // set_no=01 is the 1 stored; only the w1 row with set 1 is in scope.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,01,Squat,120\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1", "set_no=01"]);
    assert_eq!(out.trim_end(), "removed 1, added 1, unchanged 0");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\nw1,2,Squat,105\n\"w2\",1,Bench,70\nw2,2,\"Bench\",75\nw1,1,Squat,120\n");
    // The scope may also be written as repeated flags.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw2,2,Bench,80\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w2", "--scope", "set_no=2"]);
    assert_eq!(out.trim_end(), "removed 1, added 1, unchanged 0");
}

#[test]
fn a_file_row_outside_the_scope_refuses_and_leaves_the_file_byte_identical() {
    let fx = Fx::new();
    seeded(&fx);
    let before = bytes(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\nw2,9,Row,50\nw1,3,Squat,115\n");
    let (code, out, err) = fx.run(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("rows.csv:3: is outside the scope (workout differs from workout=w1)"), "{err}");
    assert!(err.contains("nothing was written"), "{err}");
    assert_eq!(bytes(&fx), before);
    // A scope column the file does not carry puts every row outside the scope.
    let f = csv(&fx, "workout,set_no,exercise\nw1,1,Squat\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "weight=100"]);
    assert!(err.contains("rows.csv:2: is outside the scope (weight differs from weight=100)"), "{err}");
    assert_eq!(bytes(&fx), before);
    // So does a fault elsewhere in the file: a bad value, with the scope otherwise met.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\nw1,x,Squat,1\n");
    fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(bytes(&fx), before);
    // And a scope that is not col=value, or names no column, or a value of the wrong type.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\n");
    assert!(fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "colour=red"]).contains("no column 'colour'"));
    assert!(fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "set_no=abc"]).contains("set_no:"));
    assert_eq!(bytes(&fx), before);
}

#[test]
fn a_header_only_file_with_scope_removes_the_rows_in_scope() {
    let fx = Fx::new();
    seeded(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w2"]);
    assert_eq!(out.trim_end(), "removed 2, added 0, unchanged 0");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,2,Squat,105\n");
    // A scope that matches nothing, with a header-only file, writes nothing.
    let before = bytes(&fx);
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w9"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 0");
    assert_eq!(bytes(&fx), before);
}

#[test]
fn a_key_held_outside_the_scope_refuses_and_with_replace_merges() {
    let fx = Fx::new();
    // The key is exercise and set_no only, so a w1 row can clash with a w2 row.
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "workout:text!", "--column", "set_no:integer!", "--column", "exercise:text!",
        "--column", "weight:number", "--key", "exercise,set_no",
    ]);
    fx.write_bytes(SETS, b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw2,1,Bench,70\nw2,2,Bench,75\n");
    let before = bytes(&fx);
    // w1 is replaced by a file that now claims Bench set 1, which a w2 row holds.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,1,Bench,72\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert!(err.contains("rows.csv:3:") && err.contains("--replace replaces it"), "{err}");
    assert_eq!(bytes(&fx), before);
    // With --replace the clashing row is merged, so it moves into the scope; the count says so.
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1", "--replace"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 1, replaced 1");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,1,Bench,72\nw2,2,Bench,75\n");
}

#[test]
fn a_registered_page_is_rendered_once_after_the_write() {
    let fx = Fx::new();
    seeded(&fx);
    fx.write("workspace/life/p.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "p.md"]);
    assert!(fx.read("workspace/life/p.md").contains("| 4 |"));
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\n");
    fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert!(fx.read("workspace/life/p.md").contains("| n |\n|---|\n| 3 |\n"), "{}", fx.read("workspace/life/p.md"));
}

#[test]
fn scope_keeps_a_crlf_file_and_a_missing_final_newline() {
    let fx = Fx::new();
    seeded(&fx);
    fx.write_bytes(SETS, b"\xEF\xBB\xBFworkout,set_no,exercise,weight\r\nw1,1,Squat,100\r\nw2,1,Bench,70");
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,110\n");
    fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(bytes(&fx), b"\xEF\xBB\xBFworkout,set_no,exercise,weight\r\nw2,1,Bench,70\r\nw1,1,Squat,110\r\n");
}

#[test]
fn whole_makes_the_files_rows_the_ledger() {
    let fx = Fx::new();
    seeded(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\nw2,1,Bench,70\nw3,1,Row,50\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--whole"]);
    // w2 set 1 is in the file as it is in the ledger (its quotes stay); the other three rows go; one is new.
    assert_eq!(out.trim_end(), "removed 3, added 1, unchanged 1");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\n\"w2\",1,Bench,70\nw3,1,Row,50\n");
    // The same file again changes nothing.
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--whole"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 2");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\n\"w2\",1,Bench,70\nw3,1,Row,50\n");
}

#[test]
fn whole_with_a_bad_row_leaves_the_file_byte_identical() {
    let fx = Fx::new();
    seeded(&fx);
    let before = bytes(&fx);
    for text in [
        "workout,set_no,exercise,weight\nw3,1,Row,50\nw3,abc,Row,50\n",
        "workout,set_no,exercise,weight\nw3,1,Row,50\nw3,1,Row,60\n",
        "workout,set_no,exercise,weight\nw3,1,Row,50\nw3,2,Row\n",
        "workout,set_no,exercise,weight\nw3,1,Row,50\nw3,,Row,60\n",
    ] {
        let f = csv(&fx, text);
        let (code, out, err) = fx.run(&["data", "import", "sets", "--csv", &f, "--whole"]);
        assert_eq!(code, 1, "{text}: {out}{err}");
        assert_eq!(bytes(&fx), before, "{text}");
    }
}

#[test]
fn whole_is_refused_with_scope_with_replace_and_with_a_header_only_file() {
    let fx = Fx::new();
    seeded(&fx);
    let before = bytes(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\nw3,1,Row,50\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f, "--whole", "--scope", "workout=w1"]);
    assert!(err.contains("--whole") && err.contains("--scope"), "{err}");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f, "--whole", "--replace"]);
    assert!(err.contains("--whole") && err.contains("--replace"), "{err}");
    let f = csv(&fx, "workout,set_no,exercise,weight\n");
    let err = fx.refused(&["data", "import", "sets", "--csv", &f, "--whole"]);
    assert!(err.contains("af data delete"), "{err}");
    assert_eq!(bytes(&fx), before);
}

#[test]
fn without_scope_or_whole_import_still_only_adds() {
    let fx = Fx::new();
    seeded(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\nw3,1,Row,50\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f]);
    assert_eq!(out.trim_end(), "added 1, unchanged 0, replaced 0");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,2,Squat,105\n\"w2\",1,Bench,70\nw2,2,\"Bench\",75\nw3,1,Row,50\n");
}

#[test]
fn an_empty_scope_value_is_refused_before_anything_is_read_and_delete_keeps_matching_empty() {
    let fx = Fx::new();
    seeded(&fx);
    // Two rows with an empty weight, and a file that would replace them.
    fx.write_bytes(SETS, b"workout,set_no,exercise,weight\nw1,1,Squat,\nw1,2,Squat,\nw2,1,Bench,70\n");
    let before = bytes(&fx);
    let f = csv(&fx, "workout,set_no,exercise,weight\nw3,1,Row,50\n");
    for value in ["weight=", "weight= "] {
        let (code, out, err) = fx.run(&["data", "import", "sets", "--csv", &f, "--scope", value]);
        assert_eq!(code, 1, "{value:?}: {out}{err}");
        assert!(err.contains("a scope needs one") && err.contains("af data delete sets weight="), "{err}");
        assert_eq!(bytes(&fx), before, "{value:?}");
    }
    // It is refused before the file is read: a file that is not there gives the scope message, not a read error.
    let err = fx.refused(&["data", "import", "sets", "--csv", &fx.p("in/none.csv"), "--scope", "weight="]);
    assert!(err.contains("a scope needs one"), "{err}");
    // A good pair next to an empty one is refused too.
    fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1", "weight="]);
    assert_eq!(bytes(&fx), before);
    // delete still matches the empty values, on purpose.
    let out = fx.ok(&["data", "delete", "sets", "weight="]);
    assert_eq!(out.lines().next().unwrap(), "deleted 2");
}

#[test]
fn the_counts_printed_account_for_every_row_of_the_file() {
    let fx = Fx::new();
    fx.ok(&[
        "data", "new", "sets", "--file", "systems/fitness/sets.csv", "--column", "workout:text!", "--column", "set_no:integer!", "--column", "exercise:text!",
        "--column", "weight:number", "--key", "exercise,set_no",
    ]);
    fx.write_bytes(SETS, b"workout,set_no,exercise,weight\nw1,1,Squat,100\nw2,1,Bench,70\nw2,2,Bench,75\n");
    let file_rows = 3;
    // One new row, one row that takes a key a w2 row holds (merged with --replace), one more new row.
    let f = csv(&fx, "workout,set_no,exercise,weight\nw1,1,Squat,100\nw1,1,Bench,72\nw1,2,Squat,105\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1", "--replace"]);
    let n = |word: &str| -> usize {
        out.split(", ").find_map(|p| p.trim().strip_prefix(word).and_then(|v| v.trim().parse().ok())).unwrap_or(0)
    };
    assert_eq!(n("added") + n("replaced") + n("unchanged"), file_rows, "{out}");
    // With --whole the three rows of the file are already in the ledger; the fourth row goes.
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--whole"]);
    assert_eq!(out.trim_end(), format!("removed 1, added 0, unchanged {file_rows}"));
}

// ------------------------------------------------------------------ rows the file repeats stay

/// Five w1 sets (one written with quotes) and one w2 set, by hand, with weights written as `60`.
fn many(fx: &Fx) {
    new_sets(fx);
    fx.write_bytes(
        SETS,
        b"workout,set_no,exercise,weight\nw1,1,Squat,60\n\"w1\",2,\"Squat\",65\nw1,3,Squat,70\nw1,4,Squat,75\nw1,5,Squat,80\nw2,1,Bench,50\n",
    );
}

const W1_ROWS: &str = "workout,set_no,exercise,weight\nw1,1,Squat,60\nw1,2,Squat,65\nw1,3,Squat,70\nw1,4,Squat,75\nw1,5,Squat,80\n";

fn mtime(fx: &Fx, rel: &str) -> std::time::SystemTime {
    std::fs::metadata(fx.path(rel)).unwrap().modified().unwrap()
}

#[test]
fn a_re_import_that_changes_nothing_writes_nothing_under_scope_and_under_whole() {
    let fx = Fx::new();
    many(&fx);
    // A page that is stale on purpose: a re-render would put the real count in it.
    fx.write("workspace/life/p.md", "<!-- af-data: select count(*) as n from sets -->\n<!-- /af-data -->\n");
    fx.ok(&["data", "page", "p.md"]);
    let page = fx.read("workspace/life/p.md").replace("| 6 |", "| 99 |");
    assert!(page.contains("| 99 |"), "{page}");
    fx.write("workspace/life/p.md", &page);
    let (before, file_time, page_time) = (bytes(&fx), mtime(&fx, SETS), mtime(&fx, "workspace/life/p.md"));
    std::thread::sleep(std::time::Duration::from_millis(60));
    let f = csv(&fx, W1_ROWS);
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 5");
    assert_eq!(bytes(&fx), before);
    assert_eq!(mtime(&fx, SETS), file_time, "the ledger file is not written");
    assert_eq!(fx.read("workspace/life/p.md"), page, "the page is not rendered");
    assert_eq!(mtime(&fx, "workspace/life/p.md"), page_time);
    // --whole, with the file holding every row of the ledger.
    let f = csv(&fx, &format!("{W1_ROWS}w2,1,Bench,50\n"));
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--whole"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 6");
    assert_eq!(bytes(&fx), before);
    assert_eq!(mtime(&fx, SETS), file_time);
    assert_eq!(fx.read("workspace/life/p.md"), page);
    assert_eq!(mtime(&fx, "workspace/life/p.md"), page_time);
}

#[test]
fn one_changed_row_among_many_moves_alone_and_the_other_lines_keep_their_bytes() {
    let fx = Fx::new();
    many(&fx);
    // Set 3 changes weight; the rest of w1 is as it was.
    let f = csv(&fx, &W1_ROWS.replace("w1,3,Squat,70", "w1,3,Squat,72"));
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 1, added 1, unchanged 4");
    let want: &[u8] = b"workout,set_no,exercise,weight\nw1,1,Squat,60\n\"w1\",2,\"Squat\",65\nw1,4,Squat,75\nw1,5,Squat,80\nw2,1,Bench,50\nw1,3,Squat,72\n";
    assert_eq!(bytes(&fx), want);
    // --whole the same: one row of the file changes.
    let fx = Fx::new();
    many(&fx);
    let f = csv(&fx, &format!("{}w2,1,Bench,50\n", W1_ROWS.replace("w1,3,Squat,70", "w1,3,Squat,72")));
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--whole"]);
    assert_eq!(out.trim_end(), "removed 1, added 1, unchanged 5");
    assert_eq!(bytes(&fx), want);
}

#[test]
fn a_row_the_file_lacks_is_removed_and_a_row_it_adds_is_added() {
    let fx = Fx::new();
    many(&fx);
    let f = csv(&fx, &W1_ROWS.replace("w1,5,Squat,80\n", ""));
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 1, added 0, unchanged 4");
    assert_eq!(bytes(&fx), b"workout,set_no,exercise,weight\nw1,1,Squat,60\n\"w1\",2,\"Squat\",65\nw1,3,Squat,70\nw1,4,Squat,75\nw2,1,Bench,50\n");
    let fx = Fx::new();
    many(&fx);
    let f = csv(&fx, &format!("{W1_ROWS}w1,6,Squat,85\n"));
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 0, added 1, unchanged 5");
    assert_eq!(
        bytes(&fx),
        b"workout,set_no,exercise,weight\nw1,1,Squat,60\n\"w1\",2,\"Squat\",65\nw1,3,Squat,70\nw1,4,Squat,75\nw1,5,Squat,80\nw2,1,Bench,50\nw1,6,Squat,85\n"
    );
}

#[test]
fn equal_values_written_differently_or_in_another_column_order_are_the_same_row() {
    let fx = Fx::new();
    many(&fx);
    let (before, file_time) = (bytes(&fx), mtime(&fx, SETS));
    std::thread::sleep(std::time::Duration::from_millis(60));
    // 60.0 is 60 by value; set 01 is set 1; the columns come in another order.
    let f = csv(&fx, "weight,exercise,set_no,workout\n60.0,Squat,01,w1\n65,Squat,2,w1\n70.00,Squat,3,w1\n75,Squat,4,w1\n80,Squat,5,w1\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
    assert_eq!(out.trim_end(), "removed 0, added 0, unchanged 5");
    assert_eq!(bytes(&fx), before, "the ledger's own spelling stays");
    assert_eq!(mtime(&fx, SETS), file_time);
    // A row whose optional column the file leaves out is a different row from one that holds a value there.
    let f = csv(&fx, "workout,set_no,exercise\nw1,1,Squat\n");
    let out = fx.ok(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1", "set_no=1"]);
    assert_eq!(out.trim_end(), "removed 1, added 1, unchanged 0");
}

#[test]
fn a_fault_in_a_file_that_repeats_rows_leaves_the_ledger_byte_identical() {
    let fx = Fx::new();
    many(&fx);
    let before = bytes(&fx);
    // Repeated rows next to a bad one, a repeated key, and a row outside the scope: nothing is written.
    for text in [format!("{W1_ROWS}w1,6,Squat,abc\n"), format!("{W1_ROWS}w1,5,Squat,81\n"), format!("{W1_ROWS}w2,1,Bench,50\n")] {
        let f = csv(&fx, &text);
        fx.refused(&["data", "import", "sets", "--csv", &f, "--scope", "workout=w1"]);
        assert_eq!(bytes(&fx), before, "{text}");
    }
}
