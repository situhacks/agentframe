//! `af voice bundle`.

mod common;

use afcore::voice::bundle::{self, build, default_tier, exemplar, format_report, recipe_from_head, Place, Recipe, DENSITY_LINE};
use common::Fx;

fn seed(fx: &Fx, voice: &str) {
    for (rel, text) in [
        ("README.md", "# procedure\n"),
        ("identity.md", "# identity\n"),
        ("voice-profile.md", "# profile\n"),
        ("anti-patterns.md", "# anti\n"),
        ("registers/informal.md", "# informal register\n"),
        ("registers/formal.md", "# formal register\n"),
        ("pairs/informal.md", "### pair-a\n"),
        ("pairs/plain-not-clever.md", "### pair-b\n"),
        ("pairs/builder-pov.md", "### pair-c\n"),
        ("templates/substack-essay.md", "# essay shape\n"),
        ("corpus/informal/2026-02-02-second.md", "Provenance: b\n\nSecond piece.\n"),
        ("corpus/informal/2026-01-01-first.md", "Provenance: a\n\nFirst piece.\n"),
        ("corpus/informal/README.md", "not a piece\n"),
    ] {
        fx.write(&format!("{voice}/{rel}"), text);
    }
}

struct Dirs {
    fx: Fx,
    voice: String,
    out: String,
}

fn dirs() -> Dirs {
    let fx = Fx::new();
    seed(&fx, "voice");
    let (voice, out) = (fx.p("voice"), fx.p("out"));
    Dirs { fx, voice, out }
}

impl Dirs {
    fn place(&self) -> Place<'_> {
        Place { root: &self.fx.dir.path().to_str().unwrap(), voice_dir: &self.voice, out_dir: &self.out, stamp: None }
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn ends_with(path: &str, tail: &[&str]) -> bool {
    path.ends_with(&tail.join(std::path::MAIN_SEPARATOR_STR))
}

#[test]
fn test_order_and_manifest() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("informal", &["long-form"], &[], Some("substack-essay.md")), None, None).unwrap();
    assert_eq!(
        r.labels(),
        s(&[
            "procedure",
            "identity",
            "profile",
            "anti-patterns",
            "register informal",
            "template substack-essay.md",
            "pairs informal.md",
            "pairs plain-not-clever.md",
            "corpus 2026-01-01-first.md",
            "corpus 2026-02-02-second.md",
        ])
    );
    assert!(r.missing.is_empty());
    let text = std::fs::read_to_string(&r.path).unwrap();
    assert!(text.starts_with("<!-- VOICE BUNDLE"));
    assert!(text.contains("Read this file whole"));
    assert!(text.contains(DENSITY_LINE));
    assert!(text.find("First piece.").unwrap() < text.find("Second piece.").unwrap());
    assert!(!text.contains("not a piece"));
    assert!(ends_with(&r.path, &["out", "informal-long-form.md"]));
    assert_eq!(r.tier, "full");
    assert!(text.contains(&format!("VOICE BUNDLE id {}", r.id)));
    assert!(text.contains("tier full"));
    assert!(format_report(d.place().root, &r).unwrap().contains("--tier core"));
}

#[test]
fn test_core_tier_is_rules_plus_one_exemplar() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("informal", &["email"], &[], Some("substack-essay.md")), None, None).unwrap();
    assert_eq!(r.tier, "core");
    assert_eq!(
        r.labels(),
        s(&[
            "procedure",
            "identity",
            "profile",
            "anti-patterns",
            "register informal",
            "template substack-essay.md",
            "exemplar 2026-02-02-second.md",
        ])
    );
    assert!(r.missing.is_empty());
    assert!(ends_with(&r.path, &["out", "informal-email-core.md"]));
    let text = std::fs::read_to_string(&r.path).unwrap();
    assert!(text.contains("tier core"));
    assert!(!text.contains("First piece."));
    assert!(!text.contains("### pair-"));
    let report = format_report(d.place().root, &r).unwrap();
    assert!(report.contains("--tier full"));
    assert!(report.contains(&format!("bundle {}", r.id)));
    let full = build(&d.place(), &Recipe::new("informal", &["email"], &[], None), None, Some("full")).unwrap();
    assert_eq!(full.tier, "full");
    assert!(ends_with(&full.path, &["out", "informal-email.md"]));
    assert_ne!(full.id, r.id);
}

#[test]
fn test_default_tier_follows_the_contexts() {
    assert_eq!(default_tier(&s(&["email"])), "core");
    assert_eq!(default_tier(&s(&["short-form", "email"])), "core");
    assert_eq!(default_tier(&s(&["email", "long-form"])), "full");
    assert_eq!(default_tier(&[]), "full");
    assert_eq!(default_tier(&s(&["slide"])), "full");
}

