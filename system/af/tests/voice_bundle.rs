//! `af voice bundle`.

mod common;

use afcore::voice::bundle::{
    self, build, default_tier, excerpt_text, exemplar, format_report, recipe_from_head, Place, Recipe, DENSITY_LINE,
};
use common::Fx;

fn seed(fx: &Fx, voice: &str) {
    for (rel, text) in [
        ("README.md", "# procedure\n"),
        ("identity.md", "# identity\n"),
        ("voice-profile.md", "# profile\n"),
        ("anti-patterns.md", "# anti\n"),
        ("registers/informal.md", "# informal register\n"),
        ("registers/formal.md", "# formal register\n"),
        ("registers/chat.md", "# chat register\n"),
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
    let report = format_report(d.place().root, &r).unwrap();
    assert!(report.contains("--tier standard is"));
    assert!(!report.contains("--tier core"));
}

#[test]
fn test_standard_tier_is_rules_plus_one_exemplar() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("informal", &["email"], &[], Some("substack-essay.md")), None, None).unwrap();
    assert_eq!(r.tier, "standard");
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
    assert!(ends_with(&r.path, &["out", "informal-email-standard.md"]));
    let text = std::fs::read_to_string(&r.path).unwrap();
    assert!(text.contains("tier standard"));
    assert!(!text.contains("First piece."));
    assert!(!text.contains("### pair-"));
    let report = format_report(d.place().root, &r).unwrap();
    assert!(report.contains("--tier full is"));
    assert!(report.contains("--tier quick is"));
    assert!(report.contains("for a first draft, or anything over a page: adds the pairs and the whole corpus."));
    assert!(report.contains("for a text, a direct message, a comment or a one-paragraph reply"));
    assert!(report.contains(&format!("bundle {}", r.id)));
    let full = build(&d.place(), &Recipe::new("informal", &["email"], &[], None), None, Some("full")).unwrap();
    assert_eq!(full.tier, "full");
    assert!(ends_with(&full.path, &["out", "informal-email.md"]));
    assert_ne!(full.id, r.id);
}

#[test]
fn test_core_is_an_alias_of_standard() {
    let d = dirs();
    let recipe = Recipe::new("informal", &["email"], &[], Some("substack-essay.md"));
    let std_ = build(&d.place(), &recipe, None, Some("standard")).unwrap();
    let core = build(&d.place(), &recipe, None, Some("core")).unwrap();
    assert_eq!(core.tier, "standard");
    assert_eq!(core.labels(), std_.labels());
    assert_eq!(core.id, std_.id);
    assert!(ends_with(&core.path, &["out", "informal-email-standard.md"]));
    let text = std::fs::read_to_string(&core.path).unwrap();
    assert!(text.contains("tier standard") && !text.contains("tier core"));
    let report = format_report(d.place().root, &core).unwrap();
    assert!(report.contains("tier standard (asked)") && !report.contains("tier core"));
}

#[test]
fn test_quick_tier_is_rules_register_and_one_excerpt() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("informal", &["message"], &["formal"], Some("substack-essay.md")), None, None).unwrap();
    assert_eq!(r.tier, "quick");
    assert_eq!(
        r.labels(),
        s(&["anti-patterns", "identity", "register informal", "borrow register formal", "excerpt 2026-02-02-second.md"])
    );
    assert!(r.missing.is_empty());
    assert!(ends_with(&r.path, &["out", "informal-message-quick.md"]));
    let text = std::fs::read_to_string(&r.path).unwrap();
    for gone in ["# procedure", "# profile", "# essay shape", "### pair-", "First piece."] {
        assert!(!text.contains(gone), "{gone}");
    }
    let pos = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(pos("# anti") < pos("# identity"));
    assert!(pos("# identity") < pos("# informal register"));
    assert!(pos("# informal register") < pos("# formal register"));
    assert!(pos("# formal register") < pos("Second piece."));
    assert!(text.contains("an excerpt of "));
    assert!(text.contains("tier quick"));
}

