//! `af tag` and the doctor's tag checks on a fixture vault with an invented vocabulary.

mod common;

use common::*;

const REGISTRY: &str = "\
facets:
  - name: cuisine
    applies: places
    cardinality: multi
  - name: vibe
    applies: places
    cardinality: multi
  - name: price
    applies: places
    cardinality: single
  - name: interest
    applies: people
    cardinality: multi
tags:
  - id: noodles
    facet: cuisine
    status: active
    alt: [noodle, noodle-bar]
    scope: \"Noodle houses of any tradition\"
    since: 2026-01-10
  - id: pastry
    facet: cuisine
    status: active
    alt: []
    scope: \"Bakeries and dessert counters\"
    since: 2026-01-10
  - id: quiet
    facet: vibe
    status: active
    alt: [hushed]
    scope: \"You can hear the table\"
    since: 2026-01-10
  - id: calm
    facet: vibe
    status: merged
    alt: []
    scope: \"\"
    merged_into: quiet
  - id: rooftop
    facet: vibe
    status: deprecated
    alt: []
    scope: \"\"
    since: 2026-01-10
  - id: price-low
    facet: price
    status: active
    alt: [cheap]
    scope: \"Under twenty a head\"
    since: 2026-01-10
  - id: price-high
    facet: price
    status: active
    alt: []
    scope: \"Over sixty a head\"
    since: 2026-01-10
  - id: fusion
    facet: cuisine
    status: proposed
    alt: []
    scope: \"\"
    since: 2026-01-10
  - id: boardgames
    facet: interest
    status: active
    alt: []
    scope: \"Plays board games\"
    since: 2026-01-10
";

fn place(fx: &Fx, slug: &str, kind: &str, area: &str, tags: &str, body: &str) {
    fx.write(
        &format!("workspace/life/places/{slug}.md"),
        &format!("---\nprivate: true\nkind: {kind}\narea: {area}\naddress: 1 Example St\ntags: [{tags}]\nstatus: want\nchecked: 2026-01-01\n---\n# {slug}\n{body}\n"),
    );
}

fn person(fx: &Fx, slug: &str, tags: &str, body: &str) {
    fx.write(
        &format!("workspace/life/people/{slug}.md"),
        &format!("---\nprivate: true\nname: {slug}\ntype: person\ntags: [{tags}]\n---\n# {slug}\n{body}\n"),
    );
}

/// A vault whose pages are all in step with the registry.
fn vault() -> Fx {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    fx.mkdir("system");
    fx.write("workspace/life/tags.yaml", REGISTRY);
    fx.write("workspace/life/tags.md", "# Tags\n");
    place(&fx, "noodle-house", "restaurant", "harbour", "noodles, quiet, price-low", "Noodle house · quiet · [[harbour]] · want");
    place(&fx, "second-bowl", "restaurant", "harbour", "noodles, price-high", "Another noodle house · [[harbour]]");
    place(&fx, "corner-bakery", "cafe", "harbour", "pastry, quiet, price-low", "Bakery · [[harbour]] · [[sam-example]]");
    place(&fx, "harbour", "area", "", "", "Area · the harbour\n\n## Places\n- [[noodle-house]]\n- [[corner-bakery]]");
    person(&fx, "sam-example", "boardgames", "Likes board games; met at [[corner-bakery]]");
    fx
}

/// stdout with Windows line ends folded.
fn ok(fx: &Fx, args: &[&str]) -> String {
    fx.ok(args).replace("\r\n", "\n")
}

fn registry(fx: &Fx) -> afcore::tags::Registry {
    afcore::tags::Registry::load(&fx.ctx()).unwrap().unwrap()
}

fn tags_of(fx: &Fx, rel: &str) -> Vec<String> {
    afcore::fm::fm_list_values(&fx.fm(rel), "tags")
}

fn doctor(fx: &Fx) -> (Vec<String>, Vec<String>) {
    afcore::tags::doctor_checks(&fx.ctx()).unwrap()
}

// ------------------------------------------------------------------ match

