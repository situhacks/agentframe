//! Helpers shared by the life and sessions tests: registry rows, clocks, and mtimes.
#![allow(dead_code)]

use crate::common::Fx;
use std::time::{Duration, SystemTime};

pub const HEADER: &str = "| name | role | session_id | model | started | last_seen | status | handoff |\n|---|---|---|---|---|---|---|---|\n";

/// Local ISO minutes `mins` minutes from now (negative is the past).
pub fn ts(mins: i64) -> String {
    (chrono::Local::now().naive_local() + chrono::Duration::minutes(mins)).format("%Y-%m-%dT%H:%M").to_string()
}

/// A stamp written within the last minute or so (tolerates a minute boundary mid-test).
pub fn recent(stamp: &str) -> bool {
    let t = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M").unwrap();
    (chrono::Local::now().naive_local() - t).num_seconds().abs() <= 120
}

pub fn month() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

pub fn row(name: &str, role: &str, sid: &str, last_seen: &str, status: &str) -> String {
    format!("| {name} | {role} | {sid} | opus | {last_seen} | {last_seen} | {status} | system/sessions/{name}/handoff.md |\n")
}

pub fn write_registry(fx: &Fx, rows: &[String]) {
    fx.write("system/sessions/registry.md", &format!("{HEADER}{}", rows.concat()));
}

/// Set a file's mtime `secs` seconds from now (negative is the past).
pub fn set_mtime(fx: &Fx, rel: &str, secs: i64) {
    let t = if secs >= 0 {
        SystemTime::now() + Duration::from_secs(secs as u64)
    } else {
        SystemTime::now() - Duration::from_secs((-secs) as u64)
    };
    let f = std::fs::File::options().write(true).open(fx.path(rel)).unwrap();
    f.set_modified(t).unwrap();
}

/// Set a file's mtime to a local `YYYY-MM-DDTHH:MM:SS`.
pub fn set_mtime_at(fx: &Fx, rel: &str, local: &str) {
    use chrono::TimeZone;
    let naive = chrono::NaiveDateTime::parse_from_str(local, "%Y-%m-%dT%H:%M:%S").unwrap();
    let t: SystemTime = chrono::Local.from_local_datetime(&naive).single().unwrap().into();
    std::fs::File::options().write(true).open(fx.path(rel)).unwrap().set_modified(t).unwrap();
}

pub fn handoff(written: &str, touched: &[&str]) -> String {
    format!(
        "---\nwritten: {written}\nsession_id: s1\n---\n## State\nmid-build\n\n## Open questions\nnone\n\n## Next step\ntest\n\n## Files touched\n{}\n",
        touched.iter().map(|t| format!("- {t}\n")).collect::<String>()
    )
}

pub fn registry_rows(fx: &Fx) -> Vec<afcore::sessions::Row> {
    afcore::sessions::Registry::load(&fx.ctx()).unwrap().rows()
}

pub fn find(fx: &Fx, name: &str) -> afcore::sessions::Row {
    registry_rows(fx).into_iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no row {name}"))
}
