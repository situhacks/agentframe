//! `af index eval`: recall@k and MRR over the instance's golden set.

use super::embed::Embedder;
use super::rank::search;
use super::store::index_dir;
use super::{read_text, IndexDie, GOLDEN_NAME};
use crate::error::{crash, CrashKind, R};
use crate::out;
use crate::pyos::{self, py_isspace, py_repr_str, py_splitlines, py_strip};

pub struct Golden {
    pub q: String,
    pub expect: Vec<String>,
}

/// `load_golden(root)`: `- q:` lines, each followed by `expect:` lines.
pub fn load_golden(root: &str) -> R<Result<Vec<Golden>, IndexDie>> {
    let path = pyos::join(&index_dir(root), GOLDEN_NAME);
    if !pyos::isfile(&path) {
        return Ok(Err(IndexDie(format!(
            "no golden set at {} — harvest one first (question lines '- q:' with 'expect:' path lines beneath each)",
            pyos::relpath(&path, root)?
        ))));
    }
    let mut records: Vec<Golden> = Vec::new();
    for raw in py_splitlines(&read_text(&path)?) {
        let line = raw.trim_end_matches(py_isspace);
        if line.trim_start_matches(py_isspace).starts_with('#') || py_strip(line).is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("- q:") {
            records.push(Golden { q: py_strip(rest).to_string(), expect: Vec::new() });
        } else if let Some(rest) = py_strip(line).strip_prefix("expect:") {
            if let Some(cur) = records.last_mut() {
                cur.expect.push(py_strip(rest).to_string());
            }
        }
    }
    if records.is_empty() {
        return Ok(Err(IndexDie("golden set parsed empty — check its '- q:' / 'expect:' shape".into())));
    }
    Ok(Ok(records))
}

/// `f"{x:.0%}"`
fn pct0(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

/// `cmd_eval(root, k)` (after the caller's refresh).
pub fn cmd_eval(root: &str, k: i64, embedder: Embedder) -> R<Result<(), IndexDie>> {
    let records = match load_golden(root)? {
        Ok(r) => r,
        Err(d) => return Ok(Err(d)),
    };
    let (mut hits, mut mrr_total) = (0usize, 0.0f64);
    let mut rows = Vec::new();
    for (i, rec) in records.iter().enumerate() {
        let ranked: Vec<String> = search(root, &rec.q, k, embedder)?.into_iter().map(|h| h.path).collect();
        let mut uniq: Vec<String> = Vec::new();
        for p in ranked {
            if !uniq.contains(&p) {
                uniq.push(p);
            }
        }
        let rank = rec.expect.iter().find_map(|exp| uniq.iter().position(|u| u == exp).map(|i| i + 1));
        let ok = rank.is_some();
        hits += ok as usize;
        mrr_total += rank.map(|r| 1.0 / r as f64).unwrap_or(0.0);
        let expected = rec
            .expect
            .first()
            .cloned()
            .ok_or_else(|| crash(CrashKind::Other, "IndexError: list index out of range"))?;
        rows.push((i + 1, rec.q.clone(), ok, expected));
    }
    let n = records.len();
    out::line(&format!(
        "af index eval: {n} queries · recall@{k} = {hits}/{n} = {} · MRR = {:.3}",
        pct0(hits as f64 / n as f64),
        mrr_total / n as f64
    ));
    out::line("(replace the Current Standing section of system/index/EVALS.md)\n");
    for (i, q, ok, expected) in rows {
        let mark = if ok { "PASS" } else { "MISS" };
        out::line(&format!("  [{mark}] q{i:02} {}", py_repr_str(&pyos::py_prefix(&q, 52))));
        if !ok {
            out::line(&format!("         wanted: {expected}"));
        }
    }
    out::line(&format!("\nerror analysis input: {} miss(es)", n - hits));
    Ok(Ok(()))
}
