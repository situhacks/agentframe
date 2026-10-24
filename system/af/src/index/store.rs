//! The database and the corpus walk: `connect`, `iter_corpus`, `cmd_update`, `cmd_status`.

use super::chunk::{breadcrumb, chunk_markdown, file_heads, split_frontmatter, version_of};
use super::embed::{EmbedErr, Embedder, EMBED_BATCH, EMBED_MAX_CHARS, EMBED_MODEL};
use super::pynum::{fsum, st_mtime};
use super::{read_text, sql, CORPUS_ROOTS, DB_NAME, INDEX_DIR, SKIP_DIRS_ANYWHERE};
use crate::error::R;
use crate::out;
use crate::pyos;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files(
  path TEXT PRIMARY KEY, mtime REAL NOT NULL, size INTEGER NOT NULL,
  hash TEXT NOT NULL, status TEXT, version INTEGER, is_head INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS chunks(
  chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL, ord INTEGER NOT NULL,
  breadcrumb TEXT NOT NULL, body TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS chunks_path ON chunks(path);
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  body, breadcrumb, path UNINDEXED, tokenize='trigram');
CREATE TABLE IF NOT EXISTS embeddings(
  path TEXT NOT NULL, ord INTEGER NOT NULL,
  dim INTEGER NOT NULL, norm REAL NOT NULL,
  vec BLOB NOT NULL, PRIMARY KEY(path, ord));
";

pub fn index_dir(root: &str) -> String {
    pyos::join_all(&[root, INDEX_DIR[0], INDEX_DIR[1]])
}

pub fn db_path(root: &str) -> String {
    pyos::join(&index_dir(root), DB_NAME)
}

/// `connect(root)`: create the folder, open, WAL, schema.
pub fn connect(root: &str) -> R<Connection> {
    pyos::makedirs(&index_dir(root))?;
    let con = Connection::open(db_path(root)).map_err(sql)?;
    let _: String = con.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0)).map_err(sql)?;
    con.execute_batch(SCHEMA_SQL).map_err(sql)?;
    Ok(con)
}

