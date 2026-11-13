//! `af watch add | list | result | close | sweep` on workspace/life/watch.md, plus the doctor notes
//! and the session-start line. Every name, text and path here is invented.

mod common;
mod life_fx;
use common::*;
use life_fx::*;
use serde_json::Value;

const WATCH: &str = "workspace/life/watch.md";
const OPEN: &str = "workspace/life/open.md";
const BATCH_LOG: &str = "system/logs/watch-batch.log";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn stamp(hours: i64) -> String {
    (chrono::Local::now().naive_local() - chrono::Duration::hours(hours)).format("%Y-%m-%d %H:%M").to_string()
}

fn id_stamp() -> String {
    today().replace('-', "")
}

fn id(n: u32) -> String {
    format!("w-{}-{n:02}", id_stamp())
}

fn ahead(days: i64) -> String {
    days_ahead(days)
}

/// A hand-built row in the queue's grammar, owner af-main, added 2026-11-01.
fn row(id: &str, when: &str, expires: &str, status: &str, result: &str, checked: &str, after: Option<&str>) -> String {
    let tier = if when.starts_with("email") { "haiku" } else { "code" };
    let after = after.map(|a| format!(" · after {a}")).unwrap_or_default();
    format!(
        "- [ ] {id} · owner af-main · added 2026-11-01{after} · expires {expires} · when {when} · do notify+loop \"Say {id}\" · tier {tier} · checked {checked} · result {result} · status {status}"
    )
}

fn open_row(id: &str, when: &str, expires: &str) -> String {
    row(id, when, expires, "open", "none", "never", None)
}

fn seed(fx: &Fx, rows: &[String]) {
    fx.write(WATCH, &format!("# Watch queue\n\n{}\n", rows.join("\n")));
}

fn seed_open(fx: &Fx) {
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\n## Waiting on others\n\n\
- [ ] 2026-11-01 · Get the shed permit quote · nudge {n} · from: main\n\
- [ ] 2026-11-01 · Get the garden quote · nudge {n} · from: main\n\
- [ ] 2026-11-01 · Water the ferns · nudge {n} · every weekly · from: main\n",
            n = ahead(5)
        ),
    );
}

fn list_json(fx: &Fx, args: &[&str]) -> Vec<Value> {
    let mut a = vec!["watch", "list", "--json"];
    a.extend_from_slice(args);
    serde_json::from_str::<Value>(&fx.ok(&a)).unwrap().as_array().unwrap().clone()
}

fn item<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    rows.iter().find(|r| r["id"] == id).unwrap_or_else(|| panic!("no row {id} in {rows:?}"))
}

fn ids(rows: &[Value]) -> Vec<String> {
    rows.iter().map(|r| r["id"].as_str().unwrap().to_string()).collect()
}

fn usage_error(fx: &Fx, args: &[&str]) -> String {
    let (code, out, err) = fx.run(args);
    assert_eq!(code, 2, "af {args:?} expected a usage error, got {code}: {out}{err}");
    err
}

fn add(fx: &Fx, args: &[&str]) -> String {
    let mut a = vec!["watch", "add"];
    a.extend_from_slice(args);
    fx.ok(&a)
}

fn add_refused(fx: &Fx, args: &[&str]) -> String {
    let mut a = vec!["watch", "add"];
    a.extend_from_slice(args);
    fx.refused(&a)
}

// ------------------------------------------------------------------ add

#[test]
fn add_writes_one_row_per_condition_kind_with_its_tier() {
    let fx = Fx::new();
    seed_open(&fx);
    fx.write("data/a.txt", "one");
    let date = format!("{}T10:00", ahead(3));
    add(&fx, &["--date", &date, "--do", "notify", "--say", "Return the books"]);
    add(&fx, &["--loop-closed", "Get the shed", "--do", "loop", "--say", "Quote is in"]);
    add(&fx, &["--loop-open", "Get the garden", "--on", &ahead(4), "--do", "loop", "--say", "Still no garden quote"]);
    add(&fx, &["--file-exists", "inbox/scan.pdf", "--do", "notify+loop", "--say", "Scan landed"]);
    add(&fx, &["--file-changed", "data/a.txt", "--do", "notify", "--say", "Data changed"]);
    add(&fx, &["--email", "from:rail.example subject:refund", "--means", "the refund was issued", "--do", "notify", "--say", "Refund in"]);
    let text = fx.read(WATCH);
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("- [ ] ")).collect();
    assert_eq!(rows.len(), 6, "{text}");
    assert!(rows[0].contains(&format!("when date {} 10:00 · do notify \"Return the books\" · tier code · checked never · result none · status open", ahead(3))), "{}", rows[0]);
    assert!(rows[1].contains(&format!("when loop closed \"Get the shed permit quote\" nudge {} · do loop \"Quote is in\" · tier code", ahead(5))), "{}", rows[1]);
    assert!(rows[2].contains(&format!("when loop open-on {} \"Get the garden quote\" nudge {} · do loop", ahead(4), ahead(5))), "{}", rows[2]);
    assert!(rows[3].contains("when file exists \"inbox/scan.pdf\" · do notify+loop \"Scan landed\" · tier code"), "{}", rows[3]);
    assert!(rows[4].contains("when file changed \"data/a.txt\" sha256 7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed · do notify"), "{}", rows[4]);
    assert!(rows[5].contains("when email \"from:rail.example subject:refund\" means \"the refund was issued\" · do notify \"Refund in\" · tier haiku"), "{}", rows[5]);
    assert!(rows.iter().all(|r| r.starts_with("- [ ] w-") && r.contains("· owner af-main · added ")));
}

