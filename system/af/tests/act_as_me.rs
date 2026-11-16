//! `af act`: the handover state, the session-start line, `af needs close --as-him`, and the
//! "Decided as you" view; and the word caps on the two public files that shape it.

mod common;
mod life_fx;
use afcore::sessions;
use common::*;
use life_fx::*;

const STATE: &str = "workspace/life/act-as-me.md";
const OPEN: &str = "workspace/life/open.md";
const VIEW: &str = "workspace/life/needs.md";
const ORDER: &str = "src-2026-11-15-23";

fn log_rel() -> String {
    format!("workspace/life/log/{}.md", month())
}

fn log(fx: &Fx) -> String {
    if fx.isfile(&log_rel()) {
        fx.read(&log_rel())
    } else {
        String::new()
    }
}

/// A life layer with one filed order and one decision on the list.
fn fixture() -> Fx {
    let fx = Fx::new();
    fx.write(
        "workspace/life/sources/orders/order-2026-11-15-23-handover.md",
        &format!("---\nid: {ORDER}\ntype: order\ndate: 2026-11-15\norigin: test\nverbatim: true\n---\ndo whatever\n"),
    );
    fx.ok(&["needs", "add", "Which plan to take?", "--rec", "the cheaper plan"]);
    fx
}

fn start(fx: &Fx) {
    fx.ok(&["act", "start", "--from", ORDER, "--scope", "my needs list", "--until", "+2h"]);
}

/// Write a handover whose `until` is `mins` minutes from now (negative: already past).
fn write_state(fx: &Fx, mins: i64) {
    fx.write(
        STATE,
        &format!("---\nfrom: {ORDER}\nscope: \"my needs list\"\nstarted: {}\nuntil: {}\n---\n", ts(-600), ts(mins)),
    );
}