fn words(n: usize, tag: &str) -> String {
    (0..n).map(|i| format!("{tag}{i}")).collect::<Vec<_>>().join(" ")
}

#[test]
fn test_excerpt_takes_whole_paragraphs_up_to_400_words() {
    let raw = format!(
        "---\ntitle: x\ndate: 2026-01-01\n---\n\n{}\n\n{}\n\n{}\n\n{}\n",
        words(150, "a"),
        words(150, "b"),
        words(100, "c"),
        words(100, "d")
    );
    let (text, n) = excerpt_text(&raw);
    assert_eq!(n, 400);
    assert!(!text.contains("title:") && !text.contains("---"));
    assert!(text.starts_with("a0 ") && text.contains("c99") && !text.contains("d0"));
    assert_eq!(text.split_whitespace().count(), 400);
    assert_eq!(text.matches("\n\n").count(), 2);
    // one more word in the third paragraph pushes it out
    let raw = format!("{}\n\n{}\n\n{}\n", words(150, "a"), words(150, "b"), words(101, "c"));
    let (text, n) = excerpt_text(&raw);
    assert_eq!(n, 300);
    assert!(!text.contains("c0"));
}

#[test]
fn test_excerpt_cuts_a_long_first_paragraph() {
    let (text, n) = excerpt_text(&format!("{}\n\nSecond.\n", words(500, "w")));
    assert_eq!(n, 400);
    assert!(text.ends_with(" …"));
    assert!(text.contains("w399") && !text.contains("w400") && !text.contains("Second"));
    assert_eq!(text.trim_end_matches(" …").split_whitespace().count(), 400);
    // a first paragraph of exactly 400 words is whole, with no mark
    let (text, n) = excerpt_text(&words(400, "w"));
    assert_eq!((n, text.ends_with('…')), (400, false));
    // a short piece is taken whole
    assert_eq!(excerpt_text("Only one.\n"), ("Only one.".to_string(), 2));
}

#[test]
fn test_excerpt_in_a_built_bundle_and_no_corpus_no_excerpt() {
    let d = dirs();
    d.fx.write(
        "voice/corpus/formal/2026-03-03-long.md",
        &format!("---\nfm: 1\n---\n{}\n\n{}\n", words(500, "w"), "Never reached."),
    );
    let r = build(&d.place(), &Recipe::new("formal", &[], &[], None), None, Some("quick")).unwrap();
    assert_eq!(r.labels().last().unwrap(), "excerpt 2026-03-03-long.md");
    let text = std::fs::read_to_string(&r.path).unwrap();
    assert!(text.contains("an excerpt of 400 words"));
    assert!(text.contains("w399 …"));
    assert!(!text.contains("fm: 1") && !text.contains("Never reached."));
    // no corpus folder: no excerpt part and nothing missing
    let chat = build(&d.place(), &Recipe::new("chat", &[], &[], None), None, Some("quick")).unwrap();
    assert_eq!(chat.labels(), s(&["anti-patterns", "identity", "register chat"]));
    assert!(chat.missing.is_empty());
    assert!(!format_report(d.place().root, &chat).unwrap().contains("MISSING"));
}

#[test]
fn test_excerpt_of_frontmatter_only_or_empty_body() {
    // frontmatter with nothing between its fences is still frontmatter
    assert_eq!(excerpt_text("---\n---\nBody here.\n"), ("Body here.".to_string(), 2));
    assert_eq!(excerpt_text("---\r\n---\r\nBody here.\r\n"), ("Body here.".to_string(), 2));
    // a rule later in the body is not taken for the closing fence
    assert_eq!(excerpt_text("---\n---\nOne.\n\n---\n\nTwo.\n").1, 3);
    // an empty body gives nothing
    assert_eq!(excerpt_text("---\ntitle: x\n---\n"), (String::new(), 0));
    assert_eq!(excerpt_text("---\n---\n"), (String::new(), 0));
    // and the built bundle has no excerpt part and no MISSING line for such a piece
    let d = dirs();
    d.fx.write("voice/corpus/formal/2026-03-03-empty.md", "---\ntitle: x\n---\n");
    let r = build(&d.place(), &Recipe::new("formal", &[], &[], None), None, Some("quick")).unwrap();
    assert_eq!(r.labels(), s(&["anti-patterns", "identity", "register formal"]));
    assert!(r.missing.is_empty());
    let text = std::fs::read_to_string(&r.path).unwrap();
    assert!(!text.contains("an excerpt of 0 words") && !text.contains("excerpt 2026"));
}