#[test]
fn add_prints_the_row_and_the_owner_note_only_when_the_owner_defaulted() {
    let fx = Fx::new();
    let out = add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A landed"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].starts_with(&format!("- [ ] {} · owner af-main · ", id(1))), "{out}");
    assert_eq!(lines[1], "owner af-main; a side chat passes --owner <its name>");
    let out = add(&fx, &["--file-exists", "inbox/b.pdf", "--do", "notify", "--say", "B landed", "--owner", "life/travel"]);
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("owner life/travel"), "{out}");
}

#[test]
fn add_stores_after_and_then_and_defaults_the_expiry() {
    let fx = Fx::new();
    seed_open(&fx);
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A landed"]);
    let rows = list_json(&fx, &[]);
    assert_eq!(rows[0]["expires"], ahead(14), "14 days by default");

    add(&fx, &["--file-exists", "inbox/b.pdf", "--do", "notify", "--say", "B landed", "--expires", "+5d"]);
    assert_eq!(list_json(&fx, &[])[1]["expires"], ahead(5));

    add(&fx, &["--email", "from:x.example", "--means", "x replied", "--do", "loop", "--say", "X replied", "--after", "+20d", "--then", "Read the reply"]);
    let rows = list_json(&fx, &[]);
    assert_eq!(rows[2]["after"], ahead(20));
    assert_eq!(rows[2]["expires"], ahead(34), "14 days from the day it begins");
    assert_eq!(rows[2]["then"], "Read the reply");

    let date = ahead(30);
    add(&fx, &["--date", &date, "--do", "notify", "--say", "Far date"]);
    assert_eq!(list_json(&fx, &[])[3]["expires"], ahead(31), "a later date: that date plus a day");
    add(&fx, &["--date", &ahead(3), "--do", "notify", "--say", "Near date"]);
    assert_eq!(list_json(&fx, &[])[4]["expires"], ahead(14), "a near date keeps the 14 days");
    add(&fx, &["--loop-open", "Get the garden", "--on", &ahead(40), "--do", "loop", "--say", "Far loop"]);
    assert_eq!(list_json(&fx, &[])[5]["expires"], ahead(41));
}

#[test]
fn add_holds_the_ninety_day_cap_at_the_edge() {
    let fx = Fx::new();
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "Edge", "--expires", &ahead(90)]);
    let err = add_refused(&fx, &["--file-exists", "inbox/b.pdf", "--do", "notify", "--say", "Over", "--expires", &ahead(91)]);
    assert!(err.contains("more than 90 days out"), "{err}");
    let err = add_refused(&fx, &["--date", &ahead(95), "--do", "notify", "--say", "Far"]);
    assert!(err.contains("more than 90 days out"), "a date past the cap gives a default expiry past the cap: {err}");
}

#[test]
fn add_refuses_zero_or_several_conditions_and_half_conditions() {
    let fx = Fx::new();
    usage_error(&fx, &["watch", "add", "--do", "notify", "--say", "Nothing to watch"]);
    let err = usage_error(&fx, &["watch", "add", "--date", &ahead(2), "--file-exists", "x", "--do", "notify", "--say", "Two"]);
    assert!(err.contains("cannot be used with"), "{err}");
    usage_error(&fx, &["watch", "add", "--loop-open", "x", "--do", "notify", "--say", "No day"]);
    usage_error(&fx, &["watch", "add", "--email", "from:x", "--do", "notify", "--say", "No means"]);
    let err = add_refused(&fx, &["--means", "x", "--file-exists", "y", "--do", "notify", "--say", "Orphan"]);
    assert!(err.contains("--means goes with --email"), "{err}");
    let err = add_refused(&fx, &["--on", &ahead(2), "--file-exists", "y", "--do", "notify", "--say", "Orphan"]);
    assert!(err.contains("--on goes with --loop-open"), "{err}");
    assert!(!fx.exists(WATCH));
}

#[test]
fn add_refuses_bad_expiry_and_dates() {
    let fx = Fx::new();
    let base = ["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"];
    let with = |extra: &[&str]| -> Vec<String> { base.iter().chain(extra.iter()).map(|s| s.to_string()).collect() };
    let refuse = |extra: &[&str]| -> String {
        let args = with(extra);
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        add_refused(&fx, &refs)
    };
    assert!(refuse(&["--expires", &days_ago(1)]).contains("is in the past"));
    assert!(refuse(&["--expires", "tomorrow"]).contains("--expires takes YYYY-MM-DD or +Nd"));
    assert!(refuse(&["--after", &ahead(10), "--expires", &ahead(5)]).contains("before --after"));
    let err = add_refused(&fx, &["--date", &ahead(5), "--expires", &ahead(3), "--do", "notify", "--say", "Early"]);
    assert!(err.contains("before the condition's date"), "{err}");
    let err = add_refused(&fx, &["--date", &ahead(5), "--after", &ahead(2), "--do", "notify", "--say", "Both"]);
    assert!(err.contains("--after cannot go with --date"), "{err}");
    let err = add_refused(&fx, &["--date", &days_ago(1), "--do", "notify", "--say", "Gone"]);
    assert!(err.contains("has already passed"), "{err}");
    let past_today = format!("{}T00:00", today());
    let err = add_refused(&fx, &["--date", &past_today, "--do", "notify", "--say", "Midnight"]);
    assert!(err.contains("has already passed"), "{err}");
    let err = add_refused(&fx, &["--date", "next week", "--do", "notify", "--say", "Vague"]);
    assert!(err.contains("--date takes YYYY-MM-DD"), "{err}");
    let err = add_refused(&fx, &["--date", &format!("{}T25:99", ahead(2)), "--do", "notify", "--say", "Odd"]);
    assert!(err.contains("--date takes YYYY-MM-DD"), "{err}");
    assert!(!fx.exists(WATCH), "a refusal writes nothing");
}

