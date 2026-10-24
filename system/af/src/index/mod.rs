//! The retrieval index behind `af index` / `af search`, ported from system/indexer.py.
//!
//! A derived cache: files stay the memory, the database (system/index/vault.db) is
//! rebuildable and shares its schema with the Python, so either side opens what the other
//! wrote. Search results are pointers; consumers open the cited files.

pub mod chunk;
pub mod embed;
pub mod eval;
pub mod pynum;
pub mod rank;
pub mod store;

use crate::error::{crash, CrashKind, AfError, R};
use crate::out;
use crate::pyos;
use crate::runlog::json_str;
use clap::Subcommand;
use embed::{embed_texts, Embedder};

pub const INDEX_DIR: [&str; 2] = ["system", "index"];
pub const DB_NAME: &str = "vault.db";
pub const GOLDEN_NAME: &str = "golden-set.yaml";
pub const CORPUS_ROOTS: [&str; 3] = ["workspace", "library/assets", ".claude/plans"];
pub const SKIP_DIRS_ANYWHERE: [&str; 12] = [
    "_archive", "_scratch", "__pycache__", "node_modules", "snapshots", "runs", "cache", "local", "dist", "out", "source",
    "references",
];

/// The indexer's own `die()`: `af index: ERROR: msg` on stderr, exit 1, and (unlike af.py's
/// `die`) no recorded refusal in the run log.
#[derive(Debug, Clone)]
pub struct IndexDie(pub String);

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
    Update {
        #[arg(long)]
        rebuild: bool,
    },
    Status,
    Eval {
        #[arg(long, default_value = "5", value_parser = py_int, allow_negative_numbers = true)]
        k: i64,
    },
}

/// argparse `type=int`: Python's `int(str)` (surrounding whitespace, a sign, single
/// underscores between digits).
pub fn py_int(s: &str) -> Result<i64, String> {
    let t = pyos::py_strip(s);
    let (neg, digits) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let valid = !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
        && digits.chars().all(|c| c.is_ascii_digit() || c == '_');
    let clean: String = digits.chars().filter(|c| *c != '_').collect();
    match (valid, clean.parse::<i64>()) {
        (true, Ok(v)) => Ok(if neg { -v } else { v }),
        _ => Err(format!("invalid int value: {}", pyos::py_repr_str(s))),
    }
}

/// `open(path, encoding="utf-8-sig", errors="replace").read()` in universal-newline mode.
pub fn read_text(path: &str) -> R<String> {
    let bytes = std::fs::read(path).map_err(|e| pyos::os_error(&e, path))?;
    let body = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes[..] };
    Ok(pyos::universal_newlines(&String::from_utf8_lossy(body)))
}

/// A SQLite failure is an uncaught `sqlite3` exception in the Python: a crash.
pub fn sql(e: rusqlite::Error) -> AfError {
    let class = match &e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => "IntegrityError",
        rusqlite::Error::SqliteFailure(..) | rusqlite::Error::SqlInputError { .. } => "OperationalError",
        _ => "DatabaseError",
    };
    crash(CrashKind::Other, format!("{class}: {e}"))
}

fn live_embedder() -> impl Fn(&[String]) -> Result<Vec<Vec<f64>>, embed::EmbedErr> {
    |texts: &[String]| embed_texts(texts)
}

fn index_die(d: IndexDie) -> i32 {
    out::err_line(&format!("af index: ERROR: {}", d.0));
    1
}

/// `af index update|status|eval`
pub fn cmd_index(root: &str, cmd: &IndexCmd) -> R<i32> {
    let e = live_embedder();
    match cmd {
        IndexCmd::Update { rebuild } => store::cmd_update(root, *rebuild, false, &e).map(|_| 0),
        IndexCmd::Status => store::cmd_status(root).map(|_| 0),
        IndexCmd::Eval { k } => {
            // refresh_before_answer: the first call in a process always refreshes
            store::cmd_update(root, false, true, &e)?;
            match eval::cmd_eval(root, *k, &e)? {
                Ok(()) => Ok(0),
                Err(d) => Ok(index_die(d)),
            }
        }
    }
}

/// `json.dumps(results, ensure_ascii=False, indent=2)`
pub fn hits_json(hits: &[rank::Hit]) -> String {
    if hits.is_empty() {
        return "[]".into();
    }
    let mut s = String::from("[\n");
    for (i, h) in hits.iter().enumerate() {
        s.push_str("  {\n");
        s.push_str(&format!("    \"path\": {},\n", json_str(&h.path)));
        s.push_str(&format!("    \"breadcrumb\": {},\n", json_str(&h.breadcrumb)));
        s.push_str(&format!("    \"snippet\": {},\n", json_str(&h.snippet)));
        s.push_str(&format!("    \"score\": {},\n", pynum::py_float_repr(h.score)));
        s.push_str("    \"why\": [\n");
        let tags: Vec<String> = h.why.iter().map(|t| format!("      {}", json_str(t))).collect();
        s.push_str(&tags.join(",\n"));
        s.push_str("\n    ]\n  }");
        if i + 1 < hits.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push(']');
    s
}

/// `cmd_search_cli(root, query, limit, as_json)` with a given embedder.
pub fn search_cli(root: &str, query: &str, limit: i64, as_json: bool, embedder: Embedder) -> R<i32> {
    let r = store::cmd_update(root, false, true, embedder)?;
    if r.added != 0 || r.updated != 0 || r.removed != 0 {
        out::line(&format!("af index: refreshed first (+{} new · ~{} updated · -{} gone)\n", r.added, r.updated, r.removed));
    }
    let results = rank::search(root, query, limit, embedder)?;
    if as_json {
        out::line(&hits_json(&results));
        return Ok(0);
    }
    let qr = pyos::py_repr_str(query);
    if results.is_empty() {
        out::line(&format!(
            "af search: no hits for {qr} — try fewer or different words, or `af index update` if files changed recently"
        ));
        return Ok(0);
    }
    out::line(&format!("af search: {} hits for {qr}  (open cited files before relying on snippets)\n", results.len()));
    for (i, hit) in results.iter().enumerate() {
        out::line(&format!("{}. [{:7.3}] {}\n   {}", i + 1, hit.score, hit.path, hit.breadcrumb));
        out::line(&format!("   …{}…", hit.snippet));
        out::line(&format!("   why: {}\n", hit.why.join(",")));
    }
    Ok(0)
}

/// `af search "<query>" [-n LIMIT] [--json]`
pub fn cmd_search(root: &str, query: &str, limit: i64, as_json: bool) -> R<i32> {
    let e = live_embedder();
    search_cli(root, query, limit, as_json, &e)
}