#[test]
fn test_bundle_id_follows_the_excerpt_text() {
    let d = dirs();
    let recipe = Recipe::new("informal", &["message"], &[], None);
    let id = |d: &Dirs| build(&d.place(), &recipe, None, Some("quick")).unwrap().id;
    let before = id(&d);
    assert_eq!(id(&d), before);
    d.fx.write("voice/corpus/informal/2026-02-02-second.md", "Provenance: b\n\nSecond piece, changed.\n");
    assert_ne!(id(&d), before);
    // text past the 400th word is not in the excerpt, so it does not move the id
    d.fx.write("voice/corpus/informal/2026-02-02-second.md", &format!("{}\n\n{}\n", words(300, "w"), words(150, "x")));
    let short = id(&d);
    d.fx.write("voice/corpus/informal/2026-02-02-second.md", &format!("{}\n\n{}\n", words(300, "w"), words(150, "y")));
    assert_eq!(id(&d), short);
    d.fx.write("voice/corpus/informal/2026-02-02-second.md", &format!("{}\n\n{}\n", words(299, "w") + " changed", words(150, "y")));
    assert_ne!(id(&d), short);
}

#[test]
fn test_anti_patterns_is_in_every_tier() {
    let d = dirs();
    for tier in ["quick", "standard", "core", "full"] {
        let r = build(&d.place(), &Recipe::new("informal", &["email"], &[], None), None, Some(tier)).unwrap();
        assert_eq!(r.tier, if tier == "core" { "standard" } else { tier }, "{tier}");
        if tier == "quick" {
            assert_eq!(r.labels()[0], "anti-patterns");
        }
        assert!(r.labels().contains(&"anti-patterns".to_string()), "{tier}");
        assert!(std::fs::read_to_string(&r.path).unwrap().contains("# anti"), "{tier}");
    }
}

fn tier_of(d: &Dirs, register: &str, contexts: &[&str]) -> (String, String, String) {
    let r = build(&d.place(), &Recipe::new(register, contexts, &[], None), None, None).unwrap();
    (r.tier.clone(), r.tier_why.clone(), r.path.rsplit(['/', '\\']).next().unwrap().to_string())
}

#[test]
fn test_default_tier_follows_the_contexts() {
    let t = |c: &[&str], reg: &str| default_tier(&s(c), reg).0;
    // full when any context asks for it
    for c in ["long-form", "builder-pov", "market-signal", "slide", "cover"] {
        assert_eq!(t(&[c], "informal"), "full", "{c}");
        assert_eq!(t(&["message", c], "chat"), "full", "{c}");
        assert_eq!(t(&["email", c], "informal"), "full", "{c}");
    }
    // quick when every context is message or chat
    assert_eq!(t(&["message"], "informal"), "quick");
    assert_eq!(t(&["chat"], "formal"), "quick");
    assert_eq!(t(&["message", "chat"], "informal"), "quick");
    // standard for a mix of email, short-form, message, chat
    assert_eq!(t(&["email"], "informal"), "standard");
    assert_eq!(t(&["short-form", "email"], "informal"), "standard");
    assert_eq!(t(&["email", "message"], "informal"), "standard");
    assert_eq!(t(&["short-form", "chat"], "chat"), "standard");
    // no context: chat is quick, every other register standard
    assert_eq!(t(&[], "chat"), "quick");
    assert_eq!(t(&[], "informal"), "standard");
    assert_eq!(t(&[], "formal"), "standard");
    assert_eq!(default_tier(&[], "formal").1, "default, no context");
    assert_eq!(default_tier(&s(&["message"]), "formal").1, "from context message");
}