#[test]
fn add_accepts_today_as_a_date_item_and_refuses_it_for_a_loop_open_in_the_past() {
    let fx = Fx::new();
    seed_open(&fx);
    add(&fx, &["--date", &today(), "--do", "notify", "--say", "Due today"]);
    let err = add_refused(&fx, &["--loop-open", "Get the garden", "--on", &days_ago(1), "--do", "loop", "--say", "Past"]);
    assert!(err.contains("has already passed"), "{err}");
}

#[test]
fn add_refuses_an_existing_file_a_directory_and_missing_or_ambiguous_loops() {
    let fx = Fx::new();
    seed_open(&fx);
    fx.write("inbox/here.pdf", "x");
    let err = add_refused(&fx, &["--file-exists", "inbox/here.pdf", "--do", "notify", "--say", "Here"]);
    assert!(err.contains("already exists"), "{err}");
    let err = add_refused(&fx, &["--file-changed", "inbox", "--do", "notify", "--say", "Dir"]);
    assert!(err.contains("is a directory"), "{err}");
    let err = add_refused(&fx, &["--loop-closed", "No such loop", "--do", "notify", "--say", "Missing"]);
    assert!(err.contains("no open loop starts with 'No such loop'"), "{err}");
    let (code, out, err) = fx.run(&["watch", "add", "--loop-closed", "Get the", "--do", "notify", "--say", "Which"]);
    assert_eq!(code, 1);
    assert!(err.contains("ambiguous prefix"), "{err}");
    assert!(out.contains("Get the shed permit quote") && out.contains("Get the garden quote"), "{out}");
    let fx2 = Fx::new();
    let err = add_refused(&fx2, &["--loop-closed", "Anything", "--do", "notify", "--say", "No file"]);
    assert!(err.contains("open.md not found"), "{err}");
    assert!(!fx.exists(WATCH) && !fx2.exists(WATCH));
}

#[test]
fn add_refuses_empty_long_and_separator_text_and_bad_actions() {
    let fx = Fx::new();
    let f = ["--file-exists", "inbox/a.pdf"];
    let go = |extra: &[&str]| -> String {
        let mut args: Vec<&str> = f.to_vec();
        args.extend_from_slice(extra);
        add_refused(&fx, &args)
    };
    assert!(go(&["--do", "notify", "--say", "   "]).contains("--say may not be empty"));
    assert!(go(&["--do", "notify", "--say", &"x".repeat(121)]).contains("121 characters; the limit is 120"));
    assert!(go(&["--do", "notify", "--say", "a · b"]).contains("may not contain ' · '"));
    assert!(go(&["--do", "notify", "--say", "a\nb"]).contains("may not contain a newline"));
    assert!(go(&["--do", "notify", "--say", "ok", "--then", &"y".repeat(161)]).contains("--then is 161 characters; the limit is 160"));
    assert!(go(&["--do", "notify", "--say", "ok", "--then", "a · b"]).contains("--then may not contain"));
    assert!(go(&["--do", "ring", "--say", "ok"]).contains("--do takes notify, loop, or notify+loop"));
    let err = add_refused(&fx, &["--email", "from:x · y", "--means", "z", "--do", "notify", "--say", "ok"]);
    assert!(err.contains("--email may not contain"), "{err}");
    let err = add_refused(&fx, &["--email", "from:x", "--means", "", "--do", "notify", "--say", "ok"]);
    assert!(err.contains("--means may not be empty"), "{err}");
    // 120 exactly is accepted
    add(&fx, &["--file-exists", "inbox/b.pdf", "--do", "notify", "--say", &"x".repeat(120)]);
}

#[test]
fn add_refuses_bad_and_worker_owners() {
    let fx = Fx::new();
    let base = ["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"];
    let go = |owner: &str| -> String {
        let mut args: Vec<&str> = base.to_vec();
        args.extend(["--owner", owner]);
        add_refused(&fx, &args)
    };
    assert!(go("somebody").contains("is not a chat name"));
    assert!(go("project/").contains("is not a chat name"));
    assert!(go("worker/job").contains("cannot own a watch item"));
    assert!(!fx.exists(WATCH));
    let mut args: Vec<&str> = vec!["watch", "add"];
    args.extend(base);
    args.extend(["--owner", "project/garden-shed"]);
    fx.ok(&args);
}

#[test]
fn add_refuses_a_duplicate_open_item_naming_its_id_but_allows_another_owner_or_a_firing_one() {
    let fx = Fx::new();
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"]);
    let err = add_refused(&fx, &["--file-exists", "inbox/a.pdf", "--do", "loop", "--say", "Different words"]);
    assert!(err.contains(&format!("{} already watches that for af-main", id(1))), "{err}");
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A", "--owner", "life/travel"]);
    fx.ok(&["watch", "result", &id(1), "met", "--detail", "file appeared"]);
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A again"]);
    assert_eq!(ids(&list_json(&fx, &[])), vec![id(1), id(2), id(3)]);
}

#[test]
fn ids_count_up_within_a_day_and_never_reuse_one_in_the_month_log() {
    let fx = Fx::new();
    for n in 1..=3 {
        let out = add(&fx, &["--file-exists", &format!("inbox/{n}.pdf"), "--do", "notify", "--say", &format!("Item {n}")]);
        assert!(out.starts_with(&format!("- [ ] {} ·", id(n))), "{out}");
    }
    fx.ok(&["watch", "close", &id(3), "--outcome", "no longer needed"]);
    let out = add(&fx, &["--file-exists", "inbox/4.pdf", "--do", "notify", "--say", "Item 4"]);
    assert!(out.starts_with(&format!("- [ ] {} ·", id(4))), "the closed id stays used: {out}");

    let fx = Fx::new();
    fx.write(&log_rel(), &format!("- {} 09:00 · event: watch {} expired\n", today(), id(7)));
    let out = add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"]);
    assert!(out.starts_with(&format!("- [ ] {} ·", id(8))), "a log entry counts: {out}");
}

