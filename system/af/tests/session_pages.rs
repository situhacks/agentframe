//! The session page: `af session tidy`, and where the page checks show (the Stop hook, the session-start
//! bundle, `af doctor`). The checks themselves are unit-tested in `src/page.rs`; these fixtures are written
//! here and carry nothing from a real `system/sessions/`.

mod common;
mod life_fx;
use afcore::sessions::{self, HookOutput};
use common::*;
use life_fx::*;

const CUT: &str = "system/sessions/system/cut/handoff.md";
const CUT_ARCHIVE: &str = "system/sessions/system/cut/archive.md";

/// One `written:` stamp for the whole run, an hour ahead so no file in a fixture is newer than it.
fn written() -> &'static str {
    static W: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    W.get_or_init(|| ts(60))
}

/// A valid page: `extra` goes between State and Next step.
fn page(extra: &str) -> String {
    format!(
        "---\nwritten: {}\nsession_id: s1\n---\n## State\nmid-build\n\n{extra}## Next step\ntest\n\n## Files touched\n- system/backlog.md\n",
        written()
    )
}

/// The bundle with its line ends as `\n` (stdout on Windows has CRLF).
fn bundle(fx: &Fx, session: &str) -> String {
    fx.ok(&["session", "bundle", "--session", session]).replace("\r\n", "\n")
}

fn item(age: i64, text: &str) -> String {
    format!("- {} · {text}\n", days_ago(age))
}

fn fixture() -> Fx {
    let fx = Fx::new();
    write_registry(
        &fx,
        &[
            row("system/cut", "builder", "s-cut", &ts(-5), "live"),
            row("af-main", "main", "s-main", &ts(-5), "live"),
            row("worker/gone", "worker", "s-gone", &ts(-900), "ended 2026-10-20T10:00 exit"),
        ],
    );
    fx.write("system/backlog.md", "# Backlog\n");
    set_mtime(&fx, "system/backlog.md", -400 * 24 * 3600);
    fx
}

fn block(o: &HookOutput) -> String {
    let v: serde_json::Value = serde_json::from_str(&o.stdout).unwrap_or_else(|_| panic!("not a block: {o:?}"));
    assert_eq!(v["decision"], "block", "{o:?}");
    v["reason"].as_str().unwrap().to_string()
}

