//! Recurring loops: `af open add --every <monthly|weekly|Nd>` writes `· every <period>`;
//! `af open close` logs the close and re-dates the same loop one period on.

mod common;
mod life_fx;
use afcore::life::doctor_checks;
use common::*;
use life_fx::*;

const OPEN: &str = "workspace/life/open.md";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn loops(fx: &Fx) -> Vec<String> {
    fx.read(OPEN).lines().filter(|l| l.starts_with("- [ ]")).map(str::to_string).collect()
}

#[test]
fn a_monthly_loop_closes_into_the_log_and_comes_back_a_month_on() {
    let fx = Fx::new();
    let out = fx.ok(&["open", "add", "pay the card", "--nudge", "2026-11-15", "--every", "monthly", "--from", "src-1"]);
    let line = format!("- [ ] {} · pay the card · nudge 2026-11-15 · every monthly · from: src-1", today());
    assert_eq!(out.trim(), line);
    assert_eq!(loops(&fx), vec![line]);

    let out = fx.ok(&["open", "close", "pay the card", "--outcome", "paid in full"]);
    assert!(out.contains("next nudge 2026-12-15"), "{out}");
    assert_eq!(
        loops(&fx),
        vec![format!("- [ ] {} · pay the card · nudge 2026-12-15 · every monthly · from: src-1", today())],
        "the same loop stays, re-dated one month on"
    );
    let log = fx.read(&log_rel());
    assert!(
        regex::Regex::new(r"\A- \d{4}-\d{2}-\d{2} \d{2}:\d{2} · closed: pay the card → paid in full \(next nudge 2026-12-15\)\n\z")
            .unwrap()
            .is_match(&log),
        "{log}"
    );
}

#[test]
fn close_final_ends_a_recurring_loop_and_says_so_in_the_log() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        "## Waiting on the operator\n\n\
- [ ] 2026-01-02 · gym membership · nudge 2026-10-28 · every monthly · from: main\n\
- [ ] 2026-01-02 · call mum · from: main\n",
    );
    let out = fx.ok(&["open", "close", "--final", "gym", "--outcome", "cancelled the membership"]);
    assert!(out.contains("gym membership → cancelled the membership (final"), "{out}");
    assert_eq!(loops(&fx), vec!["- [ ] 2026-01-02 · call mum · from: main"], "the recurring loop is gone");
    let log = fx.read(&log_rel());
    assert!(log.ends_with(" · closed: gym membership → cancelled the membership (final)\n"), "{log}");

    fx.ok(&["open", "close", "call mum", "--final", "--outcome", "called"]);
    assert!(loops(&fx).is_empty(), "--final on a one-off loop is a plain close");
    assert!(fx.read(&log_rel()).ends_with(" · closed: call mum → called\n"));
}

#[test]
fn monthly_clamps_to_month_end_and_weekly_and_n_days_add_days() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        "## Waiting on the operator\n\n\
- [ ] 2026-01-02 · rent · nudge 2026-01-31 · every monthly · from: main\n\
- [ ] 2026-01-02 · bins · nudge 2026-10-28 · every weekly · from: main\n\
- [ ] 2026-01-02 · water plants · nudge 2026-10-28 · every 10d · from: main\n",
    );
    fx.ok(&["open", "close", "rent", "--outcome", "paid"]);
    fx.ok(&["open", "close", "bins", "--outcome", "out"]);
    fx.ok(&["open", "close", "water", "--outcome", "watered"]);
    assert_eq!(
        loops(&fx),
        vec![
            "- [ ] 2026-01-02 · rent · nudge 2026-02-28 · every monthly · from: main",
            "- [ ] 2026-01-02 · bins · nudge 2026-11-04 · every weekly · from: main",
            "- [ ] 2026-01-02 · water plants · nudge 2026-11-07 · every 10d · from: main",
        ]
    );
    assert_eq!(fx.read(&log_rel()).lines().filter(|l| l.contains("· closed: ")).count(), 3);
}

#[test]
fn redate_keeps_the_period_and_due_shows_it() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        &format!(
            "## Waiting on the operator\n\n\
- [ ] {a} · renew prescription · nudge {n} · every 28d · from: main\n\
- [ ] {a} · one-off · nudge {n} · from: main\n",
            a = days_ago(30),
            n = days_ago(1),
        ),
    );
    let due = fx.ok(&["open", "due"]);
    assert!(due.contains("· every 28d ·"), "{due}");
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["open", "due", "--json"])).unwrap();
    assert_eq!(v[0]["every"], "28d");
    assert!(v[1]["every"].is_null(), "a one-off loop has no period");

    fx.ok(&["open", "redate", "renew", "--nudge", "+3d"]);
    assert!(loops(&fx)[0].ends_with(&format!("nudge {} · every 28d · from: main", days_ahead(3))), "{:?}", loops(&fx));
}

#[test]
fn add_refuses_a_period_without_a_nudge_or_an_unknown_period() {
    let fx = Fx::new();
    let err = fx.refused(&["open", "add", "x", "--every", "monthly"]);
    assert!(err.contains("--every needs --nudge"), "{err}");
    let err = fx.refused(&["open", "add", "x", "--nudge", "+1d", "--every", "fortnightly"]);
    assert!(err.contains("--every takes monthly, weekly, or Nd"), "{err}");
    fx.refused(&["open", "add", "x", "--nudge", "+1d", "--every", "0d"]);
    let err = fx.refused(&["open", "add", "pay rent · every monthly", "--nudge", "+1d"]);
    assert!(err.contains("grammar"), "{err}");
    assert!(!fx.exists(OPEN));
}

#[test]
fn doctor_accepts_a_recurring_loop_and_reports_a_broken_period() {
    let fx = Fx::new();
    fx.write(
        OPEN,
        "# Open loops\n\n## Waiting on the operator\n\n\
- [ ] 2026-10-24 · pay the card · nudge 2026-11-15 · every monthly · from: main\n\
- [ ] 2026-10-24 · bins · nudge 2026-11-01 · every weekly · from: main\n\
- [ ] 2026-10-24 · plants · nudge 2026-11-01 · every 10d · from: main\n",
    );
    let (i, _) = doctor_checks(&fx.ctx()).unwrap();
    assert!(i.is_empty(), "{i:?}");

    fx.write(
        OPEN,
        "## Waiting on the operator\n\n\
- [ ] 2026-10-24 · bins · nudge 2026-11-01 · every fortnight · from: main\n\
- [ ] 2026-10-24 · rent · every monthly · from: main\n",
    );
    let (i, _) = doctor_checks(&fx.ctx()).unwrap();
    assert_eq!(i.len(), 2, "{i:?}");
    assert!(i[0].starts_with("workspace/life/open.md:3: line does not parse"), "{}", i[0]);
    assert!(i[1].starts_with("workspace/life/open.md:4: line does not parse"), "{}", i[1]);
}