#[test]
fn test_default_tier_builds_the_named_file_and_says_why() {
    let d = dirs();
    assert_eq!(
        tier_of(&d, "informal", &["message"]),
        ("quick".into(), "from context message".into(), "informal-message-quick.md".into())
    );
    assert_eq!(tier_of(&d, "informal", &["email", "message"]).2, "informal-email+message-standard.md");
    assert_eq!(tier_of(&d, "informal", &["slide"]), ("full".into(), "from context slide".into(), "informal-slide.md".into()));
    assert_eq!(
        tier_of(&d, "informal", &[]),
        ("standard".into(), "default, no context".into(), "informal-general-standard.md".into())
    );
    assert_eq!(
        tier_of(&d, "chat", &[]),
        ("quick".into(), "default, no context".into(), "chat-general-quick.md".into())
    );
    // an asked-for full with no context stays long-form: its file name and its context pair
    let full = build(&d.place(), &Recipe::new("informal", &[], &[], None), None, Some("full")).unwrap();
    assert!(ends_with(&full.path, &["out", "informal-long-form.md"]));
    assert!(full.labels().contains(&"pairs plain-not-clever.md".to_string()));
    // standard with no context adds no pair at all
    let q = build(&d.place(), &Recipe::new("informal", &[], &[], None), None, Some("standard")).unwrap();
    assert!(q.labels().iter().all(|l| !l.starts_with("pairs ")));
    let report = format_report(d.place().root, &full).unwrap();
    assert!(report.lines().next().unwrap().contains("tier full (asked)"), "{report}");
    let first = format_report(d.place().root, &q).unwrap();
    assert!(first.lines().next().unwrap().contains("tier standard (asked)"));
    let dflt = build(&d.place(), &Recipe::new("informal", &[], &[], None), None, None).unwrap();
    let line = format_report(d.place().root, &dflt).unwrap();
    assert!(line.lines().next().unwrap().contains("tier standard (default, no context)"));
}

#[test]
fn test_chat_register_builds_in_every_tier() {
    let d = dirs();
    for tier in ["quick", "standard", "core", "full"] {
        let r = build(&d.place(), &Recipe::new("chat", &["message"], &["informal"], None), None, Some(tier)).unwrap();
        assert!(r.missing.is_empty(), "{tier}: {:?}", r.missing);
        assert!(r.labels().contains(&"register chat".to_string()), "{tier}");
        let text = std::fs::read_to_string(&r.path).unwrap();
        assert!(text.contains("# chat register"), "{tier}");
    }
    let head = d.fx.write("t-v1.md", "---\nvoice:\n  base_register: chat\n  borrow_from: [chat, formal]\n---\nx\n");
    let (base, borrow, _) = recipe_from_head(&head).unwrap();
    assert_eq!((base.as_deref(), borrow), (Some("chat"), s(&["chat", "formal"])));
    // the plain `register:` key resolves to chat too
    let head = d.fx.write("u-v1.md", "---\nregister: chat\n---\nx\n");
    assert_eq!(recipe_from_head(&head).unwrap(), (Some("chat".to_string()), vec![], None));
}

#[test]
fn test_registers_pair_file_is_optional_but_a_context_pair_is_not() {
    let d = dirs();
    // formal has no pairs/formal.md: no MISSING line for it, and nothing listed
    let r = build(&d.place(), &Recipe::new("formal", &["email"], &["chat"], None), None, Some("full")).unwrap();
    assert!(r.missing.is_empty(), "{:?}", r.missing);
    assert!(!r.labels().contains(&"pairs formal.md".to_string()));
    assert!(r.labels().contains(&"pairs plain-not-clever.md".to_string()));
    // a register's pair file is added when it exists
    let i = build(&d.place(), &Recipe::new("informal", &["email"], &[], None), None, Some("full")).unwrap();
    assert!(i.labels().contains(&"pairs informal.md".to_string()));
    // a context's pair file that does not exist still prints MISSING
    let m = build(&d.place(), &Recipe::new("formal", &["market-signal"], &[], None), None, Some("full")).unwrap();
    assert_eq!(m.missing.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>(), s(&["pairs market-signal.md"]));
    assert!(format_report(d.place().root, &m).unwrap().contains("MISSING"));
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
    assert!(r.missing.is_empty(), "the register's own pair file is optional: {:?}", r.missing);
}