#[test]
fn watch_md_keeps_prose_and_crlf_and_a_missing_file_is_an_empty_queue() {
    let fx = Fx::new();
    assert_eq!(fx.ok(&["watch", "list"]), "");
    assert_eq!(list_json(&fx, &[]).len(), 0);
    fx.write(WATCH, "# Watch queue\r\n\r\nFree prose stays.\r\n\r\n");
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"]);
    let raw = std::fs::read_to_string(fx.path(WATCH)).unwrap();
    assert!(raw.starts_with("# Watch queue\r\n\r\nFree prose stays.\r\n\r\n- [ ] w-"), "{raw:?}");
    assert!(!raw.replace("\r\n", "").contains('\n'));
}

// ------------------------------------------------------------------ list

#[test]
fn list_due_honours_after_and_expiry_edges_and_always_includes_firing_rows() {
    let fx = Fx::new();
    let t = today();
    seed(
        &fx,
        &[
            row("w-20261101-01", "file exists \"a\"", &ahead(3), "open", "none", "never", Some(ahead(1).as_str())), // not begun
            row("w-20261101-02", "file exists \"b\"", &ahead(3), "open", "none", "never", Some(t.as_str())),   // begins today
            row("w-20261101-03", "file exists \"c\"", &t, "open", "none", "never", None),                    // expires today
            row("w-20261101-04", "file exists \"d\"", &days_ago(1), "open", "none", "never", None),          // expired
            row("w-20261101-05", "file exists \"e\"", &days_ago(5), "firing", "met: x", &stamp(1), Some(ahead(9).as_str())), // firing, odd dates
            row("w-20261101-06", "file exists \"f\"", &ahead(3), "met", "met: x", &stamp(1), None),          // met, not due
            row("w-20261101-07", "file exists \"g\"", &ahead(3), "open", "none", "never", Some(days_ago(4).as_str())),
        ],
    );
    let all = list_json(&fx, &[]);
    assert_eq!(all.len(), 7);
    let due = list_json(&fx, &["--due"]);
    assert_eq!(ids(&due), vec!["w-20261101-02", "w-20261101-03", "w-20261101-05", "w-20261101-07"]);
    let text = fx.ok(&["watch", "list", "--due"]);
    assert_eq!(text.lines().count(), 4);
    assert!(text.lines().all(|l| l.starts_with("- [ ] w-20261101-")));
}

#[test]
fn list_filters_by_owner() {
    let fx = Fx::new();
    add(&fx, &["--file-exists", "inbox/a.pdf", "--do", "notify", "--say", "A"]);
    add(&fx, &["--file-exists", "inbox/b.pdf", "--do", "notify", "--say", "B", "--owner", "project/garden-shed"]);
    assert_eq!(ids(&list_json(&fx, &["--owner", "project/garden-shed"])), vec![id(2)]);
    assert_eq!(ids(&list_json(&fx, &["--owner", "af-main"])), vec![id(1)]);
    assert_eq!(list_json(&fx, &["--owner", "life/nobody"]).len(), 0);
}

#[test]
fn list_json_gives_every_field() {
    let fx = Fx::new();
    add(&fx, &["--email", "from:rail.example", "--means", "the refund was issued", "--do", "notify+loop", "--say", "Refund in", "--then", "Check the card", "--after", "+1d", "--owner", "life/travel"]);
    let r = list_json(&fx, &[]);
    let v = &r[0];
    assert_eq!(v["id"], id(1));
    assert_eq!(v["owner"], "life/travel");
    assert_eq!(v["added"], today());
    assert_eq!(v["after"], ahead(1));
    assert_eq!(v["kind"], "email");
    assert_eq!(v["query"], "from:rail.example");
    assert_eq!(v["means"], "the refund was issued");
    assert_eq!(v["do"], "notify+loop");
    assert_eq!(v["say"], "Refund in");
    assert_eq!(v["then"], "Check the card");
    assert_eq!(v["tier"], "haiku");
    assert!(v["checked"].is_null());
    assert_eq!(v["result"], "none");
    assert_eq!(v["status"], "open");
    assert!(v["code_met"].is_null(), "an email item is judged by the runner");
}

#[test]
fn code_met_for_a_date_item_follows_the_clock() {
    let fx = Fx::new();
    seed(
        &fx,
        &[
            open_row("w-20261101-01", &format!("date {} 00:00", days_ago(1)), &ahead(3)),
            open_row("w-20261101-02", &format!("date {}", today()), &ahead(3)),
            open_row("w-20261101-03", &format!("date {} 23:59", ahead(1)), &ahead(3)),
            open_row("w-20261101-04", &format!("date {}", ahead(2)), &ahead(3)),
        ],
    );
    let r = list_json(&fx, &[]);
    assert_eq!(item(&r, "w-20261101-01")["code_met"], true);
    assert_eq!(item(&r, "w-20261101-02")["code_met"], true, "a bare date is that day's midnight");
    assert_eq!(item(&r, "w-20261101-03")["code_met"], false);
    assert_eq!(item(&r, "w-20261101-04")["code_met"], false);
}