#[test]
fn test_exemplar_is_newest_under_cap_else_smallest() {
    let fx = Fx::new();
    fx.write("corpus/formal/2026-01-01-a.md", &"a".repeat(50));
    fx.write("corpus/formal/2026-02-01-b.md", &"b".repeat(200));
    fx.write("corpus/formal/2026-03-01-c.md", &"c".repeat(80));
    fx.write("corpus/formal/README.md", "not a piece");
    let corpus = fx.p("corpus/formal");
    assert!(exemplar(&corpus, 100).unwrap().unwrap().ends_with("2026-03-01-c.md"));
    assert!(exemplar(&corpus, 60).unwrap().unwrap().ends_with("2026-01-01-a.md"));
    assert!(exemplar(&corpus, 10).unwrap().unwrap().ends_with("2026-01-01-a.md"));
    assert!(exemplar(&fx.p("corpus/informal"), bundle::CORE_EXEMPLAR_CAP).unwrap().is_none());
}

#[test]
fn test_borrow_and_context_pairs_dedupe() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("informal", &["builder-pov", "long-form"], &["formal"], None), None, None).unwrap();
    let labels = r.labels();
    assert!(labels.contains(&"borrow register formal".to_string()));
    assert_eq!(labels.iter().filter(|l| *l == "pairs plain-not-clever.md").count(), 1);
    assert!(labels.contains(&"pairs builder-pov.md".to_string()));
    assert_eq!(r.missing.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(), s(&["pairs formal.md"]));
}

#[test]
fn test_missing_files_are_reported_not_fatal() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("formal", &[], &[], None), None, None).unwrap();
    assert!(r.missing.iter().any(|(m, _)| m == "pairs formal.md"));
    assert!(r.parts.iter().any(|(l, _)| l == "register formal"));
    let report = format_report(d.place().root, &r).unwrap();
    assert!(report.contains("MISSING"));
    assert!(report.contains("Read it whole"));
}

#[test]
fn test_voice_block_and_type() {
    let fx = Fx::new();
    let head = fx.write(
        "essay-v3.md",
        "---\nstatus: drafting\ntype: substack-essay\nregister: substack-informal\nvoice:\n  base_register: informal\n  borrow_from: [formal]\n---\nBody.\n",
    );
    let (base, borrow, dtype) = recipe_from_head(&head).unwrap();
    assert_eq!((base.as_deref(), borrow, dtype.as_deref()), (Some("informal"), s(&["formal"]), Some("substack-essay")));
    assert_eq!(bundle::type_template("substack-essay"), Some("substack-essay.md"));
    assert_eq!(bundle::type_context("substack-essay"), Some("long-form"));
}

#[test]
fn test_register_scalar_only() {
    let fx = Fx::new();
    let head = fx.write("note-v1.md", "---\nregister: formal\n---\nBody.\n");
    assert_eq!(recipe_from_head(&head).unwrap(), (Some("formal".to_string()), vec![], None));
}

#[test]
fn test_no_recipe() {
    let fx = Fx::new();
    let head = fx.write("plain-v1.md", "---\nstatus: drafting\n---\nBody.\n");
    assert_eq!(recipe_from_head(&head).unwrap(), (None, vec![], None));
}

// ------------------------------------------------------------------ the verb

const HEAD: &str = "---\nstatus: drafting\ntype: substack-essay\nvoice:\n  base_register: informal\n---\nBody.\n";

fn verb_fixture() -> Fx {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    seed(&fx, "workspace/life/voice");
    fx.write(
        "workspace/projects/p/project.md",
        "---\nname: p\nslug: p\ndeliverables:\n  essay:\n    status: drafting\n    file: essay/essay-v1.md\n  ghost:\n    status: drafting\n    file: essay/ghost-v1.md\n---\n",
    );
    fx.write("workspace/projects/p/essay/essay-v1.md", HEAD);
    fx
}

#[test]
fn test_verb_bundles_from_the_head_recipe() {
    let fx = verb_fixture();
    let out = fx.ok(&["voice", "bundle", "p", "essay"]);
    assert!(out.starts_with("af voice bundle: system/index/voice/informal-long-form.md — 10 files"), "{out}");
    assert!(out.contains("template substack-essay.md"));
    let text = fx.read("system/index/voice/informal-long-form.md");
    assert!(text.contains("head workspace/projects/p/essay/essay-v1.md."));
    let rows = fx.run_log_rows();
    assert_eq!(rows.last().unwrap()["verb"], "voice");
}

#[test]
fn test_verb_register_without_a_project() {
    let fx = verb_fixture();
    let out = fx.ok(&["voice", "bundle", "--register", "informal", "--context", "email"]);
    assert!(out.contains("informal-email-core.md"), "{out}");
    assert!(out.contains("head none") || fx.read("system/index/voice/informal-email-core.md").contains("head none"));
}

#[test]
fn test_verb_refusals() {
    let fx = verb_fixture();
    assert!(fx.refused(&["voice", "bundle", "p"]).contains("give both project and deliverable"));
    assert!(fx.refused(&["voice", "bundle", "p", "ghost"]).contains("head not found: essay/ghost-v1.md"));
    assert!(fx.refused(&["voice", "bundle", "nope", "essay"]).contains("project 'nope' not found"));
    fx.write("workspace/projects/p/essay/essay-v1.md", "---\nstatus: drafting\n---\nBody.\n");
    assert!(fx.refused(&["voice", "bundle", "p", "essay"]).contains("declares no voice recipe"));
    let (code, _, _) = fx.run(&["voice", "bundle", "--register", "neither"]);
    assert_eq!(code, 2);
}
