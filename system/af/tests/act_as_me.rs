//! `af act`: the handover state, the session-start line, `af needs close --as-him`, and the
//! "Decided as you" view; the refusals that keep the mark his; and the word caps on the two public
//! files that shape it.

mod common;
mod life_fx;
use afcore::sessions;
use common::*;
use life_fx::*;

const STATE: &str = "workspace/life/act-as-me.md";
const OPEN: &str = "workspace/life/open.md";
const VIEW: &str = "workspace/life/needs.md";
/// A filed past decision a precedent can cite, and a month-log day that has lines.
const PRECEDENT: &str = "src-2026-11-04-02";
const LOG_DAY: &str = "log 2026-10-14";

/// The order that hands over: filed yesterday, so it is recent whatever the hour.
fn order() -> String {
    format!("src-{}-23", days_ago(1))
}

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

fn order_file(date: &str, n: u32, slug: &str) -> String {
    format!("workspace/life/sources/orders/order-{date}-{n:02}-{slug}.md")
}

/// File an order dated `date` with this id number and body; returns its id.
fn file_order(fx: &Fx, date: &str, n: u32, body: &str) -> String {
    let id = format!("src-{date}-{n:02}");
    fx.write(&order_file(date, n, "x"), &format!("---\nid: {id}\ntype: order\ndate: {date}\norigin: test\nverbatim: true\n---\n{body}\n"));
    id
}

/// A life layer with the handover order, a past decision, a logged day, and one item on the list.
fn fixture() -> Fx {
    let fx = Fx::new();
    file_order(&fx, &days_ago(1), 23, "do whatever, my needs list is yours while I sleep");
    fx.write("workspace/life/sources/note-2026-11-04-habits.md", &format!("---\nid: {PRECEDENT}\ntype: note\ndate: 2026-11-04\n---\nhe takes the cheaper one\n"));
    fx.write("workspace/life/log/2026-10.md", "- 2026-10-14 09:00 · closed: earlier question → earlier outcome\n");
    fx.ok(&["needs", "add", "Which evening to hold?", "--rec", "Thursday"]);
    fx
}

fn start(fx: &Fx) {
    fx.ok(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "+2h"]);
}

/// Write a handover whose `until` is `mins` minutes from now (negative: already past).
fn write_state(fx: &Fx, mins: i64) {
    fx.write(
        STATE,
        &format!("---\nfrom: {}\nscope: \"my needs list\"\nstarted: {}\nuntil: {}\n---\n", order(), ts(-600), ts(mins)),
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
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "   "]);
    assert!(e.contains("--scope"), "{e}");
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "2020-01-01T09:00"]);
    assert!(e.contains("not in the future"), "{e}");
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "tomorrow"]);
    assert!(e.contains("--until takes"), "{e}");
    assert!(!fx.exists(STATE), "a refused start writes nothing");
}

