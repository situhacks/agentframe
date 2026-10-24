//! `af voice lint`.

mod common;

use afcore::pyjson::PyVal;
use afcore::voice::lint::{lint, previous_version};
use common::Fx;

const VOICED: &str = "---\nstatus: drafting\nvoice:\n  base_register: informal\n---\n";
const PLAIN: &str = "---\nstatus: drafting\n---\n";

fn voiced(body: &str) -> String {
    format!("{VOICED}{body}")
}

fn count(codes: &[&str], code: &str) -> usize {
    codes.iter().filter(|c| **c == code).count()
}

// UserVoicedTests

#[test]
fn test_voice_block_marks_user_voiced() {
    assert!(lint(&voiced("Fine prose."), None).user_voiced);
}

#[test]
fn test_register_scalar_marks_user_voiced() {
    assert!(lint("---\nregister: formal\n---\nFine prose.", None).user_voiced);
}

#[test]
fn test_plain_deliverable_is_skipped() {
    let r = lint(&format!("{PLAIN}Your brain quietly checks out."), None);
    assert!(!r.user_voiced);
    assert!(r.hard.is_empty());
}

// BannedTicTests

#[test]
fn test_quietly_is_hard() {
    let r = lint(&voiced("An automated workflow gets things right and your brain quietly checks out."), None);
    assert_eq!(r.hard_codes(), vec!["banned-tic"]);
    assert_eq!(r.hard[0].line, 6);
}

#[test]
fn test_my_read_is_hard() {
    assert_eq!(lint(&voiced("My honest read is that this works."), None).hard_codes(), vec!["banned-tic"]);
}

#[test]
fn test_where_this_lands_is_hard() {
    let r = lint(&voiced("Here's where I land on all of it. And where this lands is the point."), None);
    assert_eq!(r.hard_codes(), vec!["banned-tic", "banned-tic"]);
    assert!(!r.soft_codes().contains(&"lands-as-arrival"));
}

#[test]
fn test_quoted_speech_is_exempt() {
    assert!(lint(&voiced("Bosworth said \"keep doing it quietly\" and the dashboard died."), None).hard.is_empty());
}

#[test]
fn test_html_comments_and_tables_are_ignored() {
    let body = "<!-- quietly, my read -->\n| quietly | x |\n\nA clean line.\n";
    assert!(lint(&voiced(body), None).hard.is_empty());
}

// DashTests

#[test]
fn test_two_dashes_in_one_sentence_is_hard() {
    let r = lint(&voiced("That's me — the person who built it — writing an essay."), None);
    assert_eq!(r.hard_codes(), vec!["double-dash"]);
}

#[test]
fn test_one_dash_per_sentence_is_fine_but_consecutive_is_soft() {
    let r = lint(&voiced("First point — with a pivot. Second point — another pivot."), None);
    assert!(r.hard.is_empty());
    assert!(r.soft_codes().contains(&"dash-consecutive"));
}

#[test]
fn test_dash_rate_alarm() {
    let body = vec!["Short sentence — with a dash."; 10].join(" ");
    assert!(lint(&voiced(&body), None).soft_codes().contains(&"dash-rate"));
}

#[test]
fn test_number_range_en_dash_does_not_count() {
    let r = lint(&voiced("Runs 19–34 words and stays plain."), None);
    assert_eq!(r.stat("em_dashes"), Some(&PyVal::Int(0)));
}

// HyphenNormalisationTests

#[test]
fn test_operator_hyphen_turned_into_em_dash_is_hard() {
    let prev = voiced("All press is good press even the bad ones - right? Another line.");
    let head = voiced("All press is good press even the bad ones — right? Another line.");
    assert_eq!(lint(&head, Some(&prev)).hard_codes(), vec!["hyphen-normalised"]);
}

#[test]
fn test_rewritten_sentence_is_not_flagged() {
    let prev = voiced("Sonnet is fine - but do you want to think about it? More.");
    let head = voiced("Sonnet is fine, but do you want to weigh the options? More.");
    assert!(lint(&head, Some(&prev)).hard.is_empty());
}

#[test]
fn test_hyphen_shift_is_soft() {
    let prev = voiced("One - two. Three - four. Five.");
    let head = voiced("One, two. Three — and four. Five.");
    assert!(lint(&head, Some(&prev)).soft_codes().contains(&"hyphen-shift"));
}

// SoftShapeTests

#[test]
fn test_litotes_and_thread_are_soft() {
    let r = lint(&voiced("Not unreasonable. Picking up that thread later. The email thread stays."), None);
    let codes = r.soft_codes();
    assert!(codes.contains(&"litotes"));
    assert_eq!(count(&codes, "thread-as-topic"), 1);
    assert!(r.hard.is_empty());
}

#[test]
fn test_lands_reach_ship_and_act_refs_are_soft() {
    let body = "# Act II: The threat\n\n\
The whole industry reached for the same three letters. That's where the argument lands. \
The day the model ships is the day it graduates. I'll get to that in Act II. The landing page stays.";
    let r = lint(&voiced(body), None);
    let codes = r.soft_codes();
    assert!(r.hard.is_empty());
    assert_eq!(count(&codes, "reach-for"), 1);
    assert_eq!(count(&codes, "lands-as-arrival"), 1);
    assert_eq!(count(&codes, "ship-for-release"), 1);
    assert_eq!(count(&codes, "act-cross-ref"), 1);
}