#[test]
fn code_met_for_a_closed_loop_and_for_a_recurring_loop_closed() {
    let fx = Fx::new();
    seed_open(&fx);
    add(&fx, &["--loop-closed", "Get the shed", "--do", "loop", "--say", "Shed quote in"]);
    add(&fx, &["--loop-closed", "Water the ferns", "--do", "notify", "--say", "Ferns watered"]);
    add(&fx, &["--loop-closed", "Get the garden", "--do", "notify", "--say", "Garden quote in"]);
    let r = list_json(&fx, &[]);
    assert_eq!(item(&r, &id(1))["code_met"], false, "still open");
    assert_eq!(item(&r, &id(2))["code_met"], false);

    fx.ok(&["open", "close", "Get the shed", "--outcome", "quote arrived"]);
    fx.ok(&["open", "close", "Water the ferns", "--outcome", "watered"]);
    let r = list_json(&fx, &[]);
    assert_eq!(item(&r, &id(1))["code_met"], true, "the loop is gone");
    assert_eq!(item(&r, &id(2))["code_met"], true, "a recurring loop closed is re-dated: its nudge moved past the stored one");
    assert_eq!(item(&r, &id(3))["code_met"], false);

    fx.ok(&["open", "redate", "Get the garden", "--nudge", &ahead(40)]);
    assert_eq!(item(&list_json(&fx, &[]), &id(3))["code_met"], true, "a moved nudge counts as moved past");
}

#[test]
fn a_missing_or_damaged_open_md_never_reads_as_a_closed_loop() {
    let fx = Fx::new();
    seed_open(&fx);
    add(&fx, &["--loop-closed", "Get the shed", "--do", "loop", "--say", "Shed quote in"]);
    fx.remove(OPEN);
    assert_eq!(list_json(&fx, &[])[0]["code_met"], false, "no open.md: cannot tell");
    fx.write(OPEN, "# Open loops\n\n- [ ] 24/10/2026 · Get the shed permit quote · from: main\n");
    assert_eq!(list_json(&fx, &[])[0]["code_met"], false, "a line that fails to parse may still be the loop");
    fx.write(OPEN, "# Open loops\n\n");
    assert_eq!(list_json(&fx, &[])[0]["code_met"], true);
}

#[test]
fn code_met_for_a_loop_still_open_on_a_day() {
    let fx = Fx::new();
    seed_open(&fx);
    add(&fx, &["--loop-open", "Get the shed", "--on", &today(), "--do", "loop", "--say", "Still no shed quote"]);
    add(&fx, &["--loop-open", "Get the garden", "--on", &ahead(3), "--do", "loop", "--say", "Still no garden quote"]);
    let r = list_json(&fx, &[]);
    assert_eq!(item(&r, &id(1))["code_met"], true, "its day has come and the loop is unchanged");
    assert_eq!(item(&r, &id(2))["code_met"], false, "its day has not come");
    fx.ok(&["open", "redate", "Get the shed", "--nudge", &ahead(30)]);
    assert_eq!(item(&list_json(&fx, &[]), &id(1))["code_met"], false, "a re-dated loop is not unchanged");
    fx.ok(&["open", "close", "Get the shed", "--outcome", "quote arrived"]);
    assert_eq!(item(&list_json(&fx, &[]), &id(1))["code_met"], false, "a closed loop is not still open");
}

#[test]
fn code_met_for_a_file_appearing_and_a_file_changing() {
    let fx = Fx::new();
    fx.write("data/a.txt", "one");
    add(&fx, &["--file-exists", "inbox/scan.pdf", "--do", "notify", "--say", "Scan landed"]);
    add(&fx, &["--file-changed", "data/a.txt", "--do", "notify", "--say", "A changed"]);
    add(&fx, &["--file-changed", "data/later.txt", "--do", "notify", "--say", "Later appeared"]);
    let r = list_json(&fx, &[]);
    assert!(r.iter().all(|v| v["code_met"] == false), "{r:?}");

    fx.write("inbox/scan.pdf", "pdf");
    fx.write("data/a.txt", "two");
    fx.write("data/later.txt", "new");
    let r = list_json(&fx, &[]);
    assert!(r.iter().all(|v| v["code_met"] == true), "{r:?}");

    fx.write("data/a.txt", "one");
    assert_eq!(item(&list_json(&fx, &[]), &id(2))["code_met"], false, "back to the stored content");
    assert!(fx.read(WATCH).contains("sha256 none"), "a file absent at add time stores none");
}

#[test]
fn an_absolute_path_is_taken_as_it_is() {
    let fx = Fx::new();
    let other = Fx::new();
    let target = other.p("elsewhere/x.txt");
    add(&fx, &["--file-exists", &target, "--do", "notify", "--say", "Elsewhere"]);
    assert_eq!(list_json(&fx, &[])[0]["code_met"], false);
    other.write("elsewhere/x.txt", "x");
    assert_eq!(list_json(&fx, &[])[0]["code_met"], true);
}

// ------------------------------------------------------------------ result

fn one_open(fx: &Fx) -> String {
    add(fx, &["--file-exists", "inbox/a.pdf", "--do", "notify+loop", "--say", "A landed"]);
    id(1)
}

#[test]
fn result_not_met_stamps_checked_and_keeps_the_item_open() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let out = fx.ok(&["watch", "result", &i, "not-met"]);
    assert!(out.contains("· result not met · status open"), "{out}");
    let r = list_json(&fx, &[]);
    assert_eq!(r[0]["result"], "not met");
    assert!(recent(&r[0]["checked"].as_str().unwrap().replace(' ', "T")), "{:?}", r[0]["checked"]);
    assert_eq!(fx.read(WATCH).lines().filter(|l| l.starts_with("- [ ]")).count(), 1);
}

#[test]
fn result_met_marks_the_item_firing_and_fired_moves_it_to_met_without_a_new_stamp() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let out = fx.ok(&["watch", "result", &i, "met", "--detail", "file appeared"]);
    assert!(out.contains("· result met: file appeared · status firing"), "{out}");
    let before = list_json(&fx, &[])[0]["checked"].clone();
    assert!(before.is_string());
    set_stamp_back(&fx, &i);
    let stamped = list_json(&fx, &[])[0]["checked"].clone();
    let out = fx.ok(&["watch", "result", &i, "fired"]);
    assert!(out.contains("· result met: file appeared · status met"), "{out}");
    assert_eq!(list_json(&fx, &[])[0]["checked"], stamped, "fired does not stamp checked");
}

