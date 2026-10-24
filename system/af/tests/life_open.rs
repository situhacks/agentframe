//! `af open add | due | close` on workspace/life/open.md and the month log.

mod common;
mod life_fx;
use common::*;
use life_fx::*;

const OPEN: &str = "workspace/life/open.md";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn seeded() -> Fx {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\nFree prose stays put.\n\n## Waiting on the operator\n\n\
- [ ] {a} · renew lease · nudge {n1} · from: src-2026-10-23-02\n\
- [ ] {b} · book dentist · nudge {n2} · from: main\n\
- [ ] {c} · book flights · from: main\n\n\
## Waiting on others\n\n\
- [ ] {d} · refund from shop · nudge {fut} · from: main\n\
- [ ] {e} · book club reply · nudge {today} · from: life/people\n",
            a = days_ago(10),
            n1 = days_ago(3),
            b = days_ago(20),
            n2 = days_ago(1),
            c = days_ago(5),
            d = days_ago(2),
            fut = days_ahead(4),
            e = days_ago(1),
            today = today()
        ),
    );
    fx
}

#[test]
fn add_creates_the_file_and_the_default_section() {
    let fx = Fx::new();
    let out = fx.ok(&["open", "add", "call the landlord"]);
    let line = format!("- [ ] {} · call the landlord · from: main", today());
    assert_eq!(out.trim(), line);
    assert_eq!(fx.read(OPEN), format!("## Waiting on the operator\n\n{line}\n"));
}

#[test]
fn add_appends_at_the_end_of_its_section_and_touches_nothing_else() {
    let fx = seeded();
    let before = fx.read(OPEN);
    let out = fx.ok(&["open", "add", "chase invoice", "--nudge", "+3d", "--from", "src-9", "--section", "Waiting on others"]);
    let line = format!("- [ ] {} · chase invoice · nudge {} · from: src-9", today(), days_ahead(3));
    assert_eq!(out.trim(), line);
    let after = fx.read(OPEN);
    let mut expect: Vec<&str> = before.lines().collect();
    expect.push(&line);
    assert_eq!(after.lines().collect::<Vec<_>>(), expect);

    fx.ok(&["open", "add", "file taxes", "--nudge", "2026-12-01"]);
    let after = fx.read(OPEN);
    let lines: Vec<&str> = after.lines().collect();
    let i = lines.iter().position(|l| l.contains("file taxes")).unwrap();
    assert!(lines[i - 1].contains("book flights"), "lands after the section's last loop");
    assert!(lines[i].ends_with("· nudge 2026-12-01 · from: main"));
}

#[test]
fn add_preserves_crlf_and_creates_a_missing_section() {
    let fx = Fx::new();
    fx.write(OPEN, "# Open loops\r\n\r\n## Waiting on the operator\r\n\r\n- [ ] 2026-10-01 · x · from: main\r\n");
    fx.ok(&["open", "add", "new thing", "--section", "Health"]);
    let raw = std::fs::read_to_string(fx.path(OPEN)).unwrap();
    assert!(raw.starts_with("# Open loops\r\n\r\n## Waiting on the operator\r\n\r\n- [ ] 2026-10-01 · x · from: main\r\n\r\n## Health\r\n\r\n- [ ] "));
    assert!(!raw.replace("\r\n", "").contains('\n'));
}

#[test]
fn add_refuses_bad_nudges_and_grammar_breaking_text() {
    let fx = Fx::new();
    fx.refused(&["open", "add", "x", "--nudge", "tomorrow"]);
    fx.refused(&["open", "add", "x", "--nudge", "2026-13-40"]);
    let err = fx.refused(&["open", "add", "pay rent · from: bob"]);
    assert!(err.contains("grammar"), "{err}");
    fx.refused(&["open", "add", "   "]);
    assert!(!fx.exists(OPEN));
}

#[test]
fn due_lists_nudged_loops_oldest_first_with_age() {
    let fx = seeded();
    let out = fx.ok(&["open", "due"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "{out}");
    assert!(lines[0].contains("book dentist") && lines[0].ends_with("(20d, #2)"), "{}", lines[0]);
    assert!(lines[1].contains("renew lease") && lines[1].ends_with("(10d, #1)"));
    assert!(lines[2].contains("book club reply") && lines[2].ends_with("(1d, #5)"));
    assert!(!out.contains("refund") && !out.contains("book flights"));

    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["open", "due", "--json"])).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);
    assert_eq!(v[0]["text"], "book dentist");
    assert_eq!(v[0]["age_days"], 20);
    assert_eq!(v[0]["n"], 2);
    assert_eq!(v[0]["section"], "Waiting on the operator");
    assert_eq!(v[2]["from"], "life/people");
}

#[test]
fn due_without_open_md_is_empty_and_exits_zero() {
    let fx = Fx::new();
    assert_eq!(fx.ok(&["open", "due"]), "");
    assert_eq!(fx.ok(&["open", "due", "--json"]).trim(), "[]");
}

#[test]
fn close_by_number_moves_the_loop_to_the_month_log() {
    let fx = seeded();
    let before = fx.read(OPEN);
    let out = fx.ok(&["open", "close", "3", "--outcome", "booked for March"]);
    assert!(out.contains("book flights → booked for March"), "{out}");
    let after = fx.read(OPEN);
    let expect: Vec<&str> = before.lines().filter(|l| !l.contains("book flights")).collect();
    assert_eq!(after.lines().collect::<Vec<_>>(), expect);
    let log = fx.read(&log_rel());
    assert!(
        regex::Regex::new(r"\A- \d{4}-\d{2}-\d{2} \d{2}:\d{2} · closed: book flights → booked for March\n\z").unwrap().is_match(&log),
        "{log}"
    );
}

#[test]
fn close_by_prefix_appends_to_an_existing_log() {
    let fx = seeded();
    fx.write(&log_rel(), "- 2026-10-01 09:00 · decision: keep the flat\n");
    fx.ok(&["open", "close", "RENEW", "--outcome", "signed"]);
    let log = fx.read(&log_rel());
    assert!(log.starts_with("- 2026-10-01 09:00 · decision: keep the flat\n- "));
    assert!(log.ends_with(" · closed: renew lease → signed\n"));
    assert!(!fx.read(OPEN).contains("renew lease"));
}

#[test]
fn close_refuses_an_ambiguous_prefix_and_prints_the_candidates() {
    let fx = seeded();
    let before = fx.read(OPEN);
    let (code, out, err) = fx.run(&["open", "close", "book", "--outcome", "x"]);
    assert_eq!(code, 1);
    assert!(err.contains("ambiguous prefix 'book' matches 3 open loops"), "{err}");
    for c in ["#2", "book dentist", "#3", "book flights", "#5", "book club reply"] {
        assert!(out.contains(c), "{c} missing from {out}");
    }
    assert_eq!(fx.read(OPEN), before);
    assert!(!fx.exists(&log_rel()));
    fx.ok(&["open", "close", "book d", "--outcome", "done"]);
}

#[test]
fn close_prefers_an_exact_text_match_and_refuses_misses() {
    let fx = Fx::new();
    fx.write(OPEN, "## Waiting on the operator\n\n- [ ] 2026-10-01 · book · from: main\n- [ ] 2026-10-01 · book flights · from: main\n");
    fx.ok(&["open", "close", "book", "--outcome", "read it"]);
    assert!(fx.read(OPEN).contains("book flights"));
    fx.refused(&["open", "close", "zebra", "--outcome", "x"]);
    fx.refused(&["open", "close", "9", "--outcome", "x"]);
    fx.refused(&["open", "close", "book", "--outcome", " "]);
}
