//! `af open redate`: the nudge date changes in place; opened date, text, section, and
//! source stay; the month log records it.

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
            "# Open loops\n\n## Waiting on the operator\n\n\
- [ ] {a} · renew lease · nudge {n1} · from: src-2026-10-23-02\n\
- [ ] {b} · book flights · from: main\n\n\
## Waiting on others\n\n\
- [ ] {c} · refund from shop · nudge {fut} · from: main\n",
            a = days_ago(10),
            n1 = days_ago(3),
            b = days_ago(5),
            c = days_ago(2),
            fut = days_ahead(4),
        ),
    );
    fx
}

#[test]
fn redate_by_prefix_keeps_opened_text_section_and_from() {
    let fx = seeded();
    let out = fx.ok(&["open", "redate", "renew", "--nudge", &days_ahead(7)]);
    assert!(out.contains(&format!("#1 renew lease → nudge {}", days_ahead(7))), "{out}");
    assert!(out.contains(&format!("was {}", days_ago(3))), "{out}");
    let text = fx.read(OPEN);
    let want = format!("- [ ] {} · renew lease · nudge {} · from: src-2026-10-23-02", days_ago(10), days_ahead(7));
    assert!(text.contains(&want), "{text}");
    assert!(!text.contains(&format!("nudge {}", days_ago(3))), "{text}");
    let before = text.find("## Waiting on the operator").unwrap();
    let after = text.find("## Waiting on others").unwrap();
    let at = text.find(&want).unwrap();
    assert!(before < at && at < after, "the loop stays in its section");
    assert_eq!(fx.read(OPEN).lines().filter(|l| l.starts_with("- [ ]")).count(), 3, "nothing closed, nothing added");
    let log = fx.read(&log_rel());
    assert!(log.contains("redated") && log.contains("renew lease"), "{log}");
}

#[test]
fn redate_by_number_adds_a_nudge_where_there_was_none() {
    let fx = seeded();
    fx.ok(&["open", "redate", "2", "--nudge", "+3d"]);
    let text = fx.read(OPEN);
    assert!(text.contains(&format!("- [ ] {} · book flights · nudge {} · from: main", days_ago(5), days_ahead(3))), "{text}");
}

#[test]
fn redate_rejects_a_bad_date_and_an_unknown_loop() {
    let fx = seeded();
    let (code, _, err) = fx.run(&["open", "redate", "renew", "--nudge", "next week"]);
    assert_ne!(code, 0);
    assert!(err.contains("--nudge needs a date"), "{err}");
    let (code, _, err) = fx.run(&["open", "redate", "paint the fence", "--nudge", "+1d"]);
    assert_ne!(code, 0);
    assert!(err.contains("no open loop starts with"), "{err}");
    assert!(fx.read(OPEN).contains(&format!("renew lease · nudge {}", days_ago(3))), "untouched on refusal");
}