#[test]
fn test_ship_without_a_model_is_not_flagged() {
    let r = lint(&voiced("We shipped the pack on Tuesday and the tests stayed green."), None);
    assert_eq!(count(&r.soft_codes(), "ship-for-release"), 0);
}

#[test]
fn test_contrastive_forms_mid_paragraph_are_soft() {
    for line in [
        "A system that only works where its author is sitting isn't a system, it's a favour.",
        "It doesn't start with a bad person, it starts with a leaderboard.",
        "They own the outcome, not the ticket.",
        "That's not a character type. It's a Tuesday.",
        "The honest question isn't what the tool can do. It's what the end-user will do with it.",
        "This isn't five new projects, instead it's mostly an upgrade.",
    ] {
        let r = lint(&voiced(&format!("{line} The next sentence carries on.")), None);
        assert_eq!(count(&r.soft_codes(), "contrastive"), 1, "{line}");
        assert!(r.hard.is_empty(), "{line}");
    }
}

#[test]
fn test_contrastive_closing_a_paragraph_is_hard() {
    let body = "The fix isn't to ration, it's to route. The bill isn't the invoice, it's the unread work.\n\nNext paragraph.";
    let r = lint(&voiced(body), None);
    assert_eq!(count(&r.soft_codes(), "contrastive"), 1);
    assert_eq!(r.hard_codes(), vec!["contrastive-kicker"]);
}

#[test]
fn test_conditionals_questions_and_quotes_are_not_contrastive() {
    let body = "If the company doesn't have it yet, it's probably the first thing to build. \
Who doesn't want an extra pair of hands? That's the position this role sits in. \
He said \"it isn't a tool, it's a teammate\" on the call. \
It isn't cheap, and it takes time to set up. \
I'm not a super technical guy, so if I can't picture myself using it, it's a no-go. \
Users don't know what they don't know. That's change management, and it's old. \
I don't hold that against them. It is a prisoner's dilemma.";
    let r = lint(&voiced(body), None);
    let mut codes = r.soft_codes();
    codes.extend(r.hard_codes());
    assert!(!codes.contains(&"contrastive"));
    assert!(!codes.contains(&"contrastive-kicker"));
}

// FileTests

#[test]
fn test_previous_version_math() {
    let fx = Fx::new();
    for n in 1..=3 {
        fx.write(&format!("essay-v{n}.md"), &voiced("x."));
    }
    assert_eq!(previous_version(&fx.p("essay-v3.md")), Some(fx.p("essay-v2.md")));
    assert_eq!(previous_version(&fx.p("essay-v1.md")), None);
    assert_eq!(previous_version(&fx.p("essay-v9.md")), None);
    assert_eq!(previous_version(&fx.p("essay-FINAL.md")), None);
}

#[test]
fn test_cli_exit_codes() {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    let bad = fx.write("post-v2.md", &voiced("Your brain quietly checks out."));
    let good = fx.write("post-v1.md", &voiced("Your brain checks out."));
    let plain = fx.write("notes.md", &format!("{PLAIN}quietly"));
    let (code, out, _) = fx.run(&["voice", "lint", &bad, "--json"]);
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["hard"][0]["code"], "banned-tic");
    assert_eq!(v["prev"], good.as_str());
    let (code, out, _) = fx.run(&["voice", "lint", &good]);
    assert_eq!(code, 0);
    assert!(out.contains("ok: no hard findings"));
    let (code, out, _) = fx.run(&["voice", "lint", &plain]);
    assert_eq!(code, 0);
    assert!(out.contains("skipped"));
    // `af voice lint` writes no run-log row
    assert!(fx.run_log_rows().is_empty());
}

#[test]
fn test_cli_resolves_through_the_tracker() {
    let fx = Fx::new();
    fx.write("AGENTS.md", "# root\n");
    fx.write(
        "workspace/projects/p/project.md",
        "---\nname: p\nslug: p\ndeliverables:\n  post:\n    status: drafting\n    file: post/post-v1.md\n---\n",
    );
    fx.write("workspace/projects/p/post/post-v1.md", &voiced("Your brain quietly checks out."));
    let (code, out, _) = fx.run(&["voice", "lint", "--project", "p", "--deliverable", "post"]);
    assert_eq!(code, 1);
    assert!(out.starts_with(&format!("voice lint: workspace{0}projects{0}p{0}post{0}post-v1.md", std::path::MAIN_SEPARATOR)), "{out}");
    let (code, _, err) = fx.run(&["voice", "lint", "--project", "p", "--deliverable", "nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("could not resolve p / nope"));
    let (code, _, _) = fx.run(&["voice", "lint", "--project", "p"]);
    assert_eq!(code, 2);
    let (code, _, _) = fx.run(&["voice", "lint"]);
    assert_eq!(code, 2);
}