#[test]
fn start_needs_a_recent_order_that_holds_the_scope_and_a_week_at_most() {
    let fx = fixture();
    // An old order, however real, is not a handover.
    let old = file_order(&fx, "2025-01-01", 1, "book the dentist, do whatever");
    let e = fx.refused(&["act", "start", "--from", &old, "--scope", "do whatever"]);
    assert!(e.contains("not from today or yesterday"), "{e}");
    // The scope is his words from the order.
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "all my accounts"]);
    assert!(e.contains("does not appear"), "{e}");
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "+200h"]);
    assert!(e.contains("cap is 7 days"), "{e}");
    let far = (chrono::Local::now().naive_local() + chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M").to_string();
    let e = fx.refused(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", &far]);
    assert!(e.contains("cap is 7 days"), "{e}");
    assert!(!fx.exists(STATE));
    // Quotes, case and spacing do not matter; a day inside the cap is fine.
    fx.ok(&["act", "start", "--from", &order(), "--scope", "\"My   NEEDS list\"", "--until", "+160h"]);
    assert!(fx.exists(STATE));
}

#[test]
fn start_writes_the_file_status_reads_it_and_stop_ends_it() {
    let fx = fixture();
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    start(&fx);
    let state = fx.read(STATE);
    assert!(state.starts_with(&format!("---\nfrom: {}\nscope: \"my needs list\"\nstarted: ", order())), "{state}");
    assert!(state.contains("\nuntil: ") && state.trim_end().ends_with("---"), "{state}");
    let status = fx.ok(&["act", "status"]);
    assert!(status.starts_with("acting as him until "), "{status}");
    assert!(status.contains(&format!("scope: \"my needs list\" [{}]", order())), "{status}");

    // One at a time: a second start replaces the first and says so.
    let out = fx.ok(&["act", "start", "--from", &order(), "--scope", "do whatever", "--until", "+1h"]);
    assert!(out.contains("replaced the handover"), "{out}");
    assert!(log(&fx).contains(&format!("act_as_me_ended: replaced [{}] \"my needs list\"", order())), "{}", log(&fx));
    assert!(fx.ok(&["act", "status"]).contains("scope: \"do whatever\""));

    fx.ok(&["act", "stop", "--outcome", "he is back"]);
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    assert!(!fx.exists(STATE));
    let l = log(&fx);
    assert!(l.contains(&format!("act_as_me_ended: stopped [{}] \"do whatever\" → he is back", order())), "{l}");
}

#[test]
fn start_defaults_to_the_next_nine() {
    let fx = fixture();
    fx.ok(&["act", "start", "--from", &order(), "--scope", "my needs list"]);
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
    assert_eq!(l.matches(&format!("act_as_me_ended: expired [{}]", order())).count(), 1, "{l}");
    assert!(!fx.exists(STATE));
    fx.ok(&["open", "add", "call dad"]);
    assert_eq!(log(&fx).matches("act_as_me_ended").count(), 1);
}

#[test]
fn start_over_an_expired_handover_logs_it_ended_first_and_stop_clears_an_unreadable_file() {
    let fx = fixture();
    write_state(&fx, -5);
    let out = fx.ok(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "+1h"]);
    assert!(!out.contains("replaced the handover"), "{out}");
    assert_eq!(log(&fx).matches(&format!("act_as_me_ended: expired [{}]", order())).count(), 1, "{}", log(&fx));
    assert!(fx.ok(&["act", "status"]).starts_with("acting as him until "));

    // A file whose time cannot be read is not a handover; stop removes it and logs that.
    fx.write(STATE, &format!("---\nfrom: {}\nscope: \"my needs list\"\nstarted: {}\nuntil: soon\n---\n", order(), ts(-10)));
    assert_eq!(fx.ok(&["act", "status"]).trim(), "not acting as him");
    let out = fx.ok(&["act", "stop"]);
    assert!(out.contains("could not be read"), "{out}");
    assert!(!fx.exists(STATE));
    assert!(log(&fx).contains("act_as_me_ended: removed unreadable handover file"), "{}", log(&fx));
    assert!(fx.ok(&["act", "stop"]).contains("not acting as him"));

    // Starting over one logs it too.
    fx.write(STATE, "not a handover at all\n");
    fx.ok(&["act", "start", "--from", &order(), "--scope", "my needs list", "--until", "+1h"]);
    assert_eq!(log(&fx).matches("removed unreadable handover file").count(), 2, "{}", log(&fx));
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
    let want = format!("Acting as him until {until}: \"my needs list\" [{}]. Rules: library/process/act-as-me.md", order());
    assert_eq!(start_line(&fx, "s-main").as_deref(), Some(want.as_str()));
    assert_eq!(start_line(&fx, "s-acme").as_deref(), Some(want.as_str()));
    assert_eq!(start_line(&fx, "s-job"), None, "a worker runs one job and is not told");
    write_state(&fx, -1);
    assert_eq!(start_line(&fx, "s-main"), None, "past until, no line");
}