/// Rewrite the row's checked stamp to two hours ago so a later stamp would show.
fn set_stamp_back(fx: &Fx, id: &str) {
    let text = fx.read(WATCH);
    let re = regex_replace_checked(&text, id, &stamp(2));
    fx.write(WATCH, &re);
}

fn regex_replace_checked(text: &str, id: &str, new: &str) -> String {
    text.lines()
        .map(|l| {
            if l.contains(id) {
                let i = l.find(" · checked ").unwrap() + " · checked ".len();
                let j = l[i..].find(" · result ").unwrap() + i;
                format!("{}{}{}", &l[..i], new, &l[j..])
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn result_error_needs_a_detail_and_error_twice_records_it() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let err = fx.refused(&["watch", "result", &i, "error"]);
    assert!(err.contains("needs --detail"), "{err}");
    assert!(fx.refused(&["watch", "result", &i, "error", "--detail", "  "]).contains("needs --detail"));
    let out = fx.ok(&["watch", "result", &i, "error", "--detail", "gmail refused the search"]);
    assert!(out.contains("· result error: gmail refused the search · status open"), "{out}");
    let out = fx.ok(&["watch", "result", &i, "error-twice", "--detail", "sonnet failed too"]);
    assert!(out.contains("· result error twice: sonnet failed too · status open"), "{out}");
    let r = list_json(&fx, &[]);
    assert_eq!(r[0]["status"], "open", "an error leaves the item queued for the next run");
    assert!(r[0]["checked"].is_string());
}

#[test]
fn result_detail_is_kept_to_one_short_line_without_the_field_separator() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let out = fx.ok(&["watch", "result", &i, "met", "--detail", "email from a@b.example · 2026-11-13\nsecond line"]);
    assert!(out.contains("result met: email from a@b.example ; 2026-11-13 second line · status firing"), "{out}");
    let rows = list_json(&fx, &[]);
    assert_eq!(rows.len(), 1, "the row still parses");
    let fx = Fx::new();
    let i = one_open(&fx);
    fx.ok(&["watch", "result", &i, "met", "--detail", &"d".repeat(500)]);
    let r = list_json(&fx, &[]);
    assert_eq!(r[0]["result"].as_str().unwrap().len(), "met: ".len() + 200);
}

#[test]
fn result_refuses_wrong_states_and_unknown_ids() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let err = fx.refused(&["watch", "result", &i, "fired"]);
    assert!(err.contains("not firing"), "{err}");
    fx.ok(&["watch", "result", &i, "met", "--detail", "d"]);
    for outcome in ["met", "not-met", "error-twice"] {
        let err = fx.refused(&["watch", "result", &i, outcome, "--detail", "d"]);
        assert!(err.contains("is firing, not open"), "{outcome}: {err}");
    }
    fx.ok(&["watch", "result", &i, "fired"]);
    let err = fx.refused(&["watch", "result", &i, "fired"]);
    assert!(err.contains("is met, not firing"), "{err}");
    assert!(fx.refused(&["watch", "result", &i, "met"]).contains("is met, not open"));
    assert!(fx.refused(&["watch", "result", "w-20200101-01", "not-met"]).contains("no watch item"));
    usage_error(&fx, &["watch", "result", &i, "exploded"]);
}

// ------------------------------------------------------------------ close

#[test]
fn close_logs_the_event_and_removes_the_row() {
    let fx = Fx::new();
    let i = one_open(&fx);
    let out = fx.ok(&["watch", "close", &i, "--outcome", "handled by hand"]);
    assert!(out.contains(&format!("af watch close: {i} A landed → handled by hand")), "{out}");
    assert!(!fx.read(WATCH).contains(&i));
    let log = fx.read(&log_rel());
    assert!(log.contains(&format!("· event: watch {i} closed: A landed → handled by hand")), "{log}");
    assert!(fx.refused(&["watch", "close", &i, "--outcome", "again"]).contains("no watch item"));
}

#[test]
fn close_refuses_a_firing_row_and_an_empty_outcome_but_closes_a_met_row() {
    let fx = Fx::new();
    let i = one_open(&fx);
    assert!(fx.refused(&["watch", "close", &i, "--outcome", " "]).contains("--outcome needs the result"));
    fx.ok(&["watch", "result", &i, "met", "--detail", "d"]);
    let err = fx.refused(&["watch", "close", &i, "--outcome", "nope"]);
    assert!(err.contains("is firing"), "{err}");
    assert!(fx.read(WATCH).contains(&i), "a refusal changes nothing");
    assert!(!fx.exists(&log_rel()));
    fx.ok(&["watch", "result", &i, "fired"]);
    fx.ok(&["watch", "close", &i, "--outcome", "done"]);
    assert!(!fx.read(WATCH).contains(&i));
}

// ------------------------------------------------------------------ sweep

fn raised_loops(fx: &Fx) -> Vec<String> {
    fx.read(OPEN).lines().filter(|l| l.contains("· Watch w-")).map(|l| l.to_string()).collect()
}

fn sweep_fixture() -> Fx {
    let fx = Fx::new();
    seed_open(&fx);
    let (e1, e2) = ("w-20261101-01", "w-20261101-02");
    seed(
        &fx,
        &[
            open_row(e1, "file exists \"a\"", &days_ago(1)),
            open_row(e2, "file exists \"b\"", &days_ago(3)),
            open_row("w-20261101-03", "file exists \"c\"", &ahead(3)),
            row("w-20261101-04", "file exists \"d\"", &days_ago(9), "firing", "met: x", &stamp(1), None),
            row("w-20261101-05", "file exists \"e\"", &ahead(3), "met", "met: y", &stamp(2), None),
            open_row("w-20261101-06", "file exists \"f\"", &today()),
        ],
    );
    fx
}

#[test]
fn sweep_expires_old_rows_with_one_loop_each_clears_met_rows_and_leaves_firing_ones() {
    let fx = sweep_fixture();
    let out = fx.ok(&["watch", "sweep"]);
    assert!(out.contains("w-20261101-01 expired (loop raised)"), "{out}");
    assert!(out.contains("w-20261101-05 met, removed"), "{out}");
    assert!(out.contains("2 expired, 1 met removed, 1 firing left"), "{out}");

    let left = ids(&list_json(&fx, &[]));
    assert_eq!(left, vec!["w-20261101-03", "w-20261101-04", "w-20261101-06"], "expiring today is not expired; firing stays");

    let loops = raised_loops(&fx);
    assert_eq!(loops.len(), 2, "{loops:?}");
    assert!(loops[0].contains("Watch w-20261101-01 expired without firing for af-main: Say w-20261101-01 [w-20261101-01]"), "{}", loops[0]);
    assert!(loops[0].contains(&format!("· nudge {} · from: watch", today())), "{}", loops[0]);
    let open = fx.read(OPEN);
    let heading = open.find("## Watch").expect("the Watch section exists");
    assert!(open[heading..].contains("w-20261101-02"), "{open}");

    let log = fx.read(&log_rel());
    for line in ["event: watch w-20261101-01 expired", "event: watch w-20261101-02 expired", "event: watch w-20261101-05 met"] {
        assert_eq!(log.matches(line).count(), 1, "{line}: {log}");
    }
    assert!(!log.contains("w-20261101-04"), "a firing row is not logged");
}

#[test]
fn a_rerun_of_sweep_raises_nothing_and_logs_nothing_more() {
    let fx = sweep_fixture();
    fx.ok(&["watch", "sweep"]);
    let (open, log, watch) = (fx.read(OPEN), fx.read(&log_rel()), fx.read(WATCH));
    let out = fx.ok(&["watch", "sweep"]);
    assert!(out.contains("0 expired, 0 met removed, 1 firing left"), "{out}");
    assert_eq!((fx.read(OPEN), fx.read(&log_rel()), fx.read(WATCH)), (open, log, watch));
}

#[test]
fn a_sweep_that_crashed_after_raising_the_loop_adds_no_second_loop() {
    let fx = Fx::new();
    seed(&fx, &[open_row("w-20261101-01", "file exists \"a\"", &days_ago(2))]);
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\n## Watch\n\n- [ ] {} · Watch w-20261101-01 expired without firing for af-main: Say w-20261101-01 [w-20261101-01] · nudge {} · from: watch\n",
            today(),
            today()
        ),
    );
    fx.write(&log_rel(), &format!("- {} 08:00 · event: watch w-20261101-01 expired\n", today()));
    let out = fx.ok(&["watch", "sweep"]);
    assert!(out.contains("expired (loop already raised)"), "{out}");
    assert_eq!(raised_loops(&fx).len(), 1);
    assert_eq!(fx.read(&log_rel()).matches("watch w-20261101-01 expired").count(), 1, "logged once");
    assert_eq!(list_json(&fx, &[]).len(), 0);
}