#[test]
fn match_resolves_ids_alts_variants_and_merges_and_says_none_otherwise() {
    let fx = vault();
    assert!(ok(&fx, &["tag", "match", "Noodles"]).starts_with("af tag match: noodles · active · cuisine · via id\n"));
    assert!(fx.ok(&["tag", "match", "noodle bar"]).contains("noodles · active · cuisine · via alt label 'noodle-bar'"));
    assert!(fx.ok(&["tag", "match", "pastries"]).contains("pastry · active · cuisine · via variant of 'pastry'"));
    assert!(fx.ok(&["tag", "match", "calm"]).contains("quiet · active · vibe · via merged from 'calm'"));
    let out = fx.ok(&["tag", "match", "rooftop"]);
    assert!(out.contains("none usable for 'rooftop' · 'rooftop' is deprecated"), "{out}");
    let out = fx.ok(&["tag", "match", "Live Jazz", "--facet", "vibe"]);
    assert!(out.starts_with("af tag match: none for 'Live Jazz' · if it is genuinely new: af tag propose live-jazz --facet <vibe>"), "{out}");
    // a facet narrows the match
    assert!(fx.ok(&["tag", "match", "noodles", "--facet", "vibe"]).contains("none for 'noodles'"));
    let err = fx.refused(&["tag", "match", "x", "--facet", "mood"]);
    assert!(err.contains("no facet 'mood'"), "{err}");
    // a suggestion pass with no model degrades to a note, never a match
    let out = fx.af().env("AF_OLLAMA_URL", "http://127.0.0.1:9/api/embed").args(["tag", "match", "ramen", "--suggest"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    assert!(text.contains("none for 'ramen'") && text.contains("suggestions unavailable"), "{text}");
}

#[test]
fn without_a_registry_the_verbs_name_init() {
    let fx = Fx::new();
    let err = fx.refused(&["tag", "match", "x"]);
    assert!(err.contains("workspace/life/tags.yaml does not exist; af tag init"), "{err}");
    let out = fx.ok(&["tag", "init"]);
    assert!(out.contains("with 12 facets and no tags"), "{out}");
    assert!(fx.isfile("workspace/life/tags.yaml") && fx.isfile("workspace/life/tags.md") && fx.isfile("workspace/life/tags-log.md"));
    assert!(fx.refused(&["tag", "init"]).contains("already exists"));
    assert!(fx.ok(&["tag", "match", "x"]).contains("none for 'x'"));
}

// ------------------------------------------------------------------ lifecycle

#[test]
fn propose_promote_deprecate_and_drop_move_a_tag_through_its_life_and_log_it() {
    let fx = vault();
    let out = fx.ok(&["tag", "propose", "ramen", "--facet", "cuisine", "--scope", "Ramen shops", "--alt", "Ramen Bar, ramen-ya"]);
    assert!(out.starts_with("af tag propose: ramen · proposed · cuisine"), "{out}");
    let t = registry(&fx).get("ramen").cloned().unwrap();
    assert_eq!((t.status.as_str(), t.facet.as_str(), t.scope.as_str()), ("proposed", "cuisine", "Ramen shops"));
    assert_eq!(t.alt, vec!["ramen-bar", "ramen-ya"]);
    assert_eq!(t.since.as_deref(), Some(today().as_str()));
    assert!(fx.ok(&["tag", "match", "ramen ya"]).contains("ramen · proposed · cuisine · via alt label 'ramen-ya'"));

    assert!(fx.refused(&["tag", "propose", "ramen", "--facet", "cuisine"]).contains("already exists (proposed, cuisine)"));
    assert!(fx.refused(&["tag", "propose", "noodle", "--facet", "cuisine"]).contains("is an alt label of 'noodles'"));
    assert!(fx.refused(&["tag", "propose", "Bad_Slug", "--facet", "cuisine"]).contains("not a tag slug"));
    assert!(fx.refused(&["tag", "propose", "x", "--facet", "mood"]).contains("no facet 'mood'"));
    assert!(fx.refused(&["tag", "propose", "y", "--facet", "vibe", "--alt", "hushed"]).contains("alt label 'hushed' already belongs to 'quiet'"));

    assert!(fx.ok(&["tag", "promote", "ramen"]).contains("ramen · active"));
    assert_eq!(registry(&fx).get("ramen").unwrap().status, "active");
    assert!(fx.refused(&["tag", "promote", "ramen"]).contains("is active, only a proposed tag is promoted"));
    assert!(fx.refused(&["tag", "promote", "nothing"]).contains("no tag 'nothing'"));

    let out = fx.ok(&["tag", "deprecate", "quiet", "--reason", "folded into the card line"]);
    assert!(out.contains("quiet · deprecated") && out.contains("still on 2 page(s): workspace/life/places/corner-bakery.md, workspace/life/places/noodle-house.md"), "{out}");
    assert!(fx.refused(&["tag", "deprecate", "quiet"]).contains("already deprecated"));

    assert!(fx.refused(&["tag", "drop", "noodles"]).contains("is active; only a proposed tag is dropped"));
    assert!(fx.ok(&["tag", "drop", "fusion"]).contains("fusion removed"));
    assert!(registry(&fx).get("fusion").is_none());

    let log = fx.read("workspace/life/tags-log.md");
    let t = today();
    for line in [
        format!("- {t} · propose · ramen · cuisine · Ramen shops"),
        format!("- {t} · promote · ramen"),
        format!("- {t} · deprecate · quiet · folded into the card line"),
        format!("- {t} · drop · fusion · proposed, unused"),
    ] {
        assert!(log.contains(&line), "{log}");
    }
    // the rendered view follows every change
    let view = fx.read("workspace/life/tags.md");
    assert!(view.contains("- **cuisine**: noodles, pastry, ramen"), "{view}");
    assert!(view.contains("quiet (deprecated)") && view.contains("calm -> quiet"), "{view}");
}

#[test]
fn merge_rewrites_every_page_records_the_alt_label_and_is_idempotent() {
    let fx = vault();
    let out = ok(&fx, &["tag", "merge", "pastry", "--into", "noodles"]);
    assert!(out.starts_with("af tag merge: pastry (tag) -> noodles · 1 page(s) rewritten\n  workspace/life/places/corner-bakery.md\n"), "{out}");
    assert_eq!(tags_of(&fx, "workspace/life/places/corner-bakery.md"), vec!["noodles", "quiet", "price-low"]);
    let reg = registry(&fx);
    let pastry = reg.get("pastry").unwrap();
    assert_eq!((pastry.status.as_str(), pastry.merged_into.as_deref()), ("merged", Some("noodles")));
    assert_eq!(reg.get("noodles").unwrap().alt, vec!["noodle", "noodle-bar", "pastry"]);
    assert!(fx.ok(&["tag", "match", "pastry"]).contains("noodles · active · cuisine · via merged from 'pastry'"));
    // the other lines of the page are untouched
    let fm = fx.fm("workspace/life/places/corner-bakery.md");
    assert!(fm.contains("kind: cafe\narea: harbour\naddress: 1 Example St\ntags: [noodles, quiet, price-low]\nstatus: want"), "{fm}");

    // a page written later with the old tag: rerun is the fix, and it is idempotent
    place(&fx, "late-page", "cafe", "harbour", "pastry", "x");
    let out = ok(&fx, &["tag", "merge", "pastry", "--into", "noodles"]);
    assert!(out.contains("1 page(s) rewritten\n  workspace/life/places/late-page.md"), "{out}");
    assert_eq!(registry(&fx).get("noodles").unwrap().alt, vec!["noodle", "noodle-bar", "pastry"]);

    assert!(fx.refused(&["tag", "merge", "pastry", "--into", "quiet"]).contains("already merged into 'noodles'"));
    assert!(fx.refused(&["tag", "merge", "noodles", "--into", "calm"]).contains("is merged; merge into a live tag"));
    assert!(fx.refused(&["tag", "merge", "noodles", "--into", "noodle"]).contains("'noodle' is an alt label of 'noodles'"));
    assert!(fx.refused(&["tag", "merge", "ghost", "--into", "noodles"]).contains("neither a tag nor on any page"));
    assert!(fx.refused(&["tag", "merge", "noodles", "--into", "nothing"]).contains("no tag 'nothing'"));
}

/// The 2026-10-30 drift: pages carried variants the registry already named (a plural, a
/// shortened form) and a label nothing owned. `merge` from a bare label rewrites the pages
/// and records the label; the doctor had to show each one first.
#[test]
fn merge_from_a_bare_label_fixes_the_drift_the_doctor_reports() {
    let fx = vault();
    place(&fx, "drifted", "bar", "harbour", "noodle, cheap, groups", "x");
    let (issues, _) = doctor(&fx);
    assert!(any_contains(&issues, "workspace/life/places/drifted.md: tag 'noodle' is an alt label of 'noodles'; af tag merge noodle --into noodles"), "{issues:?}");
    assert!(any_contains(&issues, "tag 'cheap' is an alt label of 'price-low'; af tag merge cheap --into price-low"), "{issues:?}");
    assert!(any_contains(&issues, "tag 'groups' is not in workspace/life/tags.yaml; af tag match groups, then af tag merge groups --into <id> or af tag propose groups"), "{issues:?}");
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("tag 'groups' is not in workspace/life/tags.yaml"), "{out}");

    assert!(fx.ok(&["tag", "merge", "noodle", "--into", "noodles"]).contains("noodle (alt label) -> noodles · 1 page(s) rewritten"));
    assert!(fx.ok(&["tag", "merge", "cheap", "--into", "price-low"]).contains("cheap (alt label) -> price-low · 1 page(s)"));
    fx.ok(&["tag", "propose", "group", "--facet", "vibe", "--scope", "Takes a table of eight"]);
    assert!(fx.ok(&["tag", "merge", "groups", "--into", "group"]).contains("groups (label) -> group · 1 page(s) rewritten"));
    assert_eq!(tags_of(&fx, "workspace/life/places/drifted.md"), vec!["noodles", "price-low", "group"]);
    assert_eq!(registry(&fx).get("group").unwrap().alt, vec!["groups"]);
    let (issues, _) = doctor(&fx);
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn rename_moves_the_id_and_keeps_the_old_one_as_an_alt() {
    let fx = vault();
    let out = fx.ok(&["tag", "rename", "quiet", "serene"]);
    assert!(out.starts_with("af tag rename: quiet -> serene · active · 2 page(s) rewritten; quiet stays an alt label"), "{out}");
    assert_eq!(tags_of(&fx, "workspace/life/places/noodle-house.md"), vec!["noodles", "serene", "price-low"]);
    let reg = registry(&fx);
    let t = reg.get("serene").unwrap();
    assert_eq!((t.status.as_str(), t.facet.as_str(), t.scope.as_str()), ("active", "vibe", "You can hear the table"));
    assert_eq!(t.alt, vec!["hushed", "quiet"]);
    assert_eq!(reg.get("quiet").unwrap().merged_into.as_deref(), Some("serene"));
    assert!(fx.ok(&["tag", "match", "calm"]).contains("serene · active · vibe · via merged from 'quiet'"));
    assert!(fx.refused(&["tag", "rename", "noodles", "pastry"]).contains("already exists; af tag merge noodles --into pastry"));
    assert!(fx.refused(&["tag", "rename", "noodles", "Up"]).contains("not a tag slug"));
}

#[test]
fn add_writes_validated_tags_and_refuses_what_the_registry_refuses() {
    let fx = vault();
    let out = ok(&fx, &["tag", "add", "workspace/life/places/second-bowl.md", "hushed", "noodles"]);
    assert!(out.starts_with("af tag add: workspace/life/places/second-bowl.md · +1 · tags: [noodles, price-high, quiet]\n  'hushed' written as quiet (alt label 'hushed')"), "{out}");
    assert!(fx.refused(&["tag", "add", "workspace/life/places/second-bowl.md", "price-low"]).contains("already carries 'price-high' in the single-valued facet 'price'"));
    assert!(fx.refused(&["tag", "add", "workspace/life/places/second-bowl.md", "rooftop"]).contains("is deprecated"));
    assert!(fx.refused(&["tag", "add", "workspace/life/places/second-bowl.md", "Live Jazz"]).contains("af tag propose live-jazz --facet <f>"));
    assert!(fx.refused(&["tag", "add", "workspace/life/places/second-bowl.md", "boardgames"]).contains("a people facet; this is a places page"));
    assert!(fx.refused(&["tag", "add", "workspace/life/knowledge/x.md", "noodles"]).contains("is not a page under workspace/life/places"));
    // an area page with an empty tags field gets its first tag; the cap holds
    fx.ok(&["tag", "add", "workspace/life/places/harbour.md", "quiet"]);
    assert_eq!(tags_of(&fx, "workspace/life/places/harbour.md"), vec!["quiet"]);
    for i in 0..8 {
        fx.ok(&["tag", "propose", &format!("t{i}"), "--facet", "vibe"]);
    }
    let many: Vec<String> = (0..8).map(|i| format!("t{i}")).collect();
    let mut args = vec!["tag", "add", "workspace/life/places/harbour.md"];
    args.extend(many.iter().map(|s| s.as_str()));
    assert!(fx.refused(&args).contains("would carry 9 tags, over the cap of 8"));
    assert_eq!(tags_of(&fx, "workspace/life/places/harbour.md"), vec!["quiet"], "a refusal writes nothing");
}

// ------------------------------------------------------------------ lint, stats, list

#[test]
fn lint_reports_each_class_and_fix_applies_only_promotions_and_drops() {
    let fx = vault();
    // a proposed tag at the promotion threshold, one stale, one single-facet clash, an under-tagged venue
    fx.ok(&["tag", "propose", "patio", "--facet", "vibe", "--scope", "Outdoor seats"]);
    for (slug, tags) in [("a", "patio, noodles, price-low"), ("b", "patio, noodles, price-high"), ("c", "patio, noodles, price-low, price-high")] {
        place(&fx, slug, "bar", "harbour", tags, "x");
    }
    place(&fx, "thin", "club", "harbour", "noodles", "x");
    person(&fx, "untagged", "", "x");
    fx.ok(&["tag", "propose", "bare", "--facet", "vibe"]);
    fx.ok(&["tag", "promote", "bare"]);
    let out = ok(&fx, &["tag", "lint"]);
    for want in [
        "  issue · workspace/life/places/c.md: two 'price' tags (price-low, price-high); the facet takes one per page",
        "  promote · 'patio' (proposed) is on 3 pages; af tag promote patio",
        "  drop · 'fusion' (proposed ",
        " days, no page); af tag drop fusion",
        "  scope · 'bare' has no scope note",
        "  review · 'boardgames' (active, ",
        " days) is on 1 page(s); keep it if the facet value is real, else af tag deprecate boardgames",
        "  under-tagged · workspace/life/places/thin.md (club) has 1 tag(s), floor 3",
        "  under-tagged · workspace/life/people/untagged.md has no tags",
    ] {
        assert!(out.contains(want), "missing {want:?} in\n{out}");
    }
    assert!(!out.contains("scope · 'noodles'"), "a scoped tag is not listed: {out}");
    assert!(!out.contains("under-tagged · workspace/life/places/harbour.md"), "areas are exempt: {out}");

    let out = ok(&fx, &["tag", "lint", "--fix"]);
    assert!(out.contains("applied 2 mechanical item(s); merges, splits and deprecations stay yours"), "{out}");
    let reg = registry(&fx);
    assert_eq!(reg.get("patio").unwrap().status, "active");
    assert!(reg.get("fusion").is_none());
    assert!(reg.get("price-low").unwrap().status == "active", "a facet clash is never auto-fixed");
}

#[test]
fn lint_finds_variant_pairs_and_always_together_pairs_and_overloads() {
    let fx = vault();
    fx.ok(&["tag", "propose", "noodle-bars", "--facet", "cuisine"]);
    fx.ok(&["tag", "propose", "patios", "--facet", "vibe"]);
    fx.ok(&["tag", "propose", "patio", "--facet", "vibe"]);
    fx.ok(&["tag", "propose", "counter", "--facet", "vibe"]);
    place(&fx, "corner-bakery", "cafe", "harbour", "pastry, counter, quiet, price-low", "x");
    for i in 0..3 {
        place(&fx, &format!("p{i}"), if i % 2 == 0 { "bar" } else { "club" }, "harbour", "pastry, counter, price-low", "x");
    }
    for i in 0..3 {
        place(&fx, &format!("q{i}"), "restaurant", "harbour", "price-low", "x");
    }
    let out = ok(&fx, &["tag", "lint"]);
    assert!(out.contains("near-duplicate · 'patio' and 'patios' are singular/plural or hyphen variants"), "{out}");
    assert!(out.contains("near-duplicate · 'counter' and 'pastry' always appear together (4 pages)"), "{out}");
    assert!(out.contains("overload · 'price-low' spans 4 kinds (bar 2, cafe 1, club 1, restaurant 4)"), "{out}");
    assert!(!out.contains("'noodle-bars' are singular"), "an alt label is not a tag pair: {out}");
    let clean = Fx::new();
    clean.write("workspace/life/tags.yaml", "facets: []\ntags: []\n");
    assert_eq!(ok(&clean, &["tag", "lint"]), "af tag lint: clean · 0 tags · 0 pages\n");
}

#[test]
fn stats_and_list_count_uses_from_the_pages() {
    let fx = vault();
    let out = ok(&fx, &["tag", "stats"]);
    assert!(out.starts_with("af tag stats: 9 tags · 6 active · 1 proposed · 1 deprecated · 1 merged · 5 pages (4 places, 1 people)\n"), "{out}");
    assert!(out.contains("  cuisine (places): fusion? 0 · noodles 2 · pastry 1\n"), "{out}");
    assert!(out.contains("  vibe (places): quiet 2 · rooftop! 0\n"), "{out}");
    assert!(out.contains("  singleton share: 3/6 active tags"), "{out}");
    assert!(out.contains("  venue pages with a tag in: cuisine 3/3 · vibe 2/3 · price 3/3"), "{out}");
    let out = ok(&fx, &["tag", "list", "--facet", "cuisine"]);
    assert!(out.starts_with("cuisine (places, multi):\n  fusion · proposed · 0 use(s)\n  noodles · active · 2 use(s) · alt: noodle, noodle-bar · Noodle houses of any tradition\n"), "{out}");
    assert!(fx.ok(&["tag", "list", "--status", "merged"]).contains("calm · merged"));
}

// ------------------------------------------------------------------ doctor

#[test]
fn a_clean_vault_is_quiet_and_each_break_names_its_fix() {
    let fx = vault();
    assert_eq!(doctor(&fx), (vec![], vec![]));
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}");

    place(&fx, "bad", "bar", "harbour", "calm, rooftop, noodles, price-low, price-high", "see [[nowhere]] and [[harbour|the harbour]] and [[sam-example#notes]]; `[[in-code]]` is skipped\n\n```\n[[fenced]]\n```");
    let (issues, notes) = doctor(&fx);
    assert!(any_contains(&issues, "workspace/life/places/bad.md: tag 'calm' was merged into 'quiet'; rerun af tag merge calm --into quiet"), "{issues:?}");
    assert!(any_contains(&issues, "bad.md: two 'price' tags (price-low, price-high)"), "{issues:?}");
    assert!(any_contains(&issues, "workspace/life/places/bad.md: [[nowhere]] has no page under workspace/life/places/ or people/"), "{issues:?}");
    assert_eq!(issues.len(), 3, "{issues:?}");
    assert!(any_contains(&notes, "bad.md: tag 'rooftop' is deprecated; remove it or af tag merge rooftop --into <replacement>"), "{notes:?}");

    // a knowledge page's dangling link counts; sources and the log do not
    fx.write("workspace/life/knowledge/city.md", "# City\n\n[[noodle-house]] and [[gone-place]]\n");
    fx.write("workspace/life/sources/research/x.md", "[[not-checked]]\n");
    fx.write("workspace/life/log/2026-01.md", "[[not-checked-either]]\n");
    let (issues, _) = doctor(&fx);
    assert!(any_contains(&issues, "workspace/life/knowledge/city.md: [[gone-place]] has no page"), "{issues:?}");
    assert!(!any_contains(&issues, "not-checked"), "{issues:?}");
}

#[test]
fn a_stale_view_a_missing_registry_and_a_broken_one_are_reported() {
    let fx = vault();
    fx.write("workspace/life/tags.md", "old view\n");
    std::thread::sleep(std::time::Duration::from_millis(30));
    fx.write("workspace/life/tags.yaml", REGISTRY);
    let (_, notes) = doctor(&fx);
    assert!(any_contains(&notes, "workspace/life/tags.md is older than workspace/life/tags.yaml; af tag render"), "{notes:?}");
    fx.ok(&["tag", "render"]);
    assert!(doctor(&fx).1.is_empty());

    fx.remove("workspace/life/tags.yaml");
    let (issues, notes) = doctor(&fx);
    assert!(issues.is_empty(), "{issues:?}");
    assert!(any_contains(&notes, "workspace/life/tags.yaml missing while 4 page(s) carry tags; af tag init"), "{notes:?}");

    fx.write("workspace/life/tags.yaml", "facets:\n  - name: vibe\ntags:\n  - id: quiet\n    facet: mood\n");
    let (issues, _) = doctor(&fx);
    assert!(any_contains(&issues, "workspace/life/tags.yaml does not parse"), "{issues:?}");
    let err = fx.refused(&["tag", "match", "x"]);
    assert!(err.contains("tag 'quiet' names facet 'mood', which is not declared under facets"), "{err}");
}

// ------------------------------------------------------------------ list forms

/// `tags:` on a hand-written page may be a block list (indented or not) or a flow list
/// wrapped over lines. A write replaces the whole field; it never leaves orphan item lines.
#[test]
fn add_rewrites_block_and_wrapped_tag_lists_as_one_field() {
    let fx = vault();
    let forms = [
        ("block-indented", "tags:\n  - noodles\n  - quiet\n"),
        ("block-flush", "tags:\n- noodles\n- quiet\n"),
        ("flow-wrapped", "tags: [noodles,\n  quiet]\n"),
        ("flow-wrapped-each", "tags: [\n  noodles,\n  quiet,\n]\n"),
    ];
    for (slug, field) in forms {
        let rel = format!("workspace/life/places/{slug}.md");
        fx.write(&rel, &format!("---\nprivate: true\nkind: restaurant\narea: harbour\n{field}status: want\n---\n# {slug}\nRestaurant · noodles · quiet\n"));
        assert_eq!(tags_of(&fx, &rel), vec!["noodles", "quiet"], "{slug}: read");
        ok(&fx, &["tag", "add", &rel, "price-low"]);
        assert_eq!(fx.fm(&rel), "private: true\nkind: restaurant\narea: harbour\ntags: [noodles, quiet, price-low]\nstatus: want", "{slug}: write");
        assert_eq!(fx.body(&rel), format!("# {slug}\nRestaurant · noodles · quiet\n"), "{slug}: body");
    }
    // a field this parser cannot own is refused, not clobbered
    let rel = "workspace/life/places/odd.md";
    fx.write(rel, "---\nprivate: true\nkind: restaurant\narea: harbour\ntags: [noodles]\n  - quiet\nstatus: want\n---\n# odd\n");
    let err = fx.refused(&["tag", "add", rel, "price-low"]);
    assert!(err.contains("tags: field is followed by an indented line"), "{err}");
    assert!(fx.read(rel).contains("tags: [noodles]\n  - quiet\n"));
}

#[test]
fn merge_stays_inside_one_facet() {
    let fx = vault();
    let err = fx.refused(&["tag", "merge", "noodles", "--into", "quiet"]);
    assert!(err.contains("'noodles' is in facet 'cuisine' and 'quiet' in facet 'vibe'; a merge stays inside one facet"), "{err}");
    assert_eq!(registry(&fx).get("noodles").unwrap().status, "active");
    assert_eq!(tags_of(&fx, "workspace/life/places/noodle-house.md"), vec!["noodles", "quiet", "price-low"]);
}

// ------------------------------------------------------------------ topic tags: knowledge pages and sources

/// The vault plus the `topic` facet (through the verb under test) and four active topics.
fn topic_vault() -> Fx {
    let fx = vault();
    fx.ok(&["tag", "facet", "add", "topic", "--applies", "context"]);
    for (slug, scope) in [("leases", "Rentals and leases"), ("health", "Health care and records"), ("travel", "Trips and bookings"), ("money", "Spending and bills")] {
        fx.ok(&["tag", "propose", slug, "--facet", "topic", "--scope", scope]);
        fx.ok(&["tag", "promote", slug]);
    }
    fx
}

const FM: &str = "id: src-2026-11-01-01\ntype: note\ndate: 2026-11-01\nverbatim: true\n";

fn knowledge_page(fx: &Fx, slug: &str, tags: Option<&str>) -> String {
    let rel = format!("workspace/life/knowledge/{slug}.md");
    let head = tags.map(|t| format!("---\nprivate: true\ntags: [{t}]\n---\n")).unwrap_or_default();
    fx.write(&rel, &format!("{head}# {slug}\n\n## Current\n\n- A fact (as of 2026-11-01) [src-2026-11-01-01]\n\n## Timeline\n\n- 2026-11-01 · filed [src-2026-11-01-01]\n"));
    rel
}

fn source_file(fx: &Fx, name: &str, tags: Option<&str>) -> String {
    let rel = format!("workspace/life/sources/{name}.md");
    let line = tags.map(|t| format!("tags: [{t}]\n")).unwrap_or_default();
    fx.write(&rel, &format!("---\n{FM}{line}---\nthe body, kept as it came\n"));
    rel
}

fn bytes(fx: &Fx, rel: &str) -> Vec<u8> {
    std::fs::read(fx.path(rel)).unwrap()
}

#[test]
fn the_starter_registry_has_a_topic_facet_for_knowledge_pages_and_sources() {
    let fx = Fx::new();
    fx.ok(&["tag", "init"]);
    let f = registry(&fx).facet("topic").cloned().unwrap();
    assert_eq!((f.applies.as_str(), f.single), ("context", false));
    assert!(fx.read("workspace/life/tags.md").replace("\r\n", "\n").contains("## Knowledge and sources\n\n- **topic**: \n"));
}

#[test]
fn facet_add_declares_a_facet_logs_it_and_renders_it_and_refuses_bad_input() {
    let fx = vault();
    let out = ok(&fx, &["tag", "facet", "add", "topic", "--applies", "context"]);
    assert_eq!(out, "af tag facet add: topic · context · multi\n");
    ok(&fx, &["tag", "facet", "add", "depth", "--applies", "knowledge", "--single"]);
    ok(&fx, &["tag", "facet", "add", "origin", "--applies", "sources"]);
    let reg = registry(&fx);
    let got: Vec<(String, String, bool)> = reg.facets.iter().skip(4).map(|f| (f.name.clone(), f.applies.clone(), f.single)).collect();
    assert_eq!(
        got,
        vec![("topic".into(), "context".into(), false), ("depth".into(), "knowledge".into(), true), ("origin".into(), "sources".into(), false)]
    );
    // the facets that were there are untouched
    assert_eq!(reg.tags.len(), 9);
    let log = fx.read("workspace/life/tags-log.md");
    assert!(log.contains(&format!("- {} · facet · topic · context · multi", today())), "{log}");
    assert!(log.contains(&format!("- {} · facet · depth · knowledge · single", today())), "{log}");
    let view = fx.read("workspace/life/tags.md").replace("\r\n", "\n");
    assert!(view.contains("## Knowledge and sources\n\n- **topic**: \n- **depth** (knowledge pages only) (one per page): \n- **origin** (sources only): \n"), "{view}");
    assert!(view.contains("## Places\n") && view.contains("## People\n"), "{view}");
    assert!(view.contains("one to 3 on a knowledge page or source"), "{view}");
    // a registry with no such facet has no such section
    let plain = vault();
    assert!(!plain.read("workspace/life/tags.md").contains("Knowledge and sources"));
    plain.ok(&["tag", "render"]);
    assert!(!plain.read("workspace/life/tags.md").contains("Knowledge and sources"));

    assert!(fx.refused(&["tag", "facet", "add", "topic", "--applies", "context"]).contains("facet 'topic' already exists (context, multi)"));
    assert!(fx.refused(&["tag", "facet", "add", "vibe", "--applies", "sources"]).contains("facet 'vibe' already exists (places, multi)"));
    assert!(fx.refused(&["tag", "facet", "add", "x", "--applies", "pages"]).contains("--applies 'pages' is not one of places, people, both, knowledge, sources, context"));
    assert!(fx.refused(&["tag", "facet", "add", "Bad_Name", "--applies", "context"]).contains("not a facet name"));
    assert_eq!(registry(&fx).facets.len(), 7, "a refusal adds nothing");
    let none = Fx::new();
    assert!(none.refused(&["tag", "facet", "add", "topic", "--applies", "context"]).contains("af tag init"));
}

#[test]
fn a_registry_naming_a_value_applies_does_not_know_is_a_refusal() {
    let fx = vault();
    fx.write("workspace/life/tags.yaml", &REGISTRY.replace("applies: people", "applies: everyone"));
    let err = fx.refused(&["tag", "match", "x"]);
    assert!(err.contains("applies 'everyone' is not one of places, people, both, knowledge, sources, context"), "{err}");
}

#[test]
fn which_facets_apply_to_which_side() {
    for (applies, sides) in [
        ("places", ["places"].as_slice()),
        ("people", &["people"]),
        ("both", &["places", "people"]),
        ("knowledge", &["knowledge"]),
        ("sources", &["sources"]),
        ("context", &["knowledge", "sources"]),
    ] {
        for side in ["places", "people", "knowledge", "sources"] {
            assert_eq!(afcore::tags::facet_applies(applies, side), sides.contains(&side), "{applies} on {side}");
        }
    }
}

#[test]
fn add_tags_a_knowledge_page_and_a_source_under_the_facet_sides_and_a_cap_of_three() {
    let fx = topic_vault();
    let page = knowledge_page(&fx, "housing", None);
    let before = fx.read(&page);
    assert!(!before.starts_with("---"));
    let out = ok(&fx, &["tag", "add", &page, "leases", "Money"]);
    assert!(out.starts_with(&format!("af tag add: {page} · +2 · tags: [leases, money]\n")), "{out}");
    // a page that began at its heading gets a block of its own and keeps every byte below it
    assert_eq!(fx.read(&page), format!("---\ntags: [leases, money]\n---\n{before}"));
    assert_eq!(tags_of(&fx, &page), vec!["leases", "money"]);

    let src = source_file(&fx, "note-2026-11-01-lease", None);
    ok(&fx, &["tag", "add", &src, "leases"]);
    assert_eq!(tags_of(&fx, &src), vec!["leases"]);
    assert_eq!(fx.read(&src), format!("---\n{FM}tags: [leases]\n---\nthe body, kept as it came\n"));

    ok(&fx, &["tag", "add", &page, "health"]);
    assert_eq!(tags_of(&fx, &page), vec!["leases", "money", "health"]);
}

#[test]
fn a_place_facet_never_lands_on_a_knowledge_page_or_source_nor_a_topic_on_a_place() {
    let fx = topic_vault();
    let page = knowledge_page(&fx, "housing", None);
    let src = source_file(&fx, "note-2026-11-01-lease", None);
    for rel in [&page, &src] {
        let err = fx.refused(&["tag", "add", rel, "noodles"]);
        assert!(err.contains("tag 'noodles' is in facet 'cuisine', a places facet; this is a "), "{err}");
        let err = fx.refused(&["tag", "add", rel, "boardgames"]);
        assert!(err.contains("a people facet"), "{err}");
    }
    let place_err = fx.refused(&["tag", "add", "workspace/life/places/second-bowl.md", "leases"]);
    assert!(place_err.contains("tag 'leases' is in facet 'topic', a context facet; this is a places page"), "{place_err}");
    let person_err = fx.refused(&["tag", "add", "workspace/life/people/sam-example.md", "leases"]);
    assert!(person_err.contains("a context facet; this is a people page"), "{person_err}");
    // a knowledge-only facet reaches the page and not the source, a sources-only facet the reverse
    fx.ok(&["tag", "facet", "add", "depth", "--applies", "knowledge", "--single"]);
    fx.ok(&["tag", "facet", "add", "origin", "--applies", "sources"]);
    fx.ok(&["tag", "propose", "overview", "--facet", "depth"]);
    fx.ok(&["tag", "propose", "email", "--facet", "origin"]);
    fx.ok(&["tag", "add", &page, "overview"]);
    assert!(fx.refused(&["tag", "add", &src, "overview"]).contains("a knowledge facet; this is a sources page"));
    fx.ok(&["tag", "add", &src, "email"]);
    assert!(fx.refused(&["tag", "add", &page, "email"]).contains("a sources facet; this is a knowledge page"));
    // a refusal wrote nothing
    assert_eq!(tags_of(&fx, &page), vec!["overview"]);
    assert_eq!(tags_of(&fx, &src), vec!["email"]);
}

#[test]
fn the_cap_on_a_knowledge_page_or_source_is_three_and_a_places_page_keeps_eight() {
    let fx = topic_vault();
    fx.ok(&["tag", "propose", "extra", "--facet", "topic"]);
    let page = knowledge_page(&fx, "housing", Some("leases, health"));
    let src = source_file(&fx, "note-2026-11-01-lease", Some("leases, health, travel"));
    let err = fx.refused(&["tag", "add", &page, "travel", "money"]);
    assert!(err.contains(&format!("{page} would carry 4 tags, over the cap of 3")), "{err}");
    assert_eq!(tags_of(&fx, &page), vec!["leases", "health"], "a refusal writes nothing");
    let err = fx.refused(&["tag", "add", &src, "money"]);
    assert!(err.contains("would carry 4 tags, over the cap of 3"), "{err}");
    ok(&fx, &["tag", "add", &page, "travel"]);
    assert_eq!(tags_of(&fx, &page), vec!["leases", "health", "travel"]);
    // adding what is already there is no change and no refusal
    ok(&fx, &["tag", "add", &src, "leases"]);
    // the place cap is still eight
    for i in 0..4 {
        fx.ok(&["tag", "propose", &format!("t{i}"), "--facet", "vibe"]);
    }
    ok(&fx, &["tag", "add", "workspace/life/places/second-bowl.md", "t0", "t1", "t2", "t3", "quiet", "pastry"]);
    assert_eq!(tags_of(&fx, "workspace/life/places/second-bowl.md").len(), 8);
}

#[test]
fn a_source_outside_the_scan_is_refused_with_the_reason() {
    let fx = topic_vault();
    fx.write("workspace/life/sources/INDEX.md", "# Source Index\n");
    fx.write("workspace/life/sources/inbox/drop.md", &format!("---\n{FM}---\nx\n"));
    fx.write("workspace/life/sources/research/plain.md", "no frontmatter\n");
    assert!(fx.refused(&["tag", "add", "workspace/life/sources/INDEX.md", "leases"]).contains("is not a page under"));
    assert!(fx.refused(&["tag", "add", "workspace/life/sources/inbox/drop.md", "leases"]).contains("sources/inbox/, a staging folder"));
    let err = fx.refused(&["tag", "add", "workspace/life/sources/research/plain.md", "leases"]);
    assert!(err.contains("has no frontmatter block, so it carries no tags"), "{err}");
    assert_eq!(fx.read("workspace/life/sources/research/plain.md"), "no frontmatter\n");
    // the scan counts neither: stats sees no source at all
    assert!(ok(&fx, &["tag", "stats"]).contains("  sources with a topic: 0/0\n"));
}

/// The rule for a filed source: one field of the frontmatter may change and every other byte stays.
#[test]
fn a_source_keeps_every_byte_but_its_tags_field_whatever_its_endings_body_or_list_style() {
    let fx = topic_vault();
    let cases: Vec<(&str, Vec<u8>, Vec<u8>)> = vec![
        ("lf", b"---\nid: a\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntype: note\ntags: [leases]\n---\nbody\n".to_vec()),
        ("crlf", b"---\r\nid: a\r\ntype: note\r\n---\r\nline one\r\n\r\nline two\r\n".to_vec(), b"---\r\nid: a\r\ntype: note\r\ntags: [leases]\r\n---\r\nline one\r\n\r\nline two\r\n".to_vec()),
        ("trailing-blank-lines", b"---\nid: a\n---\nbody\n\n\n\n".to_vec(), b"---\nid: a\ntags: [leases]\n---\nbody\n\n\n\n".to_vec()),
        (
            "body-has-its-own-rule-lines",
            b"---\nid: a\n---\nintro\n---\ntags: [not, mine]\n---\n\ntail\n".to_vec(),
            b"---\nid: a\ntags: [leases]\n---\nintro\n---\ntags: [not, mine]\n---\n\ntail\n".to_vec(),
        ),
        ("no-final-newline", b"---\nid: a\n---\nbody with no end".to_vec(), b"---\nid: a\ntags: [leases]\n---\nbody with no end".to_vec()),
        ("closing-rule-ends-the-file", b"---\nid: a\n---".to_vec(), b"---\nid: a\ntags: [leases]\n---".to_vec()),
        ("empty-frontmatter", b"---\n---\nbody\n".to_vec(), b"---\ntags: [leases]\n---\nbody\n".to_vec()),
        ("inline-list-in-the-middle", b"---\nid: a\ntags: [health]\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntags: [health, leases]\ntype: note\n---\nbody\n".to_vec()),
        ("inline-list-double-quoted", b"---\nid: a\ntags: [\"health\"]\ntype: note\n---\nb\n".to_vec(), b"---\nid: a\ntags: [health, leases]\ntype: note\n---\nb\n".to_vec()),
        ("block-list-indented", b"---\nid: a\ntags:\n  - health\n  - travel\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntags: [health, travel, leases]\ntype: note\n---\nbody\n".to_vec()),
        ("block-list-flush", b"---\nid: a\ntags:\n- health\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntags: [health, leases]\ntype: note\n---\nbody\n".to_vec()),
        ("block-list-last-in-crlf", b"---\r\nid: a\r\ntags:\r\n  - health\r\n---\r\nbody\r\n".to_vec(), b"---\r\nid: a\r\ntags: [health, leases]\r\n---\r\nbody\r\n".to_vec()),
        ("flow-list-wrapped", b"---\nid: a\ntags: [health,\n  travel]\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntags: [health, travel, leases]\ntype: note\n---\nbody\n".to_vec()),
        ("empty-tags-field", b"---\nid: a\ntags:\ntype: note\n---\nbody\n".to_vec(), b"---\nid: a\ntags: [leases]\ntype: note\n---\nbody\n".to_vec()),
        ("bom", b"\xEF\xBB\xBF---\nid: a\n---\nbody\n".to_vec(), b"\xEF\xBB\xBF---\nid: a\ntags: [leases]\n---\nbody\n".to_vec()),
        ("mixed-endings", b"---\nid: a\n---\r\nbody\r\nmore\n".to_vec(), b"---\nid: a\ntags: [leases]\n---\r\nbody\r\nmore\n".to_vec()),
        ("closing-rule-with-trailing-space", b"---\nid: a\n--- \nbody\n".to_vec(), b"---\nid: a\ntags: [leases]\n--- \nbody\n".to_vec()),
        ("non-ascii-body", "---\nid: a\n---\ncafé · 日本語 · \u{1F600}\n".as_bytes().to_vec(), "---\nid: a\ntags: [leases]\n---\ncafé · 日本語 · \u{1F600}\n".as_bytes().to_vec()),
        ("a-tags-key-below-the-rule-is-body", b"---\nid: a\n---\ntags:\n  - body-list\n".to_vec(), b"---\nid: a\ntags: [leases]\n---\ntags:\n  - body-list\n".to_vec()),
    ];
    for (name, before, want) in cases {
        let rel = format!("workspace/life/sources/note-2026-11-01-{name}.md");
        fx.write_bytes(&rel, &before);
        let out = fx.run(&["tag", "add", &rel, "leases"]);
        assert_eq!(out.0, 0, "{name}: {}{}", out.1, out.2);
        let got = bytes(&fx, &rel);
        assert_eq!(String::from_utf8_lossy(&got), String::from_utf8_lossy(&want), "{name}");
        assert_eq!(got, want, "{name}");
        assert!(tags_of_raw(&got).contains(&"leases".to_string()), "{name}: the scan reads what was written");
    }
}

fn tags_of_raw(raw: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(raw).replace("\r\n", "\n");
    let fm = afcore::fm::split_fm(text.trim_start_matches('\u{feff}'), "x").unwrap().0;
    afcore::fm::fm_list_values(&fm, "tags")
}

#[test]
fn a_tags_field_that_cannot_be_owned_and_a_file_that_is_not_text_are_refused_and_left_alone() {
    let fx = topic_vault();
    let stray = b"---\nid: a\ntags: [health]\n  - travel\n---\nbody\n".to_vec();
    let rel = "workspace/life/sources/note-2026-11-01-stray.md";
    fx.write_bytes(rel, &stray);
    let err = fx.refused(&["tag", "add", rel, "leases"]);
    assert!(err.contains("the tags: field is followed by an indented line that is not part of it"), "{err}");
    assert_eq!(bytes(&fx, rel), stray);
    assert!(!err.contains("body"), "no refusal prints a source's body: {err}");

    let mut binary = b"---\nid: a\n---\nbody ".to_vec();
    binary.extend_from_slice(&[0xFF, 0xFE, 0x80, b'\n']);
    let rel = "workspace/life/sources/note-2026-11-01-binary.md";
    fx.write_bytes(rel, &binary);
    let err = fx.refused(&["tag", "add", rel, "leases"]);
    assert!(err.contains("is not UTF-8 text; nothing was written"), "{err}");
    assert_eq!(bytes(&fx, rel), binary);
    // reading such a source never stops the weekly pass or the doctor
    fx.ok(&["tag", "stats"]);
    fx.ok(&["tag", "lint"]);
}

#[test]
fn a_file_without_the_closing_rule_is_not_a_source_with_frontmatter() {
    let fx = topic_vault();
    let rel = "workspace/life/sources/note-2026-11-01-open.md";
    fx.write(rel, "---\nid: a\nnever closed\nbody\n");
    assert!(fx.refused(&["tag", "add", rel, "leases"]).contains("has no frontmatter block, so it carries no tags"));
    assert_eq!(fx.read(rel), "---\nid: a\nnever closed\nbody\n");
}

#[test]
fn a_knowledge_page_written_with_crlf_and_no_frontmatter_gets_a_crlf_block_and_keeps_the_rest() {
    let fx = topic_vault();
    let rel = "workspace/life/knowledge/crlf.md";
    fx.write_bytes(rel, b"# Crlf\r\n\r\n## Current\r\n\r\n- fact\r\n");
    ok(&fx, &["tag", "add", rel, "money"]);
    assert_eq!(bytes(&fx, rel), b"---\r\ntags: [money]\r\n---\r\n# Crlf\r\n\r\n## Current\r\n\r\n- fact\r\n".to_vec());
    let rel = "workspace/life/knowledge/bom.md";
    fx.write_bytes(rel, b"\xEF\xBB\xBF# Bom\n");
    ok(&fx, &["tag", "add", rel, "money"]);
    assert_eq!(bytes(&fx, rel), b"\xEF\xBB\xBF---\ntags: [money]\n---\n# Bom\n".to_vec());
    // a page that has frontmatter keeps its other fields, and a body line that looks like a rule
    let rel = "workspace/life/knowledge/fm.md";
    fx.write_bytes(rel, b"---\nprivate: true\n---\n# Fm\n---\nnot frontmatter\n");
    ok(&fx, &["tag", "add", rel, "money"]);
    assert_eq!(bytes(&fx, rel), b"---\nprivate: true\ntags: [money]\n---\n# Fm\n---\nnot frontmatter\n".to_vec());
}

#[test]
fn merge_and_rename_rewrite_knowledge_pages_and_sources_under_the_same_byte_rule() {
    let fx = topic_vault();
    fx.ok(&["tag", "propose", "rentals", "--facet", "topic"]);
    let page = "workspace/life/knowledge/housing.md";
    fx.write_bytes(page, b"---\r\nprivate: true\r\ntags: [leases, health]\r\n---\r\n# Housing\r\n\r\n- fact\r\n");
    let crlf_src = "workspace/life/sources/note-2026-11-01-crlf.md";
    fx.write_bytes(crlf_src, b"---\r\nid: a\r\ntags:\r\n  - leases\r\n  - money\r\n---\r\nbody\r\n\r\n\r\n");
    let lf_src = "workspace/life/sources/note-2026-11-01-lf.md";
    fx.write_bytes(lf_src, b"---\nid: b\ntags: [travel]\n---\nbody\n---\nend");
    let other = "workspace/life/sources/note-2026-11-01-other.md";
    fx.write_bytes(other, b"---\nid: c\ntags: [money]\n---\nuntouched\n");
    let untouched = bytes(&fx, other);

    let out = ok(&fx, &["tag", "merge", "leases", "--into", "rentals"]);
    assert!(out.starts_with("af tag merge: leases (tag) -> rentals · 2 page(s) rewritten\n"), "{out}");
    assert_eq!(bytes(&fx, page), b"---\r\nprivate: true\r\ntags: [rentals, health]\r\n---\r\n# Housing\r\n\r\n- fact\r\n".to_vec());
    assert_eq!(bytes(&fx, crlf_src), b"---\r\nid: a\r\ntags: [rentals, money]\r\n---\r\nbody\r\n\r\n\r\n".to_vec());
    assert_eq!(bytes(&fx, other), untouched);

    let out = ok(&fx, &["tag", "rename", "travel", "trips"]);
    assert!(out.contains("1 page(s) rewritten"), "{out}");
    assert_eq!(bytes(&fx, lf_src), b"---\nid: b\ntags: [trips]\n---\nbody\n---\nend".to_vec());
    assert_eq!(bytes(&fx, other), untouched);
    // the search side agrees
    assert!(ok(&fx, &["tag", "list", "--facet", "topic"]).contains("rentals · proposed · 2 use(s)"));
}

#[test]
fn a_merge_that_cannot_rewrite_one_page_writes_none() {
    let fx = topic_vault();
    fx.ok(&["tag", "propose", "rentals", "--facet", "topic"]);
    let good = "workspace/life/knowledge/a-good.md";
    let bad = "workspace/life/sources/note-2026-11-01-zbad.md";
    fx.write_bytes(good, b"---\ntags: [leases]\n---\n# Good\n");
    fx.write_bytes(bad, b"---\nid: a\ntags: [leases]\n  - odd\n---\nbody\n");
    let (good_before, bad_before) = (bytes(&fx, good), bytes(&fx, bad));
    let reg_before = fx.read("workspace/life/tags.yaml");
    let err = fx.refused(&["tag", "merge", "leases", "--into", "rentals"]);
    assert!(err.contains("the tags: field is followed by an indented line"), "{err}");
    assert_eq!((bytes(&fx, good), bytes(&fx, bad)), (good_before, bad_before), "no page was written");
    assert_eq!(fx.read("workspace/life/tags.yaml"), reg_before, "the registry did not move");
}

#[test]
fn lint_asks_a_knowledge_page_for_a_topic_and_never_a_source() {
    let fx = topic_vault();
    knowledge_page(&fx, "bare", None);
    knowledge_page(&fx, "no-topic-tag", Some(""));
    knowledge_page(&fx, "topical", Some("leases"));
    source_file(&fx, "note-2026-11-01-untagged", None);
    source_file(&fx, "note-2026-11-01-tagged", Some("leases"));
    let out = ok(&fx, &["tag", "lint"]);
    assert!(out.contains("  under-tagged · workspace/life/knowledge/bare.md has no topic tag; read it and af tag add one to 3 (af tag match \"<phrase>\" --facet topic)\n"), "{out}");
    assert!(out.contains("  under-tagged · workspace/life/knowledge/no-topic-tag.md has no topic tag"), "{out}");
    assert!(!out.contains("knowledge/topical.md"), "{out}");
    assert!(!out.contains("sources/"), "a source with no tag is not a finding: {out}");
    // a knowledge page whose only tag is not a topic still wants one
    fx.ok(&["tag", "facet", "add", "depth", "--applies", "knowledge"]);
    fx.ok(&["tag", "propose", "overview", "--facet", "depth"]);
    fx.ok(&["tag", "add", "workspace/life/knowledge/bare.md", "overview"]);
    assert!(ok(&fx, &["tag", "lint"]).contains("knowledge/bare.md has no topic tag"));
    fx.ok(&["tag", "add", "workspace/life/knowledge/bare.md", "money"]);
    assert!(!ok(&fx, &["tag", "lint"]).contains("knowledge/bare.md"));
    // the doctor does not carry the finding: an untagged page is a lint item, not a fault
    let (issues, notes) = doctor(&fx);
    assert!(!any_contains(&issues, "no topic tag") && !any_contains(&notes, "no topic tag"), "{issues:?} {notes:?}");
}

#[test]
fn without_a_topic_facet_lint_says_so_once_instead_of_naming_every_page() {
    let fx = vault();
    knowledge_page(&fx, "one", None);
    knowledge_page(&fx, "two", None);
    let out = ok(&fx, &["tag", "lint"]);
    assert_eq!(out.matches("knowledge page(s)").count(), 1, "{out}");
    assert!(!out.contains("knowledge/"), "no page is named: {out}");
    assert!(out.contains("2 knowledge page(s) cannot carry a topic: workspace/life/tags.yaml has no 'topic' facet; af tag facet add topic --applies context"), "{out}");
    let none = vault();
    assert!(!ok(&none, &["tag", "lint"]).contains("knowledge"), "no knowledge page, no finding");
}

/// The places rules stay on places: the floor of three, the venue lines, the overload kinds, the cap of eight.
#[test]
fn the_place_rules_do_not_fire_on_knowledge_pages_or_sources() {
    let fx = topic_vault();
    knowledge_page(&fx, "one-tag", Some("leases"));
    source_file(&fx, "note-2026-11-01-one", Some("leases"));
    // one topic spread over many knowledge pages and sources, kinds that would be an overload on places
    for i in 0..8 {
        knowledge_page(&fx, &format!("k{i}"), Some("money"));
        source_file(&fx, &format!("note-2026-11-01-s{i}"), Some("money"));
    }
    let out = ok(&fx, &["tag", "lint"]);
    assert!(!out.contains("knowledge/") && !out.contains("sources/"), "the floor of three is a place rule: {out}");
    assert!(out.contains("second-bowl.md (restaurant) has 2 tag(s), floor 3"), "and it still fires on places: {out}");
    assert!(!out.contains("overload"), "{out}");
    // the lint line for people with no tags is still only people
    assert!(!out.contains("has no tags; af tag add an interest"), "{out}");
    // the venue coverage line counts places only
    let stats = ok(&fx, &["tag", "stats"]);
    assert!(stats.starts_with("af tag stats: 13 tags · 10 active · 1 proposed · 1 deprecated · 1 merged · 5 pages (4 places, 1 people)\n"), "{stats}");
    assert!(stats.contains("  venue pages with a tag in: cuisine 3/3 · vibe 2/3 · price 3/3\n"), "{stats}");
    assert!(stats.contains("  people pages tagged: 1/1\n"), "{stats}");
    assert!(stats.contains("  topic (context): health 0 · leases 2 · money 16 · travel 0\n"), "{stats}");
    assert!(stats.contains("  knowledge pages with a topic: 9/9\n  sources with a topic: 9/9\n"), "{stats}");
}

#[test]
fn stats_counts_knowledge_pages_and_sources_with_a_topic() {
    let fx = topic_vault();
    knowledge_page(&fx, "with", Some("leases"));
    knowledge_page(&fx, "without", None);
    knowledge_page(&fx, "empty", Some(""));
    source_file(&fx, "note-2026-11-01-a", Some("health"));
    source_file(&fx, "note-2026-11-01-b", None);
    source_file(&fx, "note-2026-11-01-c", None);
    fx.write("workspace/life/knowledge/_archive/old.md", "# Old\n");
    fx.write("workspace/life/knowledge/_history/x/2026-01-01-draft.md", "# Draft\n");
    fx.write("workspace/life/sources/INDEX.md", "# Source Index\n");
    fx.write("workspace/life/sources/inbox/waiting.md", &format!("---\n{FM}---\nx\n"));
    let out = ok(&fx, &["tag", "stats"]);
    assert!(out.ends_with("  knowledge pages with a topic: 1/3\n  sources with a topic: 1/3\n"), "{out}");
    // a facet used on pages that carry no topic is not a topic
    fx.ok(&["tag", "facet", "add", "depth", "--applies", "knowledge"]);
    fx.ok(&["tag", "propose", "overview", "--facet", "depth"]);
    fx.ok(&["tag", "add", "workspace/life/knowledge/without.md", "overview"]);
    assert!(ok(&fx, &["tag", "stats"]).contains("  knowledge pages with a topic: 1/3\n"));
}

#[test]
fn the_doctor_checks_knowledge_pages_and_sources_like_places() {
    let fx = topic_vault();
    fx.ok(&["tag", "facet", "add", "depth", "--applies", "knowledge", "--single"]);
    fx.ok(&["tag", "propose", "overview", "--facet", "depth"]);
    fx.ok(&["tag", "propose", "detail", "--facet", "depth"]);
    fx.ok(&["tag", "merge", "travel", "--into", "money"]);
    fx.ok(&["tag", "deprecate", "health"]);
    // clean to begin with, whatever the pages' frontmatter looks like
    knowledge_page(&fx, "clean", Some("leases"));
    knowledge_page(&fx, "no-frontmatter", None);
    source_file(&fx, "note-2026-11-01-clean", Some("leases"));
    assert_eq!(doctor(&fx), (vec![], vec![]));
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}");

    let page = "workspace/life/knowledge/broken.md";
    fx.write(page, "---\ntags: [ghost, travel, overview, detail]\n---\n# Broken\n");
    let src = "workspace/life/sources/note-2026-11-01-broken.md";
    fx.write(src, &format!("---\n{FM}tags: [noodles, health, money, leases, mystery]\n---\nbody\n"));
    let (issues, notes) = doctor(&fx);
    for want in [
        format!("{page}: tag 'ghost' is not in workspace/life/tags.yaml; af tag match ghost"),
        format!("{page}: tag 'travel' was merged into 'money'; rerun af tag merge travel --into money"),
        format!("{page}: two 'depth' tags (overview, detail); the facet takes one per page"),
        format!("{page}: 4 tags, over the cap of 3"),
        format!("{src}: tag 'mystery' is not in workspace/life/tags.yaml"),
        format!("{src}: 5 tags, over the cap of 3"),
    ] {
        assert!(any_contains(&issues, &want), "missing {want:?} in {issues:?}");
    }
    assert!(any_contains(&notes, &format!("{src}: tag 'noodles' is in 'cuisine', a places facet, on a sources page")), "{notes:?}");
    assert!(any_contains(&notes, &format!("{src}: tag 'health' is deprecated")), "{notes:?}");
    fx.write(page, "---\ntags: [overview]\n---\n# Broken\n");
    fx.write("workspace/life/places/second-bowl.md", "---\nprivate: true\nkind: restaurant\narea: harbour\naddress: 1 Example St\ntags: [noodles, price-high, leases]\nstatus: want\nchecked: 2026-01-01\n---\n# second-bowl\nAnother noodle house · [[harbour]]\n");
    let (_, notes) = doctor(&fx);
    assert!(any_contains(&notes, "second-bowl.md: tag 'leases' is in 'topic', a context facet, on a places page"), "{notes:?}");
    // the registry that is missing: sources and knowledge pages with tags count among the pages that carry them
    fx.remove("workspace/life/tags.yaml");
    let (_, notes) = doctor(&fx);
    assert!(any_contains(&notes, "tags.yaml missing while"), "{notes:?}");
}

#[test]
fn the_places_doctor_ignores_knowledge_pages_and_sources() {
    let fx = topic_vault();
    // none of these has the private flag, a kind or a type; places and people pages need them, these do not
    knowledge_page(&fx, "plain", Some("leases"));
    fx.write("workspace/life/knowledge/housing.md", "# Housing\n\n[[harbour]]\n");
    source_file(&fx, "note-2026-11-01-plain", Some("leases"));
    assert_eq!(doctor(&fx), (vec![], vec![]));
    let (code, out, _) = fx.run(&["doctor"]);
    assert_eq!(code, 0, "{out}");
}
