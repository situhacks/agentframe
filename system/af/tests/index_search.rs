//! `af index` / `af search` on fixtures with no Ollama: the binary pointed at a dead port
//! (keyword-only path), and the library with a deterministic fake embedder (fusion).

mod common;

use afcore::index::embed::EmbedErr;
use afcore::index::{rank, store};
use common::Fx;

const DEAD: &str = "http://127.0.0.1:9/api/embed";

fn vault() -> Fx {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    fx.mkdir("system");
    let p = "workspace/projects/acme";
    fx.write(
        &format!("{p}/project.md"),
        "---\nname: acme\ndeliverables:\n  brief:\n    file: brief/brief-v2.md\n---\n# Acme\n\nProject tracker.\n",
    );
    fx.write(&format!("{p}/brief/brief-v1.md"), "# Brief\n\n## Audience\nzebra launch audience, first cut\n");
    fx.write(&format!("{p}/brief/brief-v2.md"), "# Brief\n\n## Audience\nzebra launch audience, second cut\n");
    fx.write(&format!("{p}/notes/plain.md"), "---\nstatus: drafting\n---\n# Notes\n\n## Deep\n### Deeper\nzebra launch notes\n");
    fx.write(&format!("{p}/sources/INDEX.md"), "# Sources\n\n- sources/call.md\n");
    fx.write(&format!("{p}/sources/call.md"), "# Call\n\nzebra launch call notes\n");
    fx.write(&format!("{p}/_archive/old.md"), "# Old\n\nzebra launch archived\n");
    fx.write(&format!("{p}/node_modules/x/README.md"), "# skip\n\nzebra launch\n");
    fx.write("workspace/life/career.md", "# Career\n\nzebra launch in life\n");
    fx.write("workspace/life/voice/v.md", "# Voice\n\nzebra launch voice\n");
    fx.write("library/assets/a.txt", "an ox and a zebra launch asset\n");
    fx
}

fn dead(fx: &Fx, args: &[&str]) -> (i32, String, String) {
    let out = fx.af().env("AF_OLLAMA_URL", DEAD).args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n"),
    )
}

fn db(fx: &Fx) -> rusqlite::Connection {
    rusqlite::Connection::open(fx.p("system/index/vault.db")).unwrap()
}