fn start_line(fx: &Fx, session: &str) -> Option<String> {
    let out = sessions::hook_session_start(&fx.ctx(), &format!(r#"{{"session_id":"{session}","source":"startup"}}"#));
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let text = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap().to_string();
    text.lines().find(|l| l.starts_with("Acting as him")).map(str::to_string)
}

#[test]
fn start_refuses_an_unknown_source_an_empty_scope_and_a_past_time() {
    let fx = fixture();
    let e = fx.refused(&["act", "start", "--from", "src-2026-11-15-99", "--scope", "my needs list"]);
    assert!(e.contains("not a filed order"), "{e}");
    // A filed source that is not an order does not count.
    fx.write("workspace/life/sources/email-2026-11-15-x.md", "---\nid: src-2026-11-15-05\ntype: email\n---\nhi\n");
    let e = fx.refused(&["act", "start", "--from", "src-2026-11-15-05", "--scope", "my needs list"]);
    assert!(e.contains("not a filed order"), "{e}");
    let e = fx.refused(&["act", "start", "--from", ORDER, "--scope", "   "]);
    assert!(e.contains("--scope"), "{e}");
    let e = fx.refused(&["act", "start", "--from", ORDER, "--scope", "my needs list", "--until", "2020-01-01T09:00"]);
    assert!(e.contains("not in the future"), "{e}");
    let e = fx.refused(&["act", "start", "--from", ORDER, "--scope", "my needs list", "--until", "tomorrow"]);
    assert!(e.contains("--until takes"), "{e}");
    assert!(!fx.exists(STATE), "a refused start writes nothing");
}

#[test]
fn start_writes_the_file_status_reads_it_and_stop_ends_it() {
    let fx = fixture();
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    start(&fx);
    let state = fx.read(STATE);
    assert!(state.starts_with("---\nfrom: src-2026-11-15-23\nscope: \"my needs list\"\nstarted: "), "{state}");
    assert!(state.contains("\nuntil: ") && state.trim_end().ends_with("---"), "{state}");
    let status = fx.ok(&["act", "status"]);
    assert!(status.starts_with("acting as him until "), "{status}");
    assert!(status.contains(&format!("scope: \"my needs list\" [{ORDER}]")), "{status}");

    // One at a time: a second start replaces the first and says so.
    let out = fx.ok(&["act", "start", "--from", ORDER, "--scope", "do whatever", "--until", "+1h"]);
    assert!(out.contains("replaced the handover"), "{out}");
    assert!(log(&fx).contains("act_as_me_ended: replaced [src-2026-11-15-23] \"my needs list\""), "{}", log(&fx));
    assert!(fx.ok(&["act", "status"]).contains("scope: \"do whatever\""));

    fx.ok(&["act", "stop", "--outcome", "he is back"]);
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    assert!(!fx.exists(STATE));
    let l = log(&fx);
    assert!(l.contains("act_as_me_ended: stopped [src-2026-11-15-23] \"do whatever\" → he is back"), "{l}");
}

#[test]
fn start_defaults_to_the_next_nine() {
    let fx = fixture();
    fx.ok(&["act", "start", "--from", ORDER, "--scope", "my needs list"]);
    assert!(fx.read(STATE).contains("T09:00\n"), "{}", fx.read(STATE));
}

#[test]
fn an_expired_handover_reads_as_ended_and_the_next_loop_write_logs_it_once() {
    let fx = fixture();
    write_state(&fx, -5);
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    assert!(!log(&fx).contains("act_as_me_ended"), "a read logs nothing");
    fx.ok(&["open", "add", "call mum"]);
    let l = log(&fx);
    assert_eq!(l.matches("act_as_me_ended: expired [src-2026-11-15-23]").count(), 1, "{l}");
    assert!(!fx.exists(STATE));
    fx.ok(&["open", "add", "call dad"]);
    assert_eq!(log(&fx).matches("act_as_me_ended").count(), 1);
}

#[test]
fn session_start_prints_the_line_while_in_force_and_not_to_a_worker() {
    let fx = fixture();
    write_registry(
        &fx,
        &[
            row("af-main", "main", "s-main", &ts(-5), "live"),
            row("project/acme", "operator", "s-acme", &ts(-5), "live"),
            row("worker/job", "worker", "s-job", &ts(-5), "live"),
        ],
    );
    assert_eq!(start_line(&fx, "s-main"), None, "no handover, no line");
    write_state(&fx, 120);
    let until = ts(120);
    let want = format!("Acting as him until {until}: \"my needs list\" [{ORDER}]. Rules: library/process/act-as-me.md");
    assert_eq!(start_line(&fx, "s-main").as_deref(), Some(want.as_str()));
    assert_eq!(start_line(&fx, "s-acme").as_deref(), Some(want.as_str()));
    assert_eq!(start_line(&fx, "s-job"), None, "a worker runs one job and is not told");
    write_state(&fx, -1);
    assert_eq!(start_line(&fx, "s-main"), None, "past until, no line");
}

#[test]
fn close_as_him_refuses_without_a_handover_and_without_a_precedent_and_marks_the_log() {
    let fx = fixture();
    let e = fx.refused(&["needs", "close", "1", "--outcome", "cheaper, as in src-2026-11-04-02", "--as-him"]);
    assert!(e.contains("needs a handover in force"), "{e}");
    assert!(fx.read(OPEN).contains("Which plan to take?"), "a refused close leaves the item");

    start(&fx);
    let e = fx.refused(&["needs", "close", "1", "--outcome", "the cheaper plan", "--as-him"]);
    assert!(e.contains("precedent"), "{e}");
    assert!(fx.read(OPEN).contains("Which plan to take?"));

    fx.ok(&["needs", "close", "1", "--outcome", "the cheaper plan, as in src-2026-11-04-02", "--as-him"]);
    assert!(!fx.read(OPEN).contains("Which plan to take?"));
    let l = log(&fx);
    assert!(
        l.contains("closed: Which plan to take? → as him [src-2026-11-15-23]: the cheaper plan, as in src-2026-11-04-02"),
        "{l}"
    );

    // A log date counts as a precedent too; a plain close is never marked.
    fx.ok(&["needs", "add", "Renew?", "--rec", "renew"]);
    fx.ok(&["needs", "close", "1", "--outcome", "renewed, like log 2026-11-04", "--as-him"]);
    fx.ok(&["needs", "add", "Move?", "--rec", "stay"]);
    fx.ok(&["needs", "close", "1", "--outcome", "stay"]);
    assert!(!log(&fx).contains("as him [src-2026-11-15-23]: stay"));
}

#[test]
fn the_view_the_count_and_review_follow_the_decisions_made_as_him() {
    let fx = fixture();
    assert!(!fx.read(VIEW).contains("Decided as you"), "no decisions, no section");
    start(&fx);
    fx.ok(&["needs", "close", "1", "--outcome", "the cheaper plan, as in src-2026-11-04-02", "--as-him"]);
    fx.ok(&["needs", "add", "Renew?", "--rec", "renew"]);

    let view = fx.read(VIEW).replace("\r\n", "\n");
    let at_later = view.find("## Later").expect("later");
    let at_decided = view.find("## Decided as you (1)").unwrap_or_else(|| panic!("{view}"));
    assert!(at_decided > at_later, "{view}");
    assert!(view.contains("| When | Decision | What I chose |\n|---|---|---|\n"), "{view}");
    assert!(view.contains(" | Which plan to take? | the cheaper plan, as in src-2026-11-04-02 |
"), "{view}");

    let list = fx.ok(&["needs", "list"]);
    assert_eq!(list.lines().next().unwrap(), "needs you: 1 now, 0 to do, 0 later, 1 decided as you (workspace/life/needs.md)", "{list}");

    let out = fx.ok(&["act", "review"]);
    assert!(out.contains("1 decided as him marked seen"), "{out}");
    assert!(log(&fx).contains("act_review: 1 decided as him seen"));
    assert!(!fx.read(VIEW).contains("Decided as you"), "{}", fx.read(VIEW));
    let list = fx.ok(&["needs", "list"]);
    assert_eq!(list.lines().next().unwrap(), "needs you: 1 now, 0 to do, 0 later (workspace/life/needs.md)", "{list}");
    assert!(fx.ok(&["act", "review"]).contains("nothing decided as him is waiting"));

    // A later decision shows again, and only that one.
    fx.ok(&["needs", "close", "1", "--outcome", "renewed, like log 2026-11-04", "--as-him"]);
    let view = fx.read(VIEW);
    assert!(view.contains("## Decided as you (1)") && view.contains("renewed, like log 2026-11-04"), "{view}");
    assert!(!view.contains("the cheaper plan, as in"), "{view}");
}

fn words(rel: &str) -> usize {
    std::fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")).split_whitespace().count()
}

#[test]
fn the_process_file_and_the_schema_keep_their_word_caps() {
    let process = words("library/process/act-as-me.md");
    assert!(process > 0 && process <= 500, "library/process/act-as-me.md is {process} words; the cap is 500");
    let schema = words("library/schema/decision-profile.md");
    assert!(schema > 0 && schema <= 400, "library/schema/decision-profile.md is {schema} words; the cap is 400");
}

#[test]
fn the_process_file_carries_the_never_list() {
    let text = std::fs::read_to_string(repo_root().join("library/process/act-as-me.md")).unwrap().to_lowercase();
    for never in ["paying", "buying", "transferring", "trades", "securities orders", "signed or legally binding", "deleting an account", "first message to someone new"] {
        assert!(text.contains(never), "act-as-me.md must name {never:?} in its never-list");
    }
    assert!(text.contains("af voice bundle"));
    assert!(text.contains("af act stop"));
}