#[test]
fn test_missing_files_are_reported_not_fatal() {
    let d = dirs();
    let r = build(&d.place(), &Recipe::new("formal", &["market-signal"], &[], None), None, None).unwrap();
    assert!(r.missing.iter().any(|(m, _)| m == "pairs market-signal.md"));
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
    assert!(out.contains("informal-email-standard.md"), "{out}");
    assert!(out.contains("tier standard (from context email)"), "{out}");
    assert!(out.contains("head none") || fx.read("system/index/voice/informal-email-standard.md").contains("head none"));
    let core = fx.ok(&["voice", "bundle", "--register", "informal", "--context", "email", "--tier", "core"]);
    assert!(core.contains("informal-email-standard.md") && core.contains("tier standard (asked)"), "{core}");
}

#[test]
fn test_verb_chat_register_and_default_tiers() {
    let fx = verb_fixture();
    let out = fx.ok(&["voice", "bundle", "--register", "chat"]);
    assert!(out.contains("chat-general-quick.md") && out.contains("tier quick (default, no context)"), "{out}");
    assert!(!out.contains("MISSING"), "{out}");
    let out = fx.ok(&["voice", "bundle", "--register", "informal", "--borrow", "chat", "--context", "chat"]);
    assert!(out.contains("informal-chat-quick.md") && out.contains("borrow register chat"), "{out}");
    for tier in ["quick", "standard", "full"] {
        let out = fx.ok(&["voice", "bundle", "--register", "chat", "--tier", tier]);
        assert!(!out.contains("MISSING"), "{tier}: {out}");
    }
    let out = fx.ok(&["voice", "bundle", "--register", "informal"]);
    assert!(out.contains("informal-general-standard.md"), "{out}");
    let (code, _, _) = fx.run(&["voice", "bundle", "--register", "slang"]);
    assert_eq!(code, 2);
    let (code, _, _) = fx.run(&["voice", "bundle", "--register", "chat", "--tier", "huge"]);
    assert_eq!(code, 2);
}

#[test]
fn test_verb_refuses_a_mistyped_context() {
    let fx = verb_fixture();
    let (code, out, err) = fx.run(&["voice", "bundle", "--register", "informal", "--context", "mesage"]);
    assert_ne!(code, 0, "{out}{err}");
    for legal in ["long-form", "short-form", "email", "builder-pov", "market-signal", "slide", "cover", "message", "chat"] {
        assert!(err.contains(legal), "{legal} not listed: {err}");
    }
    // every legal context is accepted, and a head's own type still supplies one
    for c in ["long-form", "short-form", "email", "builder-pov", "market-signal", "slide", "cover", "message", "chat"] {
        let (code, out, err) = fx.run(&["voice", "bundle", "--register", "informal", "--context", c]);
        assert_eq!(code, 0, "{c}: {out}{err}");
    }
    assert!(fx.ok(&["voice", "bundle", "p", "essay"]).contains("from context long-form"));
}

#[test]
fn test_verb_head_type_and_default_tier() {
    let fx = verb_fixture();
    // an essay head gives a long-form context, so full
    let out = fx.ok(&["voice", "bundle", "p", "essay"]);
    assert!(out.contains("tier full (from context long-form)"), "{out}");
    // a head whose type maps to no context gives standard
    fx.write(
        "workspace/projects/p/essay/essay-v1.md",
        "---\nstatus: drafting\ntype: memo\nvoice:\n  base_register: informal\n---\nBody.\n",
    );
    let out = fx.ok(&["voice", "bundle", "p", "essay"]);
    assert!(out.contains("informal-general-standard.md") && out.contains("tier standard (default, no context)"), "{out}");
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
