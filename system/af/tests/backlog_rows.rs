//! `af backlog`: which open rows a builder can work now.

mod common;

use afcore::backlog::parse;
use common::*;

const BACKLOG: &str = "# Backlog\n\nOpen rows.\n\n## BB-2026-10-24-01 · af open has no way to re-date a loop\nSurfaced: main\nFix: a verb\nSurfaces: open.rs\nVerify: a test\n\n## BB-2026-08-28-01 · a deck proof\nSurfaced: agent\nFix: run it\nSurfaces: deck.md\nVerify: a render\nWaits: the next real deck\n\n## BB-2026-09-01-02 — an older shape title\nSurfaced: agent\nFix: x\n";

#[test]
fn parse_keeps_the_row_text_and_reads_the_waits_line() {
    let rows = parse(BACKLOG);
    assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["BB-2026-10-24-01", "BB-2026-08-28-01", "BB-2026-09-01-02"]);
    assert_eq!(rows[0].title, "af open has no way to re-date a loop");
    assert_eq!(rows[0].waits, None);
    assert!(rows[0].text.starts_with("## BB-2026-10-24-01") && rows[0].text.ends_with("Verify: a test"), "{}", rows[0].text);
    assert_eq!(rows[1].waits.as_deref(), Some("the next real deck"));
    assert_eq!(rows[2].title, "an older shape title");
}

#[test]
fn ready_leaves_out_waiting_rows_and_sorts_oldest_first() {
    let fx = Fx::new();
    fx.write("system/backlog.md", BACKLOG);
    let out = fx.ok(&["backlog", "ready"]);
    assert!(!out.contains("a deck proof"), "{out}");
    assert!(out.find("BB-2026-09-01-02").unwrap() < out.find("BB-2026-10-24-01").unwrap(), "{out}");
    assert!(out.contains("2 row(s) ready, 1 waiting"), "{out}");
}

#[test]
fn ready_json_honours_the_limit_and_carries_the_row_text() {
    let fx = Fx::new();
    fx.write("system/backlog.md", BACKLOG);
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["backlog", "ready", "--limit", "1", "--json"])).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["id"], "BB-2026-09-01-02");
    assert!(v[0]["text"].as_str().unwrap().contains("Fix: x"));
}

#[test]
fn list_names_the_wait_and_no_backlog_is_no_rows() {
    let fx = Fx::new();
    assert!(fx.ok(&["backlog", "list"]).contains("0 open row(s)"));
    fx.write("system/backlog.md", BACKLOG);
    assert!(fx.ok(&["backlog", "list"]).contains("a deck proof · waits: the next real deck"));
}
