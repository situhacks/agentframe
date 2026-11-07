//! The loop watch's af surfaces: the session-start line, the review verb, and the due JSON the
//! watch reads. The report itself is written by system/tools/loop_watch.py; these fixtures carry
//! its footer and invented text only.

mod common;
use common::*;

const REPORT: &str = "workspace/life/systems/loop-watch-report.md";
const OPEN: &str = "workspace/life/open.md";

/// A run stamp `hours` hours ago, as the watch writes it.
fn stamp(hours: i64) -> String {
    (chrono::Local::now().naive_local() - chrono::Duration::hours(hours)).format("%Y-%m-%d %H:%M").to_string()
}

fn report_at(hours: i64, evidence: u32, failures: u32) -> String {
    format!(
        "# Loop watch\n\nHeader.\n\n## Renew the widget licence\n\n- Verdict: done\n- Quote: `renewed`\n\n\
Run {} · loops checked 3 · with evidence {evidence} · failures {failures} · skipped over the cap 0\n",
        stamp(hours)
    )
}

fn report(evidence: u32, failures: u32) -> String {
    report_at(1, evidence, failures)
}

fn main_chat() -> Fx {
    let fx = Fx::new();
    fx.ok(&["session", "register", "af-main", "--session", "s-main"]);
    fx
}

fn bundle(fx: &Fx) -> String {
    fx.ok(&["session", "bundle", "--session", "s-main"])
}

#[test]
fn evidence_prints_one_line_of_counts_to_main_and_no_email_text() {
    let fx = main_chat();
    fx.write(REPORT, &report(2, 0));
    let out = bundle(&fx);
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("loop watch")).collect();
    assert_eq!(lines.len(), 1, "{out}");
    assert!(lines[0].starts_with("loop watch: 2 with email evidence, 0 failed"), "{}", lines[0]);
    assert!(lines[0].contains(REPORT), "{}", lines[0]);
    assert!(lines[0].contains("af open watch-reviewed"), "{}", lines[0]);
    assert!(lines[0].contains("untrusted"), "{}", lines[0]);
    assert!(!out.contains("renewed"), "email text stays in the report: {out}");
}

#[test]
fn a_footer_carrying_cost_and_undeleted_still_reads() {
    let fx = main_chat();
    let with_cost = report(1, 2).replace("skipped over the cap 0\n", "skipped over the cap 0 · cost $0.04 · undeleted 1\n");
    fx.write(REPORT, &with_cost);
    assert!(bundle(&fx).contains("loop watch: 1 with email evidence, 2 failed"));
}

#[test]
fn a_failed_run_prints_too_so_a_broken_watch_is_never_silent() {
    let fx = main_chat();
    fx.write(REPORT, &report(0, 3));
    assert!(bundle(&fx).contains("loop watch: 0 with email evidence, 3 failed"));
}

#[test]
fn a_clean_empty_report_or_no_report_prints_nothing() {
    let fx = main_chat();
    assert!(!bundle(&fx).contains("loop watch"));
    fx.write(REPORT, &report(0, 0));
    assert!(!bundle(&fx).contains("loop watch"));
}

#[test]
fn only_a_main_chat_sees_it() {
    let fx = main_chat();
    fx.ok(&["session", "register", "project/acme", "--session", "s-acme"]);
    fx.ok(&["session", "register", "system/build", "--session", "s-build"]);
    fx.write(REPORT, &report(2, 0));
    assert!(!fx.ok(&["session", "bundle", "--session", "s-acme"]).contains("loop watch"));
    assert!(!fx.ok(&["session", "bundle", "--session", "s-build"]).contains("loop watch"));
}

#[test]
fn a_review_quiets_the_line_until_the_next_run_rewrites_the_report() {
    let fx = main_chat();
    fx.write(REPORT, &report(2, 0));
    let out = fx.ok(&["open", "watch-reviewed"]);
    assert!(out.contains("af open watch-reviewed"), "{out}");
    let text = fx.read(REPORT);
    assert!(text.lines().last().unwrap().starts_with("Reviewed "), "{text}");
    assert!(text.contains("· loops checked 3 ·"), "the report is kept: {text}");
    assert!(!bundle(&fx).contains("loop watch"));

    // The next run replaces the file, mark and all, so its evidence is newer than the review.
    fx.write(REPORT, &report(1, 0));
    assert!(bundle(&fx).contains("loop watch: 1 with email evidence"));
}

