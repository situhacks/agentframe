//! `af place`, `af person`, and the doctor's check of their frontmatter, on a fixture vault
//! with invented places and people.

mod common;

use common::*;

const REGISTRY: &str = "\
facets:
  - name: cuisine
    applies: places
    cardinality: multi
  - name: interest
    applies: people
    cardinality: multi
tags:
  - id: noodles
    facet: cuisine
    status: active
    alt: [noodle]
    scope: \"Noodle houses\"
    since: 2026-01-10
  - id: climbing
    facet: interest
    status: active
    alt: []
    scope: \"Climbs\"
    since: 2026-01-10
";

const AREA: &str = "---\nprivate: true\nkind: area\narea:\naddress:\ntransit:\ntags: []\nstatus: been\nchecked: 2026-01-01\n---\n# West End\nArea\n\n## Places\n";

fn rig() -> Fx {
    let fx = Fx::new();
    fx.write("workspace/life/tags.yaml", REGISTRY);
    fx.write("workspace/life/open.md", "# Open loops\n\n## Waiting on the operator\n");
    fx.write("workspace/life/places/west-end.md", AREA);
    fx
}

const P: &str = "workspace/life/places/ramen-example.md";

fn add_place(fx: &Fx) -> String {
    fx.ok(&[
        "place", "add", "ramen-example", "--name", "Ramen Example", "--kind", "restaurant", "--area", "west-end", "--address",
        "12 Example St", "--transit", "12 bus", "--status", "want", "--tag", "noodle",
    ])
}

#[test]
fn add_writes_the_schema_shape_and_resolves_tags_through_the_registry() {
    let fx = rig();
    let out = add_place(&fx);
    assert!(out.contains("[[west-end]]"), "{out}");
    let text = fx.read(P);
    assert!(text.contains("kind: restaurant\narea: west-end\naddress: 12 Example St\ntransit: 12 bus\ntags: [noodles]\nstatus: want\n"), "{text}");
    assert!(text.contains("# Ramen Example\nRestaurant · noodles · [[west-end]] · 12 Example St · 12 bus · want\n"), "{text}");
    for s in ["## What it is", "## Practical", "## Vibe and music", "## Reviews found", "## His take", "## Visits"] {
        assert!(text.contains(s), "{s}");
    }
    assert!(text.contains(&format!("checked: {}", today())), "{text}");
}

#[test]
fn add_refuses_a_duplicate_slug_across_places_and_people_and_notes_a_shared_address() {
    let fx = rig();
    add_place(&fx);
    let msg = fx.refused(&["place", "add", "ramen-example", "--name", "X", "--kind", "bar", "--area", "west-end"]);
    assert!(msg.contains("already exists at workspace/life/places/ramen-example.md"), "{msg}");
    fx.ok(&["person", "add", "a-friend", "--name", "A Friend"]);
    let msg = fx.refused(&["place", "add", "a-friend", "--name", "X", "--kind", "bar", "--area", "west-end"]);
    assert!(msg.contains("workspace/life/people/a-friend.md"), "{msg}");
    let out = fx.ok(&[
        "place", "add", "ramen-renamed", "--name", "Renamed", "--kind", "restaurant", "--area", "west-end", "--address", "12 example st",
    ]);
    assert!(out.contains("[[ramen-example]] has the same address"), "{out}");
}

#[test]
fn add_refuses_bad_values_and_names_the_fix() {
    let fx = rig();
    let msg = fx.refused(&["place", "add", "x", "--name", "X", "--kind", "pub", "--area", "west-end"]);
    assert!(msg.contains("kind 'pub' is not one of restaurant"), "{msg}");
    let msg = fx.refused(&["place", "add", "x", "--name", "X", "--kind", "bar", "--area", "nowhere"]);
    assert!(msg.contains("does not resolve to a kind: area page"), "{msg}");
    let msg = fx.refused(&["place", "add", "x", "--name", "X", "--kind", "bar", "--area", "west-end", "--status", "loved"]);
    assert!(msg.contains("status 'loved' is not one of"), "{msg}");
    let msg = fx.refused(&["place", "add", "x", "--name", "X", "--kind", "bar", "--area", "west-end", "--tag", "invented"]);
    assert!(msg.contains("af tag propose invented"), "{msg}");
    let msg = fx.refused(&["place", "add", "x", "--name", "X", "--kind", "bar", "--area", "west-end", "--tag", "climbing"]);
    assert!(msg.contains("people facet") || msg.contains("a people"), "{msg}");
    let msg = fx.refused(&["place", "add", "Not A Slug", "--name", "X", "--kind", "bar", "--area", "west-end"]);
    assert!(msg.contains("kebab-case"), "{msg}");
}