/// `iter_corpus(root)`: repo-relative forward-slash paths inside the corpus.
pub fn iter_corpus(root: &str) -> R<Vec<String>> {
    let mut out = Vec::new();
    let mut err = None;
    for top in CORPUS_ROOTS {
        let base = pyos::join_all(&[root, &top.replace('/', &pyos::SEP.to_string())]);
        if !pyos::isdir(&base) {
            continue;
        }
        pyos::walk(&base, &mut |dirpath: &str, dirnames: &mut Vec<String>, filenames: &[String]| {
            if err.is_some() {
                dirnames.clear();
                return;
            }
            let rel_dir = match pyos::relpath(dirpath, root) {
                Ok(r) => r.replace('\\', "/"),
                Err(e) => {
                    err = Some(e);
                    dirnames.clear();
                    return;
                }
            };
            dirnames.retain(|d| !SKIP_DIRS_ANYWHERE.contains(&d.as_str()) && !(d == "voice" && rel_dir == "workspace/life"));
            let mut names: Vec<&String> = filenames.iter().collect();
            names.sort();
            for name in names {
                let ext = pyos::splitext(name).1.to_lowercase();
                if ext != ".md" && ext != ".txt" {
                    continue;
                }
                out.push(pyos::join(&rel_dir, name).replace('\\', "/"));
            }
        });
    }
    match err {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

fn hash_of(root: &str, rel: &str) -> R<String> {
    let full = pyos::join(root, rel);
    let bytes = std::fs::read(&full).map_err(|e| pyos::os_error(&e, &full))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn drop_file_rows(con: &Connection, rel: &str) -> R<()> {
    for table in ["chunks_fts", "chunks", "embeddings", "files"] {
        con.execute(&format!("DELETE FROM {table} WHERE path = ?"), [rel]).map_err(sql)?;
    }
    Ok(())
}

type Pending = (String, i64, String);

/// `insert_file(...)`: index one file's chunks; returns the (path, ord, embed text) queue.
fn insert_file(con: &Connection, root: &str, rel: &str, mtime: f64, size: u64, hash: &str, heads: &HashSet<String>) -> R<Vec<Pending>> {
    let text = read_text(&pyos::join(root, rel))?;
    let (fm, body) = split_frontmatter(&text);
    let version = version_of(rel);
    drop_file_rows(con, rel)?;
    let mut items = Vec::new();
    let mut ins_chunk = con.prepare_cached("INSERT INTO chunks(path, ord, breadcrumb, body) VALUES(?,?,?,?)").map_err(sql)?;
    let mut ins_fts = con.prepare_cached("INSERT INTO chunks_fts(body, breadcrumb, path) VALUES(?,?,?)").map_err(sql)?;
    for (i, (crumbs, chunk)) in chunk_markdown(&body).into_iter().enumerate() {
        let crumb = breadcrumb(rel, &crumbs);
        ins_chunk.execute(params![rel, i as i64, crumb, chunk]).map_err(sql)?;
        ins_fts.execute(params![chunk, crumb, rel]).map_err(sql)?;
        items.push((rel.to_string(), i as i64, embed_text(&crumb, &chunk)));
    }
    con.execute(
        "INSERT INTO files(path, mtime, size, hash, status, version, is_head) VALUES(?,?,?,?,?,?,?)",
        params![rel, mtime, size as i64, hash, fm.get("status"), version, heads.contains(rel) as i64],
    )
    .map_err(sql)?;
    Ok(items)
}

/// `f"{crumb}\n{text}"[:EMBED_MAX_CHARS]`
fn embed_text(crumb: &str, text: &str) -> String {
    pyos::py_prefix(&format!("{crumb}\n{text}"), EMBED_MAX_CHARS)
}

pub fn count_unembedded(con: &Connection) -> R<i64> {
    con.query_row(
        "SELECT COUNT(*) FROM chunks c LEFT JOIN embeddings e ON e.path = c.path AND e.ord = c.ord WHERE e.path IS NULL",
        [],
        |r| r.get(0),
    )
    .map_err(sql)
}

fn count(con: &Connection, q: &str) -> R<i64> {
    con.query_row(q, [], |r| r.get(0)).map_err(sql)
}

/// The batch queue `cmd_update` feeds: flushes every EMBED_BATCH items and, on the first
/// backend failure, warns once and goes keyword-only for the rest of the run.
struct EmbedQueue<'a> {
    pending: Vec<Pending>,
    failed: bool,
    embedder: Embedder<'a>,
}

impl EmbedQueue<'_> {
    fn flush(&mut self, con: &Connection) {
        if self.pending.is_empty() || self.failed {
            self.pending.clear();
            return;
        }
        let n = self.pending.len().min(EMBED_BATCH);
        match self.store_batch(con, n) {
            Ok(()) => {
                self.pending.drain(..n);
            }
            Err(e) => {
                self.failed = true;
                self.pending.clear();
                out::err_line(&format!(
                    "af index: WARNING — embeddings unavailable ({}: {}); indexed keyword-only. Start Ollama and re-run `af index update` to fill vectors.",
                    e.class, e.msg
                ));
            }
        }
    }

    fn store_batch(&self, con: &Connection, n: usize) -> Result<(), EmbedErr> {
        let batch = &self.pending[..n];
        let texts: Vec<String> = batch.iter().map(|(_, _, t)| t.clone()).collect();
        let vecs = (self.embedder)(&texts)?;
        let dim = vecs[0].len() as i64;
        let dberr = |e: rusqlite::Error| EmbedErr::new("OperationalError", e.to_string());
        let row: Option<i64> = con.query_row("SELECT dim FROM embeddings LIMIT 1", [], |r| r.get(0)).optional().map_err(dberr)?;
        if let Some(old) = row {
            if old != dim {
                return Err(EmbedErr::new(
                    "RuntimeError",
                    format!("embedding dimension changed ({old} -> {dim}); run `af index update --rebuild` after switching models"),
                ));
            }
        }
        let mut ins = con
            .prepare_cached("INSERT OR REPLACE INTO embeddings(path, ord, dim, norm, vec) VALUES(?,?,?,?,?)")
            .map_err(dberr)?;
        for ((pth, ordn, _), vec) in batch.iter().zip(vecs.iter()) {
            let norm = fsum(vec.iter().map(|x| x * x)).sqrt();
            let mut packed = Vec::with_capacity(vec.len() * 4);
            for x in vec {
                packed.extend_from_slice(&(*x as f32).to_le_bytes());
            }
            ins.execute(params![pth, ordn, dim, norm, packed]).map_err(dberr)?;
        }
        Ok(())
    }

    fn queue(&mut self, con: &Connection, items: Vec<Pending>) {
        self.pending.extend(items);
        while self.pending.len() >= EMBED_BATCH {
            self.flush(con);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub files: i64,
    pub chunks: i64,
    pub embedded: i64,
    pub backfilled: i64,
}

/// `cmd_update(root, rebuild, quiet)`
pub fn cmd_update(root: &str, rebuild: bool, quiet: bool, embedder: Embedder) -> R<Summary> {
    let started = std::time::Instant::now();
    let con = connect(root)?;
    if rebuild {
        for table in ["chunks_fts", "chunks", "files", "embeddings"] {
            con.execute_batch(&format!("DROP TABLE IF EXISTS {table}")).map_err(sql)?;
        }
        con.execute_batch(SCHEMA_SQL).map_err(sql)?;
    }

    let mut disk: BTreeMap<String, (f64, u64)> = BTreeMap::new();
    let (mut n_md, mut n_txt) = (0usize, 0usize);
    for rel in iter_corpus(root)? {
        let full = pyos::join(root, &rel);
        let md = std::fs::metadata(&full).map_err(|e| pyos::os_error(&e, &full))?;
        if rel.to_lowercase().ends_with(".md") {
            n_md += 1;
        } else {
            n_txt += 1;
        }
        disk.insert(rel, (st_mtime(&md), md.len()));
    }
    let heads = file_heads(disk.keys());
    let mut known: BTreeMap<String, (f64, i64, String)> = BTreeMap::new();
    {
        let mut st = con.prepare("SELECT path, mtime, size, hash FROM files").map_err(sql)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, f64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?))))
            .map_err(sql)?;
        for row in rows {
            let (k, v) = row.map_err(sql)?;
            known.insert(k, v);
        }
    }

    // Python's sqlite3 opens a transaction at the first write and commits once at the end.
    con.execute_batch("BEGIN").map_err(sql)?;
    let (mut added, mut updated, mut unchanged, mut removed, mut rehashed_same) = (0, 0, 0, 0, 0);
    let mut q = EmbedQueue { pending: Vec::new(), failed: false, embedder };

    let disk_keys: BTreeSet<&String> = disk.keys().collect();
    let known_keys: BTreeSet<&String> = known.keys().collect();
    for rel in disk_keys.difference(&known_keys) {
        let (mtime, size) = disk[*rel];
        let h = hash_of(root, rel)?;
        let items = insert_file(&con, root, rel, mtime, size, &h, &heads)?;
        q.queue(&con, items);
        added += 1;
    }
    for rel in disk_keys.intersection(&known_keys) {
        let (mtime, size) = disk[*rel];
        let (old_mtime, old_size, old_hash) = &known[*rel];
        if *old_mtime == mtime && *old_size == size as i64 {
            unchanged += 1;
            continue;
        }
        let new_hash = hash_of(root, rel)?;
        if &new_hash == old_hash {
            con.execute("UPDATE files SET mtime=? WHERE path=?", params![mtime, rel]).map_err(sql)?;
            rehashed_same += 1;
            continue;
        }
        let items = insert_file(&con, root, rel, mtime, size, &new_hash, &heads)?;
        q.queue(&con, items);
        updated += 1;
    }
    for rel in known_keys.difference(&disk_keys) {
        drop_file_rows(&con, rel)?;
        removed += 1;
    }
    q.flush(&con);

    // Chunks a failed batch left without vectors: backfill so a refresh self-heals.
    let mut backfilled = 0i64;
    if !q.failed {
        let mut orphans: Vec<Pending> = Vec::new();
        {
            let mut st = con
                .prepare(
                    "SELECT c.path, c.ord, c.breadcrumb, c.body FROM chunks c LEFT JOIN embeddings e \
                     ON e.path = c.path AND e.ord = c.ord WHERE e.path IS NULL ORDER BY c.path, c.ord",
                )
                .map_err(sql)?;
            let rows = st
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
                })
                .map_err(sql)?;
            for row in rows {
                let (p, o, c, b) = row.map_err(sql)?;
                orphans.push((p, o, embed_text(&c, &b)));
            }
        }
        if !orphans.is_empty() {
            let n = orphans.len() as i64;
            q.queue(&con, orphans);
            q.flush(&con);
            backfilled = n - count_unembedded(&con)?;
        }
    }

    let total_chunks = count(&con, "SELECT COUNT(*) FROM chunks")?;
    let total_files = count(&con, "SELECT COUNT(*) FROM files")?;
    let embedded_total = count(&con, "SELECT COUNT(*) FROM embeddings")?;
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    con.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('updated_at', ?)", [&now]).map_err(sql)?;
    con.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('embed_model', ?)", [EMBED_MODEL]).map_err(sql)?;
    con.execute_batch("COMMIT").map_err(sql)?;
    drop(con);
    let summary = Summary {
        added,
        updated,
        removed,
        files: total_files,
        chunks: total_chunks,
        embedded: embedded_total,
        backfilled,
    };
    if !quiet {
        let back = if backfilled != 0 { format!("+{backfilled} backfilled · ") } else { String::new() };
        out::line(&format!(
            "af index: scanned {} files ({n_md} md, {n_txt} txt) · +{added} new · ~{updated} updated · ={unchanged} unchanged \
             ({rehashed_same} touch-only) · -{removed} gone · {total_files} tracked / {total_chunks} chunks · \
             E{embedded_total}/{total_chunks} embedded · {back}{:.1}s",
            disk.len(),
            started.elapsed().as_secs_f64()
        ));
    }
    Ok(summary)
}