#[test]
fn close_as_him_refuses_without_a_handover_and_without_a_precedent_and_marks_the_log() {
    let fx = fixture();
    let e = fx.refused(&["needs", "close", "1", "--outcome", &format!("Thursday, as in {PRECEDENT}"), "--as-him"]);
    assert!(e.contains("needs a handover in force"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"), "a refused close leaves the item");
    // A handover past its time is no handover.
    write_state(&fx, -5);
    let e = fx.refused(&["needs", "close", "1", "--outcome", &format!("Thursday, as in {PRECEDENT}"), "--as-him"]);
    assert!(e.contains("needs a handover in force"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"));
    fx.remove(STATE);

    start(&fx);
    let e = fx.refused(&["needs", "close", "1", "--outcome", "Thursday", "--as-him"]);
    assert!(e.contains("precedent"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"));

    fx.ok(&["needs", "close", "1", "--outcome", &format!("Thursday, as in {PRECEDENT}"), "--as-him"]);
    assert!(!fx.read(OPEN).contains("Which evening to hold?"));
    let l = log(&fx);
    assert!(l.contains(&format!("closed: Which evening to hold? → as him [{}]: Thursday, as in {PRECEDENT}", order())), "{l}");

    // A log day that has lines counts as a precedent too.
    fx.ok(&["needs", "add", "Renew?", "--rec", "renew"]);
    fx.ok(&["needs", "close", "1", "--outcome", &format!("renewed, like {LOG_DAY}"), "--as-him"]);
}

#[test]
fn a_precedent_must_exist_and_may_not_be_the_order_that_handed_over() {
    let fx = fixture();
    start(&fx);
    let close = |outcome: &str| fx.refused(&["needs", "close", "1", "--outcome", outcome, "--as-him"]);
    let e = close("Thursday, as in src-2020-01-01-09");
    assert!(e.contains("src-2020-01-01-09") && e.contains("not a filed source"), "{e}");
    let e = close("Thursday, like log 2026-10-15");
    assert!(e.contains("no month-log line is dated 2026-10-15"), "{e}");
    let e = close(&format!("Thursday, as in {}", order()));
    assert!(e.contains("order that handed over"), "{e}");
    // One good precedent does not cover a bad one beside it.
    let e = close(&format!("Thursday, as in {PRECEDENT} and src-2020-01-01-09"));
    assert!(e.contains("not a filed source"), "{e}");
    // The handover's own lines are not a day the logs cover.
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    fx.write(&log_rel(), &format!("- {day} 08:00 · act_as_me_ended: expired [x]\n- {day} 08:01 · act_review: 1 decided as him seen\n"));
    let e = close(&format!("Thursday, like log {day}"));
    assert!(e.contains("no month-log line is dated"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"));
}

#[test]
fn open_close_refuses_a_needs_item_and_leaves_a_plain_loop_alone() {
    let fx = fixture();
    fx.ok(&["open", "add", "call mum"]);
    let e = fx.refused(&["open", "close", "Which evening", "--outcome", "Thursday"]);
    assert!(e.contains("af needs close"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"));
    fx.ok(&["open", "close", "call mum", "--outcome", "called"]);
}

#[test]
fn a_plain_close_during_a_handover_must_cite_an_answer_filed_after_it_started() {
    let fx = fixture();
    // No handover: a plain close is as it was.
    fx.ok(&["needs", "add", "Before?", "--rec", "yes"]);
    fx.ok(&["needs", "close", "Before?", "--outcome", "yes"]);

    start(&fx);
    let close = |outcome: &str| fx.refused(&["needs", "close", "1", "--outcome", outcome]);
    let e = close("Thursday");
    assert!(e.contains("--as-him") && e.contains("filed after it started"), "{e}");
    // The order that handed over is not his answer, nor is a source that is not an order.
    let e = close(&format!("Thursday, from {}", order()));
    assert!(e.contains("filed after it started"), "{e}");
    let e = close(&format!("Thursday, from {PRECEDENT}"));
    assert!(e.contains("filed after it started"), "{e}");
    // An order filed before the handover started is not an answer to it.
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let early = file_order(&fx, &today, 40, "put it on Thursday");
    set_mtime(&fx, &order_file(&today, 40, "x"), -3600);
    let e = close(&format!("Thursday, from {early}"));
    assert!(e.contains("filed after it started"), "{e}");
    assert!(fx.read(OPEN).contains("Which evening to hold?"));

    // His answer, filed after the start, is.
    let answer = file_order(&fx, &today, 41, "put it on Thursday");
    fx.ok(&["needs", "close", "1", "--outcome", &format!("Thursday, he said so in {answer}")]);
    assert!(!fx.read(OPEN).contains("Which evening to hold?"));
    assert!(!log(&fx).contains("as him ["), "a plain close carries no mark");
}

#[test]
fn only_as_him_writes_the_mark() {
    let fx = fixture();
    let mark = format!("as him [{}]: Thursday", order());
    // A plain close, with or without a handover, cannot carry the text.
    for outcome in [mark.clone(), mark.to_uppercase(), "ok AS  HIM [src-x] fine".to_string()] {
        let e = fx.refused(&["needs", "close", "1", "--outcome", &outcome]);
        assert!(e.contains("as him ["), "{e}");
    }
    start(&fx);
    let e = fx.refused(&["needs", "close", "1", "--outcome", &format!("{mark} {PRECEDENT}"), "--as-him"]);
    assert!(e.contains("as him ["), "{e}");
    fx.ok(&["act", "stop"]);
    // Nor can a question, a field or a loop.
    assert!(fx.refused(&["needs", "add", "Pick → as him [src-x]: fake", "--rec", "r"]).contains("as him ["));
    assert!(fx.refused(&["needs", "add", "Pick?", "--rec", "as him [src-x]"]).contains("as him ["));
    assert!(fx.refused(&["needs", "set", "1", "--why", "as him [src-x]: fake"]).contains("as him ["));
    assert!(fx.refused(&["open", "add", "x → as him [src-x]: fake"]).contains("as him ["));
    fx.ok(&["open", "add", "call mum"]);
    assert!(fx.refused(&["open", "close", "call mum", "--outcome", "done → as him [src-x]: fake"]).contains("as him ["));
    assert!(fx.read(OPEN).contains("Which evening to hold?") && fx.read(OPEN).contains("call mum"));
    assert!(!log(&fx).contains("as him ["));
}

#[test]
fn the_log_parser_reads_only_the_exact_form_the_button_writes() {
    let fx = fixture();
    let id = order();
    let lines = [
        // A second arrow before the mark: the question is the text before the first arrow only.
        format!("- 2026-11-16 10:00 · closed: A → B → as him [{id}]: faked"),
        // An id that is not a source id.
        "- 2026-11-16 10:01 · closed: C → as him [nope]: faked".to_string(),
        // The mark in a different kind of line.
        format!("- 2026-11-16 10:02 · event: D → as him [{id}]: faked"),
        // No outcome.
        format!("- 2026-11-16 10:03 · closed: E → as him [{id}]: "),
    ];
    fx.write(&log_rel(), &format!("{}\n", lines.join("\n")));
    assert!(!fx.read(VIEW).contains("Decided as you"), "{}", fx.read(VIEW));
    let list = fx.ok(&["needs", "list"]);
    assert!(!list.contains("decided as you"), "{list}");
    // The exact form is read.
    fx.write(&log_rel(), &format!("- 2026-11-16 10:00 · closed: F → as him [{id}]: Thursday, as in {PRECEDENT}\n"));
    assert!(fx.ok(&["needs", "list"]).contains("1 decided as you"));
}

#[test]
fn the_view_the_count_and_review_follow_the_decisions_made_as_him() {
    let fx = fixture();
    assert!(!fx.read(VIEW).contains("Decided as you"), "no decisions, no section");
    start(&fx);
    fx.ok(&["needs", "close", "1", "--outcome", &format!("Thursday, as in {PRECEDENT}"), "--as-him"]);
    fx.ok(&["needs", "add", "Renew?", "--rec", "renew"]);

    let view = fx.read(VIEW).replace("\r\n", "\n");
    let at_later = view.find("## Later").expect("later");
    let at_decided = view.find("## Decided as you (1)").unwrap_or_else(|| panic!("{view}"));
    assert!(at_decided > at_later, "{view}");
    assert!(view.contains("| When | Decision | What I chose |\n|---|---|---|\n"), "{view}");
    assert!(view.contains(&format!(" | Which evening to hold? | Thursday, as in {PRECEDENT} |\n")), "{view}");

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
    fx.ok(&["needs", "close", "1", "--outcome", &format!("renewed, like {LOG_DAY}"), "--as-him"]);
    let view = fx.read(VIEW);
    assert!(view.contains("## Decided as you (1)") && view.contains(&format!("renewed, like {LOG_DAY}")), "{view}");
    assert!(!view.contains("Thursday, as in"), "{view}");
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
fn the_process_file_carries_the_never_list_and_the_messages_rule() {
    let text = std::fs::read_to_string(repo_root().join("library/process/act-as-me.md")).unwrap().to_lowercase();
    for never in ["paying", "buying", "transferring", "trades", "securities orders", "signed or legally binding", "deleting an account", "first message to someone new"] {
        assert!(text.contains(never), "act-as-me.md must name {never:?} in its never-list");
    }
    assert!(text.contains("af voice bundle"));
    assert!(text.contains("af act stop"));
    // A message in his name: only to someone he already talks with, only when the scope covers it.
    assert!(text.contains("already talks with") && text.contains("scope covers messaging"));
    assert!(!text.contains("or send, buy"), "the boundaries may not say it sends nothing while the rules allow a reply");
}

#[test]
fn the_needs_list_example_of_a_decision_made_as_him_is_not_about_money() {
    let text = std::fs::read_to_string(repo_root().join("library/schema/needs-list.md")).unwrap();
    let at = text.find("## Decided as you (1)").expect("the worked example");
    let example = &text[at..at + 300];
    for money in ["plan", "price", "$", "pay", "buy"] {
        assert!(!example.to_lowercase().contains(money), "the Decided-as-you example mentions {money:?}: {example}");
    }
}

#[test]
fn a_precedent_listed_only_in_the_sources_index_counts() {
    // A receipt or other binary source has no frontmatter, only its row in sources/INDEX.md.
    let fx = fixture();
    start(&fx);
    fx.write(
        "workspace/life/sources/INDEX.md",
        "# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n| `src-2026-11-04-07` | [`receipt.pdf`](receipt.pdf) | 2026-11-04 | a receipt |\n",
    );
    fx.ok(&["needs", "close", "1", "--outcome", "Thursday, like the receipt src-2026-11-04-07", "--as-him"]);
    assert!(log(&fx).contains("as him ["), "{}", log(&fx));
}
