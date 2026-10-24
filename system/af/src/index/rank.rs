//! Hybrid ranking (`search`): BM25 over trigram FTS5 plus short-token LIKE scan, cosine
//! over local embeddings, reciprocal-rank fusion, then most-specific-path-wins boosts.

use super::chunk::WS_CHARS;
use super::embed::{Embedder, EMBED_MAX_CHARS, EMBED_MODEL};
use super::pynum::{fsum, py_round, sumprod};
use super::store::connect;
use super::{read_text, sql};
use crate::error::R;
use crate::fm::rx;
use crate::out;
use crate::pyglob;
use crate::pyos;
use rusqlite::OptionalExtension;
use std::collections::{HashMap, HashSet};

pub const POOL_SIZE: i64 = 60;
pub const RRF_K: i64 = 60;
pub const LEXICAL_FUSION_WEIGHT: f64 = 1.0;
pub const SEMANTIC_FUSION_WEIGHT: f64 = 1.0;

#[derive(Debug, Clone)]
pub struct Hit {
    pub path: String,
    pub breadcrumb: String,
    pub snippet: String,
    pub score: f64,
    pub why: Vec<&'static str>,
}

/// `path_boost(rel)`: most-specific-path-wins class boosts -> (multiplier, tag).
pub fn path_boost(rel: &str) -> (f64, &'static str) {
    let p = rel.replace('\\', "/").to_lowercase();
    if p.contains("_archive/") {
        return (0.45, "archive");
    }
    if p.starts_with("workspace/life/") && !p.contains("/_history/") && !p.contains("/sources/") {
        return (1.5, "life");
    }
    if p.contains("/knowledge/") || p.starts_with("knowledge/") {
        return (1.5, "knowledge");
    }
    if p.contains("/sources/") || p.contains("/_local/") {
        return (1.15, "primary-material");
    }
    if p.starts_with(".claude/plans/") {
        return (1.05, "plan");
    }
    if p.starts_with("agents") {
        return (1.15, "router");
    }
    (1.0, "")
}

/// Resolve a path a tracker names: as written, else relative to the tracker's folder.
fn resolve_named(root: &str, proj_dir: &str, cand: &str) -> R<Option<String>> {
    let cand = cand.replace('\\', "/");
    let second = pyos::relpath(&pyos::normpath(&pyos::join(proj_dir, &cand)), root)?;
    for resolved in [cand.clone(), second] {
        let resolved = resolved.replace('\\', "/");
        if pyos::isfile(&pyos::join(root, &resolved)) {
            return Ok(Some(resolved));
        }
    }
    Ok(None)
}