/// `cmd_status(root)`
pub fn cmd_status(root: &str) -> R<()> {
    let con = connect(root)?;
    let files_n = count(&con, "SELECT COUNT(*) FROM files")?;
    let chunks_n = count(&con, "SELECT COUNT(*) FROM chunks")?;
    let updated_at: Option<String> =
        con.query_row("SELECT value FROM meta WHERE key='updated_at'", [], |r| r.get(0)).optional().map_err(sql)?;
    let mut stale = 0;
    {
        let mut st = con.prepare("SELECT path, mtime, size FROM files").map_err(sql)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, i64>(2)?)))
            .map_err(sql)?;
        for row in rows {
            let (rel, mtime, size) = row.map_err(sql)?;
            let full = pyos::join(root, &rel);
            if !pyos::isfile(&full) {
                stale += 1;
                continue;
            }
            let md = std::fs::metadata(&full).map_err(|e| pyos::os_error(&e, &full))?;
            if (mtime - st_mtime(&md)).abs() > 1e-6 || size != md.len() as i64 {
                stale += 1;
            }
        }
    }
    let superseded = count(&con, "SELECT COUNT(*) FROM files WHERE is_head = 0")?;
    let embedded = count(&con, "SELECT COUNT(*) FROM embeddings")?;
    let unembedded = count_unembedded(&con)?;
    let model: Option<String> =
        con.query_row("SELECT value FROM meta WHERE key='embed_model'", [], |r| r.get(0)).optional().map_err(sql)?;
    drop(con);
    out::line(&format!("af index: db {}", pyos::relpath(&db_path(root), root)?));
    out::line(&format!("  last update : {}", updated_at.as_deref().unwrap_or("never")));
    out::line(&format!("  tracked     : {files_n} files / {chunks_n} chunks"));
    let note = if embedded == 0 {
        " — keyword-only until you run `af index update`".to_string()
    } else if unembedded != 0 {
        format!(" — WARNING: {unembedded} chunks have no vector and are keyword-only; run `af index update` to backfill")
    } else {
        String::new()
    };
    out::line(&format!("  embeddings  : {embedded}/{chunks_n} chunks via {}{note}", model.as_deref().unwrap_or("n/a")));
    out::line(&format!("  stale       : {stale} (run `af index update`)"));
    out::line(&format!("  superseded  : {superseded} prior versions ranked down"));
    Ok(())
}