#[test]
fn visit_writes_one_row_and_raises_the_verdict_loop_for_the_day_after() {
    let fx = rig();
    add_place(&fx);
    fx.ok(&["person", "add", "a-friend", "--name", "A Friend"]);
    fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner for 4", "--with", "a-friend", "--from", "src-2026-11-01-01"]);
    assert!(fx.read(P).contains("## Visits\n- 2026-11-02 · dinner for 4 · with: [[a-friend]] · verdict: pending"), "{}", fx.read(P));
    let open = fx.read("workspace/life/open.md");
    assert!(open.contains("verdict: Ramen Example, 2026-11-02 dinner for 4 · nudge 2026-11-03 · from: src-2026-11-01-01"), "{open}");
    assert!(!fx.read("workspace/life/people/a-friend.md").contains("2026-11-02"), "a visit is written once, on its place page");
}

#[test]
fn visit_refuses_a_person_with_no_page_and_a_second_row_for_the_day() {
    let fx = rig();
    add_place(&fx);
    let msg = fx.refused(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner", "--with", "a-stranger"]);
    assert!(msg.contains("af person add a-stranger"), "{msg}");
    fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner"]);
    let msg = fx.refused(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "again"]);
    assert!(msg.contains("af place verdict ramen-example --date 2026-11-02"), "{msg}");
    let msg = fx.refused(&["place", "visit", "no-such", "--date", "2026-11-02", "--what", "x"]);
    assert!(msg.contains("af place add no-such"), "{msg}");
}

#[test]
fn verdict_updates_the_row_his_take_the_status_and_closes_the_loop() {
    let fx = rig();
    add_place(&fx);
    fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner for 4"]);
    fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "yes", "--words", "the broth was the point"]);
    let text = fx.read(P);
    assert!(text.contains("- 2026-11-02 · dinner for 4 · verdict: yes"), "{text}");
    assert!(text.contains("## His take\n- 2026-11-02 · the broth was the point"), "{text}");
    assert!(text.contains("status: been\n"), "{text}");
    assert!(!fx.read("workspace/life/open.md").contains("verdict: Ramen Example"));
    let month = chrono::Local::now().format("%Y-%m").to_string();
    assert!(fx.read(&format!("workspace/life/log/{month}.md")).contains("closed: verdict: Ramen Example, 2026-11-02 dinner for 4"));
}

#[test]
fn a_verdict_in_his_words_stays_and_the_new_one_is_appended() {
    let fx = rig();
    add_place(&fx);
    fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner"]);
    let text = fx.read(P).replace("verdict: pending", "verdict: loud but we would go back");
    fx.write(P, &text);
    fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "yes", "--words", "go back"]);
    assert!(fx.read(P).contains("verdict: loud but we would go back; yes"), "{}", fx.read(P));
    fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "yes", "--words", "still yes"]);
    assert!(fx.read(P).contains("verdict: loud but we would go back; yes\n"), "the same answer twice stays once: {}", fx.read(P));
    fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "no", "--words", "changed his mind"]);
    assert!(fx.read(P).contains("verdict: loud but we would go back; yes; no"), "{}", fx.read(P));
}

#[test]
fn a_verdict_leaves_avoid_closed_and_regular_alone_unless_told() {
    for held in ["avoid", "closed", "regular"] {
        let fx = rig();
        add_place(&fx);
        fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner"]);
        let text = fx.read(P);
        let line = text.lines().find(|l| l.starts_with("status:")).unwrap().to_string();
        fx.write(P, &text.replacen(&line, &format!("status: {held}"), 1));
        fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "no", "--words", "as expected"]);
        assert!(fx.read(P).contains(&format!("status: {held}\n")), "{held}: {}", fx.read(P));
        fx.ok(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "yes", "--words", "changed", "--status", "been"]);
        assert!(fx.read(P).contains("status: been\n"), "--status still moves it: {}", fx.read(P));
    }
}