#[test]
fn update_walks_the_corpus_skips_and_warns_keyword_only() {
    let fx = vault();
    let (code, out, err) = dead(&fx, &["index", "update"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("af index: scanned 8 files (7 md, 1 txt) · +8 new · ~0 updated · =0 unchanged (0 touch-only) · -0 gone · 8 tracked / "), "{out}");
    assert!(out.contains(" · E0/"), "{out}");
    assert!(err.contains("af index: WARNING — embeddings unavailable (URLError: <urlopen error "), "{err}");
    assert!(err.contains("indexed keyword-only. Start Ollama and re-run `af index update` to fill vectors."));
    let con = db(&fx);
    let paths: Vec<String> = con
        .prepare("SELECT path FROM files ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!paths.contains(&"workspace/projects/acme/_archive/old.md".to_string()));
    assert!(!paths.iter().any(|p| p.contains("node_modules") || p.contains("life/voice")));
    assert!(paths.contains(&"library/assets/a.txt".to_string()));
    let (crumb, status): (String, Option<String>) = con
        .query_row(
            "SELECT c.breadcrumb, f.status FROM chunks c JOIN files f ON f.path = c.path WHERE c.path LIKE '%plain.md' AND c.body LIKE 'zebra%'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(crumb, "plain › Notes › Deep › Deeper");
    assert_eq!(status.as_deref(), Some("drafting"));
    let heads: Vec<(String, i64, Option<i64>)> = con
        .prepare("SELECT path, is_head, version FROM files WHERE path LIKE '%brief-v%' ORDER BY path")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(heads[0].1, 0);
    assert_eq!(heads[0].2, Some(1));
    assert_eq!(heads[1].1, 1);

    let (_, status_out, _) = dead(&fx, &["index", "status"]);
    assert!(status_out.contains("af index: db system"), "{status_out}");
    assert!(status_out.contains("chunks via qwen3-embedding:0.6b — keyword-only until you run `af index update`"));
    assert!(status_out.contains("  stale       : 0 (run `af index update`)"));
    assert!(status_out.contains("  superseded  : 1 prior versions ranked down"));
}

#[test]
fn keyword_search_ranks_by_reciprocal_rank_and_boosts() {
    let fx = vault();
    dead(&fx, &["index", "update"]);
    let (code, out, err) = dead(&fx, &["search", "zebra launch", "--json", "-n", "20"]);
    assert_eq!(code, 0, "{err}");
    // no vectors, so no semantic attempt and no note
    assert!(!err.contains("semantic scoring unavailable"), "{err}");
    let hits: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
    let by_path = |p: &str| hits.iter().find(|h| h["path"].as_str().unwrap().ends_with(p)).cloned().unwrap();
    let why = |h: &serde_json::Value| h["why"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect::<Vec<_>>();
    let v1 = by_path("brief-v1.md");
    assert!(why(&v1).contains(&"superseded-version".to_string()), "{v1}");
    let v2 = by_path("brief-v2.md");
    assert_eq!(why(&v2), vec!["tracker-head", "bm25"]);
    assert_eq!(why(&by_path("career.md")), vec!["life", "bm25"]);
    assert_eq!(why(&by_path("sources/call.md")), vec!["primary-material", "registered-source", "bm25"]);
    // Six files say "zebra launch" (archive, node_modules and voice are outside the corpus);
    // hits come best first, and every score is 1/(60 + lexical rank) times its boosts.
    assert_eq!(hits.len(), 6);
    let scores: Vec<f64> = hits.iter().map(|h| h["score"].as_f64().unwrap()).collect();
    let mut sorted = scores.clone();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
    assert_eq!(scores, sorted);
    let rrf: Vec<f64> = (1..=6).map(|r| 1.0 / (60.0 + r as f64)).collect();
    let tracker = v2["score"].as_f64().unwrap() / 1.9;
    assert!(rrf.iter().any(|x| (x - tracker).abs() < 1e-6), "{tracker}");
    let superseded = v1["score"].as_f64().unwrap() / 0.55;
    assert!(rrf.iter().any(|x| (x - superseded).abs() < 2e-6), "{superseded}");
}

#[test]
fn short_tokens_scan_by_like_and_text_output_reads_like_python() {
    let fx = vault();
    dead(&fx, &["index", "update"]);
    let (_, out, _) = dead(&fx, &["search", "ox"]);
    assert!(out.starts_with("af search: 1 hits for 'ox'  (open cited files before relying on snippets)\n\n1. [  0.016] library/assets/a.txt\n   a\n   …an ox and a zebra launch asset…\n   why: bm25\n\n"), "{out}");
    let (_, out, _) = dead(&fx, &["search", "qqqzzz"]);
    assert_eq!(out, "af search: no hits for 'qqqzzz' — try fewer or different words, or `af index update` if files changed recently\n");
    let (_, out, _) = dead(&fx, &["search", "zebra", "-n", "0", "--json"]);
    assert_eq!(out, "[]\n");
}

#[test]
fn search_refreshes_changed_files_first() {
    let fx = vault();
    dead(&fx, &["index", "update"]);
    fx.write("workspace/new.md", "# New\n\nzebra launch fresh\n");
    fx.remove("library/assets/a.txt");
    let (_, out, _) = dead(&fx, &["search", "fresh"]);
    assert!(out.starts_with("af index: refreshed first (+1 new · ~0 updated · -1 gone)\n\naf search: 1 hits for 'fresh'"), "{out}");
}

#[test]
fn eval_refuses_without_a_golden_set_then_scores_one() {
    let fx = vault();
    let (code, out, err) = dead(&fx, &["index", "eval"]);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("af index: ERROR: no golden set at system\\index\\golden-set.yaml") || err.contains("af index: ERROR: no golden set at system/index/golden-set.yaml"), "{err}");
    let rows = fx.run_log_rows();
    let last = rows.last().unwrap();
    assert_eq!(last["verb"], "index eval");
    assert_eq!(last["exit"], 1);
    assert!(last.get("refusal").is_none(), "the indexer's die records no refusal: {last}");

    fx.write(
        "system/index/golden-set.yaml",
        "# set\n- q: zebra audience second\n  expect: workspace/projects/acme/brief/brief-v2.md\n- q: absent words\n  expect: nowhere.md\n",
    );
    let (code, out, _) = dead(&fx, &["index", "eval", "--k", "3"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("af index eval: 2 queries · recall@3 = 1/2 = 50% · MRR = "), "{out}");
    assert!(out.contains("  [PASS] q01 'zebra audience second'\n"), "{out}");
    assert!(out.contains("  [MISS] q02 'absent words'\n         wanted: nowhere.md\n"), "{out}");
    assert!(out.ends_with("\nerror analysis input: 1 miss(es)\n"));
}

/// A deterministic stand-in for the model: a 4-d vector from letter counts.
fn fake(texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedErr> {
    Ok(texts
        .iter()
        .map(|t| {
            let t = t.rsplit("Query: ").next().unwrap().to_lowercase();
            ["zebra", "audience", "career", "asset"].iter().map(|w| t.matches(w).count() as f64 + 0.01).collect()
        })
        .collect())
}

fn broken(_: &[String]) -> Result<Vec<Vec<f64>>, EmbedErr> {
    Err(EmbedErr::new("URLError", "<urlopen error down>"))
}

#[test]
fn fusion_adds_both_retrievers_and_semantic_only_hits_borrow_their_first_chunk() {
    let fx = vault();
    let root = fx.root();
    let s = store::cmd_update(&root, false, true, &fake).unwrap();
    assert_eq!(s.embedded, s.chunks);
    let hits = rank::search(&root, "career", 50, &fake).unwrap();
    let career = hits.iter().find(|h| h.path == "workspace/life/career.md").unwrap();
    assert_eq!(career.why, vec!["life", "semantic", "bm25"]);
    // lexical rank 1 and semantic rank 1: (1/61 + 1/61) * 1.5
    let want = ((1.0f64 / 61.0 + 1.0 / 61.0) * 1.5 * 1e6).round() / 1e6;
    assert!((career.score - want).abs() < 1e-12, "{} vs {want}", career.score);
    // a semantic-only hit carries its first chunk's breadcrumb
    let sem_only = hits.iter().find(|h| h.why.last() == Some(&"semantic")).expect("a semantic-only hit");
    assert!(!sem_only.breadcrumb.is_empty());
    assert_eq!(hits[0].path, "workspace/life/career.md");

    // A backend that fails mid-search degrades to keyword-only for that run.
    let kw = rank::search(&root, "career", 50, &broken).unwrap();
    assert!(kw.iter().all(|h| !h.why.contains(&"semantic")));
    assert_eq!(kw[0].path, "workspace/life/career.md");
}

#[test]
fn a_failed_batch_is_backfilled_on_the_next_update() {
    let fx = vault();
    let root = fx.root();
    let s = store::cmd_update(&root, false, true, &broken).unwrap();
    assert_eq!(s.embedded, 0);
    let s = store::cmd_update(&root, false, true, &fake).unwrap();
    assert_eq!((s.added, s.updated, s.backfilled), (0, 0, s.chunks));
    assert_eq!(s.embedded, s.chunks);
    // a model with another dimension is refused, not mixed
    let wide = |t: &[String]| -> Result<Vec<Vec<f64>>, EmbedErr> { Ok(t.iter().map(|_| vec![1.0; 8]).collect()) };
    fx.write("workspace/projects/acme/notes/plain.md", "# Notes\n\nchanged zebra\n");
    let s = store::cmd_update(&root, false, true, &wide).unwrap();
    assert_eq!(s.updated, 1);
    assert!(s.embedded < s.chunks);
}

#[test]
fn rebuild_starts_from_empty_tables() {
    let fx = vault();
    let root = fx.root();
    store::cmd_update(&root, false, true, &fake).unwrap();
    let before: i64 = db(&fx).query_row("SELECT MAX(chunk_id) FROM chunks", [], |r| r.get(0)).unwrap();
    let s = store::cmd_update(&root, true, true, &fake).unwrap();
    assert_eq!(s.added, 8);
    let after: i64 = db(&fx).query_row("SELECT MAX(chunk_id) FROM chunks", [], |r| r.get(0)).unwrap();
    assert_eq!(before, after, "AUTOINCREMENT restarts with the dropped table");
}