fn stop(fx: &Fx, id: &str) -> HookOutput {
    sessions::hook_stop(&fx.ctx(), &format!(r#"{{"session_id":"{id}","stop_hook_active":false}}"#))
}

// ------------------------------------------------------------------ tidy

#[test]
fn tidy_moves_old_and_surplus_done_items_to_the_archive_by_month() {
    let fx = fixture();
    let mut done = String::new();
    for i in 0..11 {
        done.push_str(&item(1, &format!("fresh {i}")));
    }
    done.push_str(&format!("{}  second line of the old one\n", item(40, "old one")));
    let before = page(&format!("## Doing\n{}\n## Done\n{done}\n", item(2, "in progress")));
    fx.write(CUT, &before);
    let out = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!(out.0, 0, "{out:?}");
    assert!(out.1.contains("moved 2 Done item(s) to system/sessions/system/cut/archive.md"), "{}", out.1);
    assert!(out.1.contains("clean"), "{}", out.1);

    let after = fx.read(CUT);
    assert!(after.contains("fresh 9\n") && !after.contains("fresh 10") && !after.contains("old one") && !after.contains("second line"));
    assert!(after.contains("in progress"));

    let old_month = (chrono::Local::now().date_naive() - chrono::Duration::days(40)).format("%Y-%m").to_string();
    let now_month = (chrono::Local::now().date_naive() - chrono::Duration::days(1)).format("%Y-%m").to_string();
    let archive = fx.read(CUT_ARCHIVE);
    assert!(archive.contains(&format!("## {now_month}\n{}", item(1, "fresh 10"))), "{archive}");
    assert!(archive.contains(&format!("## {old_month}\n{}  second line of the old one\n", item(40, "old one"))), "{archive}");
}

#[test]
fn tidy_appends_to_an_existing_archive() {
    let fx = fixture();
    let month = (chrono::Local::now().date_naive() - chrono::Duration::days(20)).format("%Y-%m").to_string();
    fx.write(CUT_ARCHIVE, &format!("## {month}\n- {} · earlier\n", days_ago(60)));
    fx.write(CUT, &page(&format!("## Done\n{}", item(20, "stale"))));
    fx.ok(&["session", "tidy", "system/cut"]);
    let archive = fx.read(CUT_ARCHIVE);
    assert!(archive.starts_with(&format!("## {month}\n- {} · earlier\n", days_ago(60))), "{archive}");
    assert!(archive.contains(&item(20, "stale")), "{archive}");
    assert_eq!(archive.matches("· earlier").count(), 1);
}

#[test]
fn tidy_changes_no_other_byte_of_the_page_and_not_written() {
    let fx = fixture();
    let lf = page(&format!("## To do\n{}\n## Done\n{}{}", item(3, "next"), item(30, "old"), item(1, "new")));
    let crlf = lf.replace('\n', "\r\n");
    fx.write_bytes(CUT, crlf.as_bytes());
    let out = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!(out.0, 0, "{out:?}");
    let want = page(&format!("## To do\n{}\n## Done\n{}", item(3, "next"), item(1, "new"))).replace('\n', "\r\n");
    let got = std::fs::read(fx.path(CUT)).unwrap();
    assert_eq!(String::from_utf8(got).unwrap(), want);
    assert!(fx.read(CUT).contains(&format!("written: {}\n", written())));
}

#[test]
fn tidy_on_a_page_with_nothing_to_move_touches_no_file() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## Done\n{}", item(1, "new"))));
    set_mtime(&fx, CUT, -3600);
    let mtime = std::fs::metadata(fx.path(CUT)).unwrap().modified().unwrap();
    let out = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!(out.0, 0);
    assert_eq!(std::fs::metadata(fx.path(CUT)).unwrap().modified().unwrap(), mtime);
    assert!(!fx.exists(CUT_ARCHIVE), "no archive is created for nothing");
}

#[test]
fn tidy_check_prints_the_issues_and_writes_nothing() {
    let fx = fixture();
    let text = page(&format!("## Done\n{}## Notes\nx\n", item(30, "old")));
    fx.write(CUT, &text);
    let (code, out, _) = fx.run(&["session", "tidy", "system/cut", "--check"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(fx.read(CUT), text);
    assert!(!fx.exists(CUT_ARCHIVE));
    assert!(out.starts_with("system/sessions/system/cut/handoff.md: "), "{out}");
    assert!(out.contains(" of 1200 words"), "{out}");
    assert!(out.contains("[shape] Done: item \"old\" is 30 days old, over 14"), "{out}");
    assert!(out.contains("[shape] heading \"## Notes\" is not allowed"), "{out}");
    assert!(!out.contains("clean"));
}

#[test]
fn tidy_exits_1_while_an_issue_remains_and_0_when_clean() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## Done\n{}## Notes\nx\n", item(30, "old"))));
    let (code, out, _) = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!(code, 1, "the Done item moved, the heading is still wrong: {out}");
    assert!(out.contains("moved 1 Done item(s)") && out.contains("not allowed") && !out.contains("is 30 days old"), "{out}");
    fx.write(CUT, &page(""));
    let (code, out, _) = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!(code, 0);
    assert!(out.contains("  clean"), "{out}");
}

#[test]
fn tidy_prints_age_issues_and_they_count_for_the_exit_code() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## To do\n{}", item(30, "stale thing"))));
    let (code, out, _) = fx.run(&["session", "tidy", "system/cut", "--check"]);
    assert_eq!(code, 1);
    assert!(out.contains("[age] To do: item \"stale thing\" is dated "), "{out}");
}

#[test]
fn tidy_all_covers_every_chat_that_has_not_ended() {
    let fx = fixture();
    let old_done = format!("## Done\n{}", item(30, "old"));
    fx.write(CUT, &page(&old_done));
    fx.write("system/sessions/af-main/handoff.md", &page(&old_done));
    fx.write("system/sessions/worker/gone/handoff.md", &page(&old_done));
    let (code, out, _) = fx.run(&["session", "tidy", "--all"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("system/sessions/system/cut/handoff.md") && out.contains("system/sessions/af-main/handoff.md"), "{out}");
    assert!(!out.contains("worker/gone"), "{out}");
    assert!(fx.exists(CUT_ARCHIVE) && fx.exists("system/sessions/af-main/archive.md"));
    assert!(fx.read("system/sessions/worker/gone/handoff.md").contains("old"), "an ended row is left alone");
    assert!(!fx.exists("system/sessions/worker/gone/archive.md"));
}

#[test]
fn tidy_by_name_works_on_an_ended_row_and_says_when_there_is_no_page() {
    let fx = fixture();
    fx.write("system/sessions/worker/gone/handoff.md", &page(&format!("## Done\n{}", item(30, "old"))));
    let (code, out, _) = fx.run(&["session", "tidy", "worker/gone"]);
    assert_eq!(code, 0, "{out}");
    assert!(fx.exists("system/sessions/worker/gone/archive.md"));
    let (code, out, _) = fx.run(&["session", "tidy", "system/cut"]);
    assert_eq!((code, out.trim()), (0, "system/sessions/system/cut/handoff.md: no handoff yet"));
    let err = fx.refused(&["session", "tidy", "project/nope"]);
    assert!(err.contains("no registry row for project/nope"), "{err}");
}

#[test]
fn tidy_with_no_name_is_the_calling_chat() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## Done\n{}", item(30, "old"))));
    let out = fx.af().args(["session", "tidy"]).env("CLAUDE_CODE_SESSION_ID", "s-cut").output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("system/sessions/system/cut/handoff.md: "));
    assert!(fx.exists(CUT_ARCHIVE));
    let err = fx.refused(&["session", "tidy"]);
    assert!(err.contains("needs a chat"), "no caller and no name: {err}");
    let out = fx.af().args(["session", "tidy"]).env("CLAUDE_CODE_SESSION_ID", "s-unknown").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn a_name_and_all_do_not_mix() {
    let fx = fixture();
    let (code, _, _) = fx.run(&["session", "tidy", "system/cut", "--all"]);
    assert_eq!(code, 2);
}

#[test]
fn a_tidy_exit_1_is_not_counted_as_a_button_refusal() {
    let fx = fixture();
    fx.write(CUT, &page("## Notes\nx\n"));
    assert_eq!(fx.run(&["session", "tidy", "system/cut", "--check"]).0, 1);
    assert!(afcore::runlog::button_notes(&fx.ctx()).is_empty(), "{:?}", afcore::runlog::button_notes(&fx.ctx()));
    assert_eq!(fx.run(&["session", "tidy", "project/nope"]).0, 1);
    assert_eq!(afcore::runlog::button_notes(&fx.ctx()).len(), 1, "a real refusal still counts");
}

// ------------------------------------------------------------------ the Stop hook

#[test]
fn stop_blocks_on_a_shape_issue_for_a_builder_and_names_the_fix() {
    let fx = fixture();
    fx.write(CUT, &page("## Notes\nx\n"));
    let reason = block(&stop(&fx, "s-cut"));
    assert!(reason.starts_with("Fix system/sessions/system/cut/handoff.md before stopping: the page has 1 shape issue(s):"), "{reason}");
    assert!(reason.contains("- heading \"## Notes\" is not allowed"), "{reason}");
    assert!(reason.contains("Run `af session tidy`, fix the rest, then set `written:`"), "{reason}");
    let (code, out, _) = fx.run(&["session", "stop", "system/cut"]);
    assert_eq!(code, 3);
    assert!(out.starts_with("HANDOFF_REQUIRED system/sessions/system/cut/handoff.md (the page has 1 shape issue(s)"), "{out}");
    let again = sessions::hook_stop(&fx.ctx(), r#"{"session_id":"s-cut","stop_hook_active":true}"#);
    assert_eq!(again, HookOutput::quiet(), "it blocks once");
}

#[test]
fn stop_is_quiet_for_a_clean_page_and_for_an_age_issue() {
    let fx = fixture();
    fx.write(CUT, &page(""));
    assert_eq!(stop(&fx, "s-cut"), HookOutput::quiet());
    fx.write(CUT, &page(&format!("## To do\n{}## Rules\n{}", item(100, "ancient"), item(100, "old rule"))));
    assert_eq!(stop(&fx, "s-cut"), HookOutput::quiet(), "age issues never block a stop");
}

#[test]
fn stop_never_blocks_a_main_row_for_a_shape_issue_but_the_verb_reports_it() {
    let fx = fixture();
    fx.write("system/sessions/af-main/handoff.md", &page("## Notes\nx\n"));
    assert_eq!(stop(&fx, "s-main"), HookOutput::quiet());
    let (code, out, _) = fx.run(&["session", "stop", "af-main"]);
    assert_eq!(code, 3, "{out}");
}

#[test]
fn stop_still_asks_about_staleness_before_the_shape() {
    let fx = fixture();
    fx.write("system/touched.md", "x");
    let text = page("## Notes\nx\n")
        .replace(written(), "2026-06-01T10:00")
        .replace("- system/backlog.md", "- system/touched.md");
    fx.write(CUT, &text);
    let reason = block(&stop(&fx, "s-cut"));
    assert!(reason.starts_with("Refresh system/sessions/system/cut/handoff.md before stopping: system/touched.md changed at"), "{reason}");
    fx.write(CUT, &text.replace("written: 2026-06-01T10:00\n", ""));
    assert!(block(&stop(&fx, "s-cut")).contains("no `written:` line"));
}

#[test]
fn the_missing_handoff_message_names_the_required_sections() {
    let fx = fixture();
    let reason = block(&stop(&fx, "s-cut"));
    assert!(reason.contains("State, Next step and Files touched"), "{reason}");
    assert!(!reason.contains("State, Open questions"), "{reason}");
    assert!(reason.contains("library/schema/handoff.md"), "{reason}");
}

#[test]
fn the_archive_does_not_make_the_page_stale() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## Done\n{}", item(30, "old"))).replace(written(), &ts(-30)));
    fx.ok(&["session", "tidy", "system/cut"]);
    assert!(fx.exists(CUT_ARCHIVE));
    let (_, notes) = afcore::life::doctor_checks(&fx.ctx()).unwrap();
    assert!(!notes.iter().any(|n| n.contains("is older than its folder's newest file")), "{notes:?}");
}

// ------------------------------------------------------------------ session start

#[test]
fn the_bundle_adds_the_tidy_block_for_a_page_with_an_issue_and_not_for_a_clean_one() {
    let fx = fixture();
    fx.write(CUT, &page("## Notes\nx\n"));
    let out = bundle(&fx, "s-cut");
    let handoff_at = out.find("last handoff (system/sessions/system/cut/handoff.md):").unwrap();
    let block_at = out.find("af: your page needs tidying before other work:\n- heading \"## Notes\" is not allowed").unwrap_or_else(|| panic!("{out}"));
    assert!(block_at > handoff_at, "the block follows the page");
    fx.write(CUT, &page(""));
    let out = bundle(&fx, "s-cut");
    assert!(out.contains("last handoff") && !out.contains("needs tidying"), "{out}");
}

#[test]
fn the_bundle_shows_age_issues_to_every_role_main_included() {
    let fx = fixture();
    fx.write("system/sessions/af-main/handoff.md", &page(&format!("## Rules\n{}", item(40, "always do this"))));
    let out = bundle(&fx, "s-main");
    assert!(out.contains("af: your page needs tidying before other work:\n- Rules: item \"always do this\" is dated "), "{out}");
}

#[test]
fn an_over_long_page_is_cut_at_1200_words_with_a_line_saying_so() {
    let fx = fixture();
    let lines: String = (0..130).map(|i| format!("LINE-{i} {}\n", ["w"; 9].join(" "))).collect();
    let text = format!("---\nwritten: {}\nsession_id: s1\n---\n## State\n{lines}\n## Next step\ntest\n\n## Files touched\n- system/backlog.md\n", ts(60));
    let words = afcore::page::Page::parse(&text).words();
    assert!(words > 1300);
    fx.write(CUT, &text);
    let out = bundle(&fx, "s-cut");
    assert!(out.contains("written: ") && out.contains("session_id: s1"), "the frontmatter is shown");
    assert!(out.contains("LINE-0 ") && out.contains("LINE-118 "), "{out}");
    assert!(!out.contains("LINE-119 ") && !out.contains("LINE-129") && !out.contains("## Files touched"), "cut after the 1200th word at a line end");
    assert!(
        out.contains(&format!("af: this page was cut after 1200 words; it has {words}. Read system/sessions/system/cut/handoff.md for the rest")),
        "{out}"
    );
    assert!(out.contains(&format!("the page is {words} words, over the 1200-word cap")), "the issue block says it too");
}

#[test]
fn a_page_of_exactly_1200_words_is_printed_whole() {
    let fx = fixture();
    let base = page("");
    let have = afcore::page::Page::parse(&base).words();
    let filler = vec!["w"; 1200 - have].join(" ");
    let text = base.replace("mid-build", &format!("mid-build {filler}"));
    assert_eq!(afcore::page::Page::parse(&text).words(), 1200);
    fx.write(CUT, &text);
    let out = bundle(&fx, "s-cut");
    assert!(out.contains("## Files touched") && !out.contains("was cut"), "{out}");
}

// ------------------------------------------------------------------ doctor

#[test]
fn doctor_has_one_note_per_issue_for_chats_that_have_not_ended() {
    let fx = fixture();
    fx.write(CUT, &page(&format!("## Notes\nx\n## To do\n{}", item(30, "stale thing"))));
    fx.write("system/sessions/worker/gone/handoff.md", &page("## Notes\nx\n"));
    let (_, notes) = afcore::life::doctor_checks(&fx.ctx()).unwrap();
    let mine: Vec<&String> = notes.iter().filter(|n| n.starts_with("system/sessions/system/cut/handoff.md: ")).collect();
    assert_eq!(mine.len(), 2, "{notes:?}");
    assert!(mine.iter().any(|n| n.contains("heading \"## Notes\" is not allowed")), "{mine:?}");
    assert!(mine.iter().any(|n| n.contains("To do: item \"stale thing\" is dated")), "{mine:?}");
    assert!(!notes.iter().any(|n| n.contains("worker/gone")), "an ended row has no note: {notes:?}");
    fx.write(CUT, &page(""));
    let (_, notes) = afcore::life::doctor_checks(&fx.ctx()).unwrap();
    assert!(!notes.iter().any(|n| n.contains("system/cut")), "{notes:?}");
}