#[test]
fn sweep_json_lists_what_it_did_and_an_empty_queue_is_quiet() {
    let fx = sweep_fixture();
    let v: Value = serde_json::from_str(&fx.ok(&["watch", "sweep", "--json"])).unwrap();
    assert_eq!(v["expired"], serde_json::json!(["w-20261101-01", "w-20261101-02"]));
    assert_eq!(v["met"], serde_json::json!(["w-20261101-05"]));
    assert_eq!(v["loops_raised"], serde_json::json!(["w-20261101-01", "w-20261101-02"]));
    assert_eq!(v["firing"], serde_json::json!(["w-20261101-04"]));
    let fx = Fx::new();
    let v: Value = serde_json::from_str(&fx.ok(&["watch", "sweep", "--json"])).unwrap();
    assert_eq!(v["expired"], serde_json::json!([]));
    assert!(!fx.exists(WATCH), "sweeping nothing creates nothing");
}

// ------------------------------------------------------------------ doctor and session start

fn notes(fx: &Fx) -> Vec<String> {
    afcore::watch::doctor_notes(&fx.ctx()).into_iter().filter(|n| !n.contains("phone path")).collect()
}

#[test]
fn doctor_notes_a_row_that_does_not_parse_with_its_line_number() {
    let fx = Fx::new();
    fx.write(BATCH_LOG, "run
");
    let good = open_row("w-20261101-01", "file exists \"a\"", &ahead(3));
    fx.write(WATCH, &format!("# Watch queue\n\n{good}\n- [ ] w-20261101-02 · owner af-main · expires soon\n- [x] w-20261101-03 · done\n<!-- - [ ] ignored -->\n"));
    let n = notes(&fx);
    assert_eq!(n.len(), 2, "{n:?}");
    assert!(n[0].starts_with("workspace/life/watch.md:4: row does not parse"), "{}", n[0]);
    assert!(n[1].starts_with("workspace/life/watch.md:5: row does not parse"), "{}", n[1]);
    assert_eq!(ids(&list_json(&fx, &[])), vec!["w-20261101-01"], "the good row still reads");
}

#[test]
fn doctor_notes_an_overdue_item_a_stale_firing_item_and_a_stale_batch_log() {
    let fx = Fx::new();
    seed(
        &fx,
        &[
            open_row("w-20261101-01", "file exists \"a\"", &days_ago(1)),
            open_row("w-20261101-02", "file exists \"b\"", &days_ago(2)),
            row("w-20261101-03", "file exists \"c\"", &ahead(3), "firing", "met: x", &stamp(25), None),
            row("w-20261101-04", "file exists \"d\"", &ahead(3), "firing", "met: x", &stamp(23), None),
        ],
    );
    fx.write(BATCH_LOG, "run\n");
    set_mtime(&fx, BATCH_LOG, -29 * 3600);
    let n = notes(&fx);
    assert_eq!(n.len(), 2, "one day past expiry is not yet a note; a firing item under a day is fine: {n:?}");
    assert!(n[0].contains("w-20261101-02 is 2 days past its expiry"), "{}", n[0]);
    assert!(n[1].contains("w-20261101-03 has been firing since"), "{}", n[1]);

    set_mtime(&fx, BATCH_LOG, -31 * 3600);
    let n = notes(&fx);
    assert_eq!(n.len(), 3, "{n:?}");
    assert!(n[2].starts_with("system/logs/watch-batch.log: no batch run since"), "{}", n[2]);
}

#[test]
fn doctor_says_a_queue_with_no_batch_log_has_never_run_and_an_empty_queue_is_quiet() {
    let fx = Fx::new();
    assert!(notes(&fx).is_empty());
    seed(&fx, &[open_row("w-20261101-01", "file exists \"a\"", &ahead(3))]);
    let n = notes(&fx);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("does not exist and items are queued"), "{}", n[0]);
    fx.write(BATCH_LOG, "run\n");
    assert!(notes(&fx).is_empty(), "a fresh log is a live batch run");
}

