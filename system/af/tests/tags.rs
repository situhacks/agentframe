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
    assert!(out.contains("with 11 facets and no tags"), "{out}");
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
    let out = fx.ok(&["tag", "rename", "quiet", "talkable"]);
    assert!(out.starts_with("af tag rename: quiet -> talkable · active · 2 page(s) rewritten; quiet stays an alt label"), "{out}");
    assert_eq!(tags_of(&fx, "workspace/life/places/noodle-house.md"), vec!["noodles", "talkable", "price-low"]);
    let reg = registry(&fx);
    let t = reg.get("talkable").unwrap();
    assert_eq!((t.status.as_str(), t.facet.as_str(), t.scope.as_str()), ("active", "vibe", "You can hear the table"));
    assert_eq!(t.alt, vec!["hushed", "quiet"]);
    assert_eq!(reg.get("quiet").unwrap().merged_into.as_deref(), Some("talkable"));
    assert!(fx.ok(&["tag", "match", "calm"]).contains("talkable · active · vibe · via merged from 'quiet'"));
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