/// `tracker_pointers(root)`: deliverable paths named by project/application trackers.
pub fn tracker_pointers(root: &str) -> R<HashSet<String>> {
    let mut pointers = HashSet::new();
    let re = rx(&format!(r#"file:[{WS_CHARS}]*["'`]?([^{WS_CHARS}"'`]+\.(?:md|txt))"#));
    let patterns = [
        pyos::join_all(&["workspace", "projects", "*", "project.md"]),
        pyos::join_all(&["workspace", "pipeline", "applications", "*", "application.md"]),
    ];
    for pattern in patterns {
        for full in pyglob::glob(&pyos::join(root, &pattern), false) {
            let proj_dir = pyos::dirname(&full);
            let text = match read_text(&full) {
                Ok(t) => t,
                Err(e) if e.is_decode_or_os() => continue,
                Err(e) => return Err(e),
            };
            for c in re.captures_iter(&text) {
                if let Some(r) = resolve_named(root, &proj_dir, &c[1])? {
                    pointers.insert(r);
                }
            }
        }
    }
    Ok(pointers)
}

/// `registered_sources(root)`: paths listed in any project's sources/INDEX.md.
pub fn registered_sources(root: &str) -> R<HashSet<String>> {
    let mut reg = HashSet::new();
    let re = rx(&format!(r"[^{WS_CHARS}()]*sources/[^{WS_CHARS}()]*\.(?:md|txt)"));
    let mut indexes = Vec::new();
    pyos::walk(&pyos::join(root, "workspace"), &mut |dirpath: &str, _d: &mut Vec<String>, _f: &[String]| {
        if pyos::basename(dirpath) != "sources" {
            return;
        }
        let idx = pyos::join(dirpath, "INDEX.md");
        if pyos::isfile(&idx) {
            indexes.push((idx, pyos::dirname(dirpath)));
        }
    });
    for (idx, proj_dir) in indexes {
        let text = read_text(&idx)?;
        for m in re.find_iter(&text) {
            if let Some(r) = resolve_named(root, &proj_dir, m.as_str())? {
                reg.insert(r);
            }
        }
    }
    Ok(reg)
}

/// `_tokenize(query)`
pub fn tokenize(query: &str) -> Vec<String> {
    rx(r"[a-z0-9_']+").find_iter(&query.to_lowercase()).map(|m| m.as_str().to_string()).collect()
}

/// `_snippet(body, tokens)`: 180 characters around the first token hit, whitespace folded.
pub fn snippet(body: &str, tokens: &[String]) -> String {
    let width = 90usize;
    let low = body.to_lowercase();
    let mut pos: Option<usize> = None;
    for t in tokens {
        if t.chars().count() < 3 {
            continue;
        }
        if let Some(b) = low.find(t.as_str()) {
            let ci = low[..b].chars().count();
            pos = Some(pos.map_or(ci, |p: usize| p.min(ci)));
        }
    }
    let pos = pos.unwrap_or(0);
    let start = pos.saturating_sub(width / 3);
    let frag: String = body.chars().skip(start).take(width * 2).collect();
    let frag = pyos::py_strip(&frag);
    rx(&format!("[{WS_CHARS}]+")).replace_all(frag, " ").into_owned()
}

/// `seq[:n]` with Python's negative-index semantics.
pub fn py_take<T>(mut v: Vec<T>, n: i64) -> Vec<T> {
    let len = v.len() as i64;
    let end = if n < 0 { (len + n).max(0) } else { n.min(len) };
    v.truncate(end as usize);
    v
}

/// The query text sent to the model (Qwen3's documented query-side instruction).
pub fn query_text(query: &str) -> String {
    let q = pyos::py_prefix(query, EMBED_MAX_CHARS);
    if EMBED_MODEL.contains("qwen3") {
        format!("Instruct: Retrieve the user's own notes and documents relevant to their request\nQuery: {q}")
    } else {
        q
    }
}

/// Semantic retriever: chunk paths by cosine, best first, top POOL_SIZE (a path may repeat).
fn semantic(con: &rusqlite::Connection, query: &str, embedder: Embedder) -> Result<Vec<String>, String> {
    let qvec = embedder(&[query_text(query)]).map_err(|e| e.class)?;
    let qarr: Vec<f32> = qvec.first().ok_or("IndexError")?.iter().map(|x| *x as f32).collect();
    let qnorm = fsum(qarr.iter().map(|x| (*x as f64) * (*x as f64))).sqrt();
    let mut st = con.prepare("SELECT path, vec, norm FROM embeddings").map_err(|_| "OperationalError".to_string())?;
    let mut rows = st.query([]).map_err(|_| "OperationalError".to_string())?;
    let mut scored: Vec<(f64, String)> = Vec::new();
    let mut carr: Vec<f32> = Vec::new();
    while let Some(r) = rows.next().map_err(|_| "OperationalError".to_string())? {
        let path: String = r.get(0).map_err(|_| "TypeError".to_string())?;
        let blob = r.get_ref(1).map_err(|_| "TypeError".to_string())?;
        let blob = blob.as_blob().map_err(|_| "TypeError".to_string())?;
        let norm: f64 = r.get(2).map_err(|_| "TypeError".to_string())?;
        if blob.len() % 4 != 0 {
            return Err("ValueError".into());
        }
        carr.clear();
        carr.extend(blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        let denom = qnorm * norm;
        let cos = if denom != 0.0 { sumprod(&qarr, &carr).ok_or("ValueError")? / denom } else { 0.0 };
        scored.push((cos, path));
    }
    // sort(reverse=True) on (cos, path) tuples
    scored.sort_by(|a, b| {
        let c = if a.0 == b.0 { a.1.cmp(&b.1) } else { a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal) };
        c.reverse()
    });
    scored.truncate(POOL_SIZE as usize);
    Ok(scored.into_iter().map(|(_, p)| p).collect())
}

/// `search(root, query, limit)`: hybrid ranked hits.
pub fn search(root: &str, query: &str, limit: i64, embedder: Embedder) -> R<Vec<Hit>> {
    let con = connect(root)?;
    let tokens = tokenize(query);
    let good: Vec<String> = tokens.iter().filter(|t| t.chars().count() >= 3).map(|t| format!("\"{}\"", t.replace('"', "\"\""))).collect();
    let shorts: Vec<&String> = tokens.iter().filter(|t| (1..3).contains(&t.chars().count())).collect();

    // --- lexical retriever: best chunk per path, smaller bm25 = better ---
    let mut best: HashMap<String, (f64, String, String)> = HashMap::new();
    let mut first_seen: Vec<String> = Vec::new();
    if !good.is_empty() {
        let mut st = con
            .prepare("SELECT path, breadcrumb, body, bm25(chunks_fts) FROM chunks_fts WHERE chunks_fts MATCH ? ORDER BY bm25(chunks_fts) LIMIT ?")
            .map_err(sql)?;
        let rows = st
            .query_map(rusqlite::params![good.join(" OR "), POOL_SIZE], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, f64>(3)?))
            })
            .map_err(sql)?;
        for row in rows {
            let (path, crumb, body, r) = row.map_err(sql)?;
            match best.get(&path) {
                None => {
                    first_seen.push(path.clone());
                    best.insert(path, (r, crumb, body));
                }
                Some(old) if r < old.0 => {
                    best.insert(path, (r, crumb, body));
                }
                _ => {}
            }
        }
    }
    let mut lexical_order = first_seen;
    lexical_order.sort_by(|a, b| best[a].0.partial_cmp(&best[b].0).unwrap_or(std::cmp::Ordering::Equal));
    for short in &shorts {
        let mut st = con
            .prepare_cached("SELECT path, breadcrumb, body FROM chunks WHERE instr(lower(body), ?) > 0 LIMIT 25")
            .map_err(sql)?;
        let rows = st
            .query_map([short.as_str()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .map_err(sql)?;
        for row in rows {
            let (path, crumb, body) = row.map_err(sql)?;
            if !best.contains_key(&path) {
                best.insert(path.clone(), (0.0, crumb, body));
                lexical_order.push(path);
            }
        }
    }

    // --- semantic retriever (degrades to keyword-only without Ollama) ---
    let mut semantic_order: Vec<String> = Vec::new();
    let emb_count: i64 = con.query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0)).map_err(sql)?;
    if emb_count != 0 && tokens.iter().any(|t| t.chars().count() >= 3) {
        match semantic(&con, query, embedder) {
            Ok(order) => semantic_order = order,
            Err(class) => out::err_line(&format!("af search: note — semantic scoring unavailable ({class}); keyword-only this run")),
        }
    }
    let semantic_set: HashSet<&String> = semantic_order.iter().collect();

    // --- fuse, then boost ---
    let mut fused: Vec<(String, f64)> = Vec::new();
    let mut slot: HashMap<String, usize> = HashMap::new();
    let mut add = |path: &String, w: f64| match slot.get(path) {
        Some(&i) => fused[i].1 += w,
        None => {
            slot.insert(path.clone(), fused.len());
            fused.push((path.clone(), 0.0 + w));
        }
    };
    for (rank, path) in lexical_order.iter().enumerate() {
        add(path, LEXICAL_FUSION_WEIGHT / ((RRF_K + rank as i64 + 1) as f64));
    }
    for (rank, path) in semantic_order.iter().enumerate() {
        add(path, SEMANTIC_FUSION_WEIGHT / ((RRF_K + rank as i64 + 1) as f64));
    }

    let trackers = tracker_pointers(root)?;
    let registered = registered_sources(root)?;

    let mut results = Vec::with_capacity(fused.len());
    let mut first_chunk = con.prepare("SELECT breadcrumb, body FROM chunks WHERE path=? ORDER BY ord LIMIT 1").map_err(sql)?;
    let mut head_q = con.prepare("SELECT is_head FROM files WHERE path=?").map_err(sql)?;
    for (path, base) in &fused {
        let (crumb, body) = match best.get(path) {
            Some((_, c, b)) => (c.clone(), b.clone()),
            None => first_chunk
                .query_row([path], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .optional()
                .map_err(sql)?
                .unwrap_or_default(),
        };
        let is_head: Option<Option<i64>> = head_q.query_row([path], |r| r.get(0)).optional().map_err(sql)?;
        let mut mult = 1.0f64;
        let mut why: Vec<&'static str> = Vec::new();
        let (cls, tag) = path_boost(path);
        if cls != 1.0 {
            mult *= cls;
            why.push(tag);
        }
        // `if row and not row[0]`: a NULL or 0 is_head ranks down
        if let Some(h) = is_head {
            if h.unwrap_or(0) == 0 {
                mult *= 0.55;
                why.push("superseded-version");
            }
        }
        if trackers.contains(path) {
            mult *= 1.9;
            why.push("tracker-head");
        }
        if registered.contains(path) {
            mult *= 1.25;
            why.push("registered-source");
        }
        if semantic_set.contains(path) {
            why.push("semantic");
        }
        if best.contains_key(path) {
            why.push("bm25");
        }
        if why.is_empty() {
            why.push("fused");
        }
        results.push(Hit { path: path.clone(), breadcrumb: crumb, snippet: snippet(&body, &tokens), score: py_round(base * mult, 6), why });
    }
    drop(first_chunk);
    drop(head_q);
    drop(con);
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(py_take(results, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boosts_follow_most_specific_path() {
        assert_eq!(path_boost("workspace/projects/x/_archive/knowledge/a.md"), (0.45, "archive"));
        assert_eq!(path_boost("workspace/life/career/a.md"), (1.5, "life"));
        assert_eq!(path_boost("workspace/life/x/sources/a.md"), (1.15, "primary-material"));
        assert_eq!(path_boost("workspace/life/_history/a.md"), (1.0, ""));
        assert_eq!(path_boost("workspace/projects/p/knowledge/a.md"), (1.5, "knowledge"));
        assert_eq!(path_boost(".claude/plans/p.md"), (1.05, "plan"));
        assert_eq!(path_boost("workspace/projects/p/brief.md"), (1.0, ""));
    }

    #[test]
    fn tokens_and_snippets() {
        assert_eq!(tokenize("Who's the FDE lead, 2026?"), vec!["who's", "the", "fde", "lead", "2026"]);
        let body = format!("{}needle here{}", "x ".repeat(100), " y".repeat(100));
        let s = snippet(&body, &["needle".to_string()]);
        assert!(s.starts_with("x x"));
        assert!(s.contains("needle here"));
        assert_eq!(snippet("  a\n\n b\tc  ", &[]), "a b c");
    }

    #[test]
    fn negative_limits_slice_like_python() {
        assert_eq!(py_take(vec![1, 2, 3], -1), vec![1, 2]);
        assert_eq!(py_take(vec![1, 2, 3], -5), Vec::<i32>::new());
        assert_eq!(py_take(vec![1, 2, 3], 8), vec![1, 2, 3]);
        assert_eq!(py_take(vec![1, 2, 3], 0), Vec::<i32>::new());
    }
}