fn doctor_without_the_phone_key(fx: &Fx) -> String {
    let out = fx.af().env_remove("REMINDERS_DISCORD_CHANNEL_ID").env_remove("REMINDERS_DISCORD_WEBHOOK").env_remove("DISCORD_BOT_TOKEN").arg("doctor").output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn doctor_notes_the_missing_phone_key_by_name_and_never_prints_a_secret() {
    let fx = Fx::new();
    assert!(!doctor_without_the_phone_key(&fx).contains("phone path"), "no notifier here, no note");
    fx.write("system/tools/notify.py", "# stub\n");
    let out = doctor_without_the_phone_key(&fx);
    let line = out.lines().find(|l| l.contains("phone path")).unwrap_or_else(|| panic!("{out}"));
    assert!(line.contains("REMINDERS_DISCORD_CHANNEL_ID") && line.contains("system/tools/.env"), "{line}");

    fx.write("system/tools/.env", "DISCORD_BOT_TOKEN=sekrit-token\nOPEN_DISCORD_CHANNEL_ID=7\n");
    let out = doctor_without_the_phone_key(&fx);
    assert!(out.contains("phone path"), "{out}");
    assert!(!out.contains("sekrit-token"), "{out}");

    fx.write("system/tools/.env", "DISCORD_BOT_TOKEN=sekrit-token\nREMINDERS_DISCORD_CHANNEL_ID=42\n");
    assert!(!doctor_without_the_phone_key(&fx).contains("phone path"));
    fx.write("system/tools/.env", "# c\nREMINDERS_DISCORD_WEBHOOK=\"https://example.invalid/h\"\n");
    assert!(!doctor_without_the_phone_key(&fx).contains("phone path"), "a webhook serves alone");
    fx.write("system/tools/.env", "DISCORD_BOT_TOKEN=\nREMINDERS_DISCORD_CHANNEL_ID=42\n");
    assert!(doctor_without_the_phone_key(&fx).contains("phone path"), "an empty token is no token");
}

fn bundle(fx: &Fx) -> String {
    fx.ok(&["session", "bundle", "--session", "s-main"])
}

#[test]
fn session_start_for_main_carries_one_counts_only_line_when_the_queue_is_stuck() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    assert!(!bundle(&fx).contains("watch queue"), "a quiet queue prints nothing");
    seed(
        &fx,
        &[
            open_row("w-20261101-02", "email \"from:secret.example\" means \"private words\"", &days_ago(3)),
            row("w-20261101-03", "file exists \"c\"", &ahead(3), "firing", "met: x", &stamp(30), None),
        ],
    );
    fx.write(BATCH_LOG, "run\n");
    let out = bundle(&fx);
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("watch queue")).collect();
    assert_eq!(lines.len(), 1, "{out}");
    assert!(lines[0].contains("1 past expiry") && lines[0].contains("1 firing for over a day"), "{}", lines[0]);
    assert!(!out.contains("secret.example") && !out.contains("private words"), "item text stays in the queue: {out}");

    fx.ok(&["watch", "sweep"]);
    fx.ok(&["watch", "result", "w-20261101-03", "fired"]);
    assert!(!bundle(&fx).contains("watch queue"), "a swept queue with a live batch is quiet");
}

#[test]
fn session_start_for_a_side_chat_does_not_carry_the_queue_line() {
    let fx = Fx::new();
    fx.ok(&["session", "register", "project/garden-shed", "--session", "s-side"]);
    seed(&fx, &[open_row("w-20261101-01", "file exists \"a\"", &days_ago(5))]);
    let out = fx.ok(&["session", "bundle", "--session", "s-side"]);
    assert!(!out.contains("watch queue"), "{out}");
}

#[test]
fn the_buttons_note_names_the_watch_verb_in_the_run_log() {
    let fx = Fx::new();
    one_open(&fx);
    fx.refused(&["watch", "result", &id(1), "fired"]);
    let rows = fx.run_log_rows();
    assert!(rows.iter().any(|r| r["verb"] == "watch result" && r["refusal"].is_string()), "{rows:?}");
}
