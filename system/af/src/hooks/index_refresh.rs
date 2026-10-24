//! At session start, when the retrieval index is more than a day old, spawn one detached
//! `af index update` and say so in one line.

use super::pathlib::pjoin;
use crate::out;
use chrono::{Local, NaiveDateTime, TimeZone};
use std::path::Path;

pub const REFRESH_AFTER_HOURS: f64 = 24.0;
pub const LOCK_STALE_HOURS: f64 = 2.0;

/// The spawned `af index update` removes the lock this names when it ends, so a finished
/// refresh never blocks the next one; a killed child leaves a lock that goes stale on its own.
pub const LOCK_ENV: &str = "AF_REFRESH_LOCK";

pub const SPAWNED_LINE: &str = "AgentFrame index: last refresh over a day old; refreshing in the background \
(af index status).";

pub struct Paths {
    pub root: String,
    pub db: String,
    pub lock: String,
    pub log: String,
}

impl Paths {
    pub fn new(root: &str) -> Paths {
        Paths {
            root: root.to_string(),
            db: pjoin(root, "system/index/vault.db"),
            lock: pjoin(root, "system/index/refresh.lock"),
            log: pjoin(root, "system/index/refresh.log"),
        }
    }
}

/// `datetime.fromisoformat(s)` for the shapes an ISO timestamp takes: (local naive time,
/// whether it carried an offset). Aware values are converted to local time, as the
/// Python's `astimezone().replace(tzinfo=None)` does.
pub fn fromisoformat_local(s: &str) -> Option<NaiveDateTime> {
    let re = crate::fm::rx(
        r"\A(\d{4})-?(\d{2})-?(\d{2})(?:.(\d{2})(?::?(\d{2})(?::?(\d{2})(?:[.,](\d{1,6})\d*)?)?)?(?:(Z)|([+-])(\d{2}):?(\d{2})(?::?(\d{2})(?:[.,]\d{1,6})?)?)?)?\z",
    );
    let c = re.captures(s)?;
    let num = |i: usize| c.get(i).map(|m| m.as_str().parse::<u32>().unwrap_or(0)).unwrap_or(0);
    let date = chrono::NaiveDate::from_ymd_opt(num(1) as i32, num(2), num(3))?;
    let micros = c.get(7).map(|m| format!("{:0<6}", m.as_str()).parse::<u32>().unwrap_or(0)).unwrap_or(0);
    let time = chrono::NaiveTime::from_hms_micro_opt(num(4), num(5), num(6), micros)?;
    let naive = date.and_time(time);
    if c.get(8).is_none() && c.get(9).is_none() {
        return Some(naive);
    }
    let secs = if c.get(8).is_some() {
        0
    } else {
        let sign = if &c[9] == "-" { -1 } else { 1 };
        sign * (num(10) as i32 * 3600 + num(11) as i32 * 60 + num(12) as i32)
    };
    let off = chrono::FixedOffset::east_opt(secs)?;
    let aware = off.from_local_datetime(&naive).single()?;
    Some(aware.with_timezone(&Local).naive_local())
}

/// `last_update(db)`: Err for a database error (the Python lets it escape), Ok(None) for
/// a missing row or an unparsable value.
pub fn last_update(db: &str) -> Result<Option<NaiveDateTime>, String> {
    use rusqlite::{types::ValueRef, Connection, OpenFlags};
    let con = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| e.to_string())?;
    let mut stmt = con.prepare("SELECT value FROM meta WHERE key='updated_at'").map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let Some(row) = rows.next().map_err(|e| e.to_string())? else { return Ok(None) };
    // str(row[0])
    let text = match row.get_ref(0).map_err(|e| e.to_string())? {
        ValueRef::Null => "None".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => crate::pyjson::float_repr(f),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(b) => format!("b{}", crate::pyos::py_repr_str(&String::from_utf8_lossy(b))),
    };
    Ok(fromisoformat_local(&text))
}

fn mtime_secs(p: &str) -> Option<f64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    })
}

/// "spawn" when a refresh is due, otherwise the reason it is not. Err is an exception the
/// Python's `main()` swallows.
pub fn decide(p: &Paths, now: NaiveDateTime) -> Result<String, String> {
    if !Path::new(&p.db).is_file() {
        return Ok("no index yet (af index update --rebuild builds the first one)".into());
    }
    if Path::new(&p.lock).is_file() {
        let now_ts = Local.from_local_datetime(&now).earliest().map(|t| t.timestamp_micros() as f64 / 1e6).unwrap_or(0.0);
        let mtime = mtime_secs(&p.lock).ok_or("lock stat failed")?;
        if (now_ts - mtime) / 3600.0 < LOCK_STALE_HOURS {
            return Ok("a refresh is already running".into());
        }
    }
    if let Some(updated) = last_update(&p.db)? {
        let age_h = (now - updated).num_microseconds().map(|m| m as f64 / 1e6).unwrap_or(f64::MAX) / 3600.0;
        if age_h < REFRESH_AFTER_HOURS {
            return Ok(format!("fresh ({}h old)", py_round0(age_h)));
        }
    }
    Ok("spawn".into())
}

/// `f"{x:.0f}"` (round half to even on the exact value).
fn py_round0(x: f64) -> String {
    format!("{x:.0}")
}

/// The lock the refresh holds: `{pid} {now}` and a newline, as `Path.write_text` writes it.
pub fn write_lock(p: &Paths) -> Result<(), String> {
    std::fs::create_dir_all(crate::pyos::dirname(&p.lock)).map_err(|e| e.to_string())?;
    let stamp = Local::now().format("%Y-%m-%dT%H:%M:%S");
    std::fs::write(&p.lock, format!("{} {}{}", std::process::id(), stamp, out::NL)).map_err(|e| e.to_string())
}

/// The detached child's argv: this binary, `index update`.
pub fn spawn_argv(_p: &Paths) -> Vec<String> {
    let exe = std::env::current_exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|_| "af".into());
    vec![exe, "index".into(), "update".into()]
}

/// After any `af index ...` run: remove the lock a session-start refresh handed it.
pub fn release_lock(argv: &[String]) {
    if argv.first().map(String::as_str) != Some("index") {
        return;
    }
    if let Ok(lock) = std::env::var(LOCK_ENV) {
        if !lock.is_empty() && Path::new(&lock).is_file() {
            let _ = std::fs::remove_file(&lock);
        }
    }
}

/// Write the lock and start the update detached, stdout and stderr appended to refresh.log.
pub fn spawn(p: &Paths) -> Result<(), String> {
    write_lock(p)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(&p.log).map_err(|e| e.to_string())?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    let argv = spawn_argv(p);
    let mut cmd = std::process::Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&p.root)
        .env("AF_ROOT", &p.root)
        .env(LOCK_ENV, &p.lock)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log2);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// The session-start refresh: the one line to show when an update was spawned. Never
/// fails the session.
pub fn refresh(root: &str) -> Option<&'static str> {
    let p = Paths::new(root);
    match decide(&p, Local::now().naive_local()) {
        Ok(d) if d == "spawn" && spawn(&p).is_ok() => Some(SPAWNED_LINE),
        _ => None,
    }
}
