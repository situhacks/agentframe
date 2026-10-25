//! The session-start index refresh.

mod common;

use afcore::hooks::index_refresh::{decide, spawn_argv, write_lock, Paths, LOCK_STALE_HOURS, REFRESH_AFTER_HOURS};
use common::Fx;
use std::time::{Duration, SystemTime};

fn now() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

struct R {
    fx: Fx,
    p: Paths,
}

impl R {
    fn new() -> R {
        let fx = Fx::new();
        fx.mkdir("system/index");
        let p = Paths::new(&fx.root());
        R { fx, p }
    }
    fn make_db(&self, hours_old: f64) {
        let stamp = (chrono::Local::now() - chrono::Duration::seconds((hours_old * 3600.0) as i64))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let con = rusqlite::Connection::open(&self.p.db).unwrap();
        con.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)", []).unwrap();
        con.execute("INSERT INTO meta VALUES ('updated_at', ?1)", [stamp]).unwrap();
    }
}

#[test]
fn test_no_index_means_no_spawn() {
    let r = R::new();
    assert!(decide(&r.p, now()).unwrap().contains("no index yet"));
}

#[test]
fn test_fresh_index_is_left_alone() {
    let r = R::new();
    r.make_db(3.0);
    assert!(decide(&r.p, now()).unwrap().starts_with("fresh"));
}

#[test]
fn test_stale_index_spawns_one_detached_update_and_locks() {
    let r = R::new();
    r.make_db(REFRESH_AFTER_HOURS + 1.0);
    assert_eq!(decide(&r.p, now()).unwrap(), "spawn");
    let argv = spawn_argv(&r.p);
    assert_eq!(argv[1..], ["index".to_string(), "update".to_string()]);
    assert!(argv[0].to_lowercase().contains("af"), "the child is this binary: {}", argv[0]);
    write_lock(&r.p).unwrap();
    assert!(r.fx.isfile("system/index/refresh.lock"));
    assert_eq!(decide(&r.p, now()).unwrap(), "a refresh is already running");
}

#[test]
fn test_a_stale_lock_does_not_block_forever() {
    let r = R::new();
    r.make_db(REFRESH_AFTER_HOURS + 1.0);
    let lock = r.fx.write("system/index/refresh.lock", "dead\n");
    let f = std::fs::File::options().write(true).open(&lock).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(((LOCK_STALE_HOURS + 1.0) * 3600.0) as u64)).unwrap();
    drop(f);
    assert_eq!(decide(&r.p, now()).unwrap(), "spawn");
}

#[test]
fn test_main_never_fails_the_session() {
    let r = R::new();
    r.fx.write("AGENTS.md", "# root\n");
    r.fx.write("system/index/vault.db", "not a database");
    let o = r.fx.af().args(["hook", "session-start"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stdout.is_empty());
}

#[test]
fn test_the_spawned_update_releases_the_lock() {
    let r = R::new();
    r.fx.write("AGENTS.md", "# root\n");
    write_lock(&r.p).unwrap();
    let o = r.fx.af().args(["index", "status"]).env(afcore::hooks::index_refresh::LOCK_ENV, &r.p.lock).output().unwrap();
    assert!(o.status.code().is_some());
    assert!(!r.fx.isfile("system/index/refresh.lock"));
    write_lock(&r.p).unwrap();
    r.fx.af().args(["doctor"]).env(afcore::hooks::index_refresh::LOCK_ENV, &r.p.lock).output().unwrap();
    assert!(r.fx.isfile("system/index/refresh.lock"), "only an index run releases it");
}