#[test]
fn verdict_refuses_a_missing_row_and_a_verdict_outside_the_vocabulary() {
    let fx = rig();
    add_place(&fx);
    let msg = fx.refused(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "yes", "--words", "x"]);
    assert!(msg.contains("af place visit ramen-example --date 2026-11-02"), "{msg}");
    fx.ok(&["place", "visit", "ramen-example", "--date", "2026-11-02", "--what", "dinner"]);
    let msg = fx.refused(&["place", "verdict", "ramen-example", "--date", "2026-11-02", "--verdict", "great", "--words", "x"]);
    assert!(msg.contains("yes, no, or maybe"), "{msg}");
}

#[test]
fn list_filters_on_exact_fields_and_finds_the_stale() {
    let fx = rig();
    add_place(&fx);
    fx.ok(&["place", "add", "bar-example", "--name", "Bar Example", "--kind", "bar", "--area", "west-end", "--status", "been"]);
    let bars = fx.ok(&["place", "list", "--kind", "bar"]);
    assert!(bars.contains("bar-example · bar · west-end · been") && !bars.contains("ramen-example"), "{bars}");
    let tagged = fx.ok(&["place", "list", "--tag", "noodle"]);
    assert!(tagged.contains("ramen-example") && !tagged.contains("bar-example"), "{tagged}");
    let want = fx.ok(&["place", "list", "--status", "want", "--area", "west-end"]);
    assert!(want.contains("1 place(s)"), "{want}");
    let stale = fx.ok(&["place", "list", "--stale"]);
    assert!(stale.contains("west-end") && !stale.contains("ramen-example"), "{stale}");
}

#[test]
fn person_add_writes_a_person_and_a_group() {
    let fx = rig();
    fx.ok(&["person", "add", "a-friend", "--name", "A Friend", "--met", "a class, 2026-09", "--tag", "climbing"]);
    let text = fx.read("workspace/life/people/a-friend.md");
    assert!(text.contains("private: true\nname: A Friend\ntype: person\nmet: a class, 2026-09\ntags: [climbing]\n"), "{text}");
    fx.ok(&["person", "add", "group-climbers", "--name", "Climbers", "--group", "--met", "a class"]);
    let text = fx.read("workspace/life/people/groups/group-climbers.md");
    assert!(text.contains("type: group\nformed: a class\nmembers: []\n") && text.contains("## Forming"), "{text}");
    let msg = fx.refused(&["person", "add", "climbers", "--name", "Climbers", "--group"]);
    assert!(msg.contains("group-climbers"), "{msg}");
    let msg = fx.refused(&["person", "add", "a-friend", "--name", "Other"]);
    assert!(msg.contains("already exists"), "{msg}");
}

#[test]
fn doctor_flags_a_hand_made_page_with_a_bad_status_and_a_dangling_link() {
    let fx = rig();
    fx.write(
        "workspace/life/places/hand-made.md",
        "---\nprivate: true\nkind: pub\narea: nowhere\ntags: []\nstatus: loved\nchecked: 2026-01-01\n---\n# Hand\nwith [[nobody-here]]\n",
    );
    fx.write("workspace/life/people/hand-person.md", "---\nname: Hand\ntype: friend\n---\n# Hand\n");
    let (issues, _) = afcore::places::doctor_checks(&fx.ctx()).unwrap();
    assert!(any_contains(&issues, "hand-made.md: kind 'pub' is not one of"), "{issues:?}");
    assert!(any_contains(&issues, "hand-made.md: status 'loved' is not one of"), "{issues:?}");
    assert!(any_contains(&issues, "hand-made.md: area 'nowhere' does not resolve"), "{issues:?}");
    assert!(any_contains(&issues, "hand-person.md: missing private: true"), "{issues:?}");
    assert!(any_contains(&issues, "hand-person.md: type 'friend' is not person or group"), "{issues:?}");
    let dangling = afcore::tags::dangling_link_issues(&fx.ctx()).unwrap();
    assert!(any_contains(&dangling, "[[nobody-here]] has no page"), "{dangling:?}");
    let (code, _, _) = fx.run(&["doctor"]);
    assert_eq!(code, 1, "doctor exits 1 on these");
}

#[test]
fn doctor_is_quiet_on_pages_the_buttons_made() {
    let fx = rig();
    add_place(&fx);
    fx.ok(&["person", "add", "a-friend", "--name", "A Friend"]);
    fx.ok(&["person", "add", "group-climbers", "--name", "Climbers", "--group"]);
    let (issues, _) = afcore::places::doctor_checks(&fx.ctx()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
}