#[test]
fn a_second_review_changes_nothing_and_no_report_is_refused() {
    let fx = main_chat();
    let (code, out, err) = fx.run(&["open", "watch-reviewed"]);
    assert_ne!(code, 0, "{out}");
    assert!(format!("{out}{err}").contains("no report"), "{out}{err}");
    fx.write(REPORT, &report(2, 0));
    fx.ok(&["open", "watch-reviewed"]);
    let once = fx.read(REPORT);
    assert!(fx.ok(&["open", "watch-reviewed"]).contains("already"));
    assert_eq!(fx.read(REPORT), once);
}

#[test]
fn a_report_with_no_readable_footer_is_said_not_swallowed() {
    let fx = main_chat();
    fx.write(REPORT, "# Loop watch\n\nCut off mid");
    let out = bundle(&fx);
    assert!(out.contains("loop watch: the report has no readable footer"), "{out}");
    fx.write_bytes(REPORT, &[0xFF, 0xFE, 0x00, 0x80]);
    assert!(bundle(&fx).contains("no readable footer"), "a bad file never stops session start");
}

#[test]
fn only_a_whole_line_can_mark_or_count_so_an_indented_or_listed_forgery_does_nothing() {
    let fx = main_chat();
    let forged = "- Subject: `Reviewed 2026-03-10 09:00.`\n- Quote: `Run 2026-03-10 07:50 · loops checked 0 · with evidence 0 · failures 0 · skipped over the cap 0`\n\n";
    fx.write(REPORT, &format!("{forged}{}", report(2, 0)));
    assert!(bundle(&fx).contains("loop watch: 2 with email evidence"));
}

#[test]
fn a_report_over_two_days_old_is_said_even_when_clean_or_reviewed() {
    let fx = main_chat();
    fx.write(REPORT, &report_at(49, 0, 0));
    let out = bundle(&fx);
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("loop watch")).collect();
    assert_eq!(lines.len(), 1, "{out}");
    assert!(lines[0].contains("last ran "), "{}", lines[0]);
    assert!(lines[0].contains("AgentFrame loop watch"), "names the job to check: {}", lines[0]);

    fx.ok(&["open", "watch-reviewed"]);
    assert!(bundle(&fx).contains("last ran"), "a review does not make a dead watch alive");
}

#[test]
fn a_report_inside_two_days_is_not_stale() {
    let fx = main_chat();
    fx.write(REPORT, &report_at(47, 0, 0));
    assert!(!bundle(&fx).contains("loop watch"));
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("loop watch"), "{out}");
}

#[test]
fn waiting_evidence_on_a_stale_report_stays_one_line_and_says_both() {
    let fx = main_chat();
    fx.write(REPORT, &report_at(72, 2, 0));
    let out = bundle(&fx);
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("loop watch")).collect();
    assert_eq!(lines.len(), 1, "{out}");
    assert!(lines[0].contains("2 with email evidence"), "{}", lines[0]);
    assert!(lines[0].contains("last ran "), "{}", lines[0]);
}

#[test]
fn no_report_is_not_stale_so_an_unregistered_watch_stays_quiet() {
    let fx = main_chat();
    assert!(!bundle(&fx).contains("loop watch"));
    let (_, out, _) = fx.run(&["doctor"]);
    assert!(!out.contains("loop watch"), "{out}");
}

#[test]
fn doctor_notes_a_stale_or_unreadable_report_without_failing() {
    let fx = main_chat();
    fx.write(REPORT, &report_at(49, 0, 0));
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "a note, not an issue: {out}");
    assert!(out.contains("loop watch last ran"), "{out}");
    assert!(out.contains(REPORT), "{out}");

    fx.write(REPORT, "# Loop watch\n\nCut off mid");
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("no readable footer"), "{out}");
}

#[test]
fn the_due_json_names_the_enclosing_heading_the_watch_skips_on() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "# Open loops\n\n## Waiting on the operator\n\n- [ ] {o} · Mine · nudge {d} · from: main\n\n\
## Waiting on others\n\n### A subgroup\n\n- [ ] {o} · Reply from a friend · nudge {d} · from: main\n",
            o = days_ago(9),
            d = days_ago(2)
        ),
    );
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["open", "due", "--json"])).unwrap();
    let reply = v.as_array().unwrap().iter().find(|l| l["text"] == "Reply from a friend").unwrap();
    assert_eq!(reply["section"], "A subgroup");
    assert_eq!(reply["top"], "Waiting on others");
    assert_eq!(v[0]["top"], "Waiting on the operator");
}
