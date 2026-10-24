//! `af brief [--json] [--write]`: due loops, open loops by section, chats, backlog,
//! pipeline follow-ups, studio slots, index age, last brief.

mod common;
mod life_fx;
use common::*;
use life_fx::*;

fn brief_fixture() -> Fx {
    let fx = Fx::new();
    fx.write(
        "workspace/life/open.md",
        &format!(
            "## Waiting on the operator\n\n- [ ] {} · renew lease · nudge {} · from: main\n- [ ] {} · call mum · from: main\n\n\
## Waiting on others\n\n- [ ] {} · refund · nudge {} · from: main\n",
            days_ago(6),
            days_ago(1),
            days_ago(2),
            days_ago(3),
            days_ahead(5)
        ),
    );
    write_registry(
        &fx,
        &[
            row("main", "main", "s1", &ts(-5), "live"),
            row("project/acme", "operator", "s2", &ts(-600), "live"),
            row("worker/gone", "worker", "s3", &ts(-900), "ended 2026-10-20T10:00 exit"),
        ],
    );
    fx.write("system/sessions/project/acme/handoff.md", &handoff(&ts(-700), &[]));
    fx.write(
        "system/backlog.md",
        "# Backlog\n\n## BB-2026-10-20-01 — open one\nStatus: open\n\n## BB-2026-10-21-02 — done one\nStatus: done\n",
    );
    let rows = board_row("acme-ai-pm", "applied", &format!("    next_nudge: {}\n", days_ago(2)))
        + &board_row("later-co", "applied", &format!("    next_nudge: {}\n", days_ahead(9)));
    write_board(&fx, &rows);
    write_calendar(
        &fx,
        &(slot_row("soon-post", "planned", &format!("    date: {}\n", days_ahead(3)))
            + &slot_row("far-post", "planned", &format!("    date: {}\n", days_ahead(30)))
            + &slot_row("done-post", "posted", &format!("    date: {}\n", days_ahead(1)))),
    );
    fx.write("workspace/life/briefs/2026-10-01.md", "old brief\n");
    fx.write("workspace/life/briefs/2026-10-12.md", "newer brief\n");
    fx
}

#[test]
fn brief_covers_every_surface() {
    let fx = brief_fixture();
    let out = fx.ok(&["brief"]);
    assert!(out.starts_with(&format!("# Brief {}", today())), "{out}");
    assert!(out.contains("## Due loops (1)\n- [ ] ") || out.contains("## Due loops (1)\r\n- [ ] "), "{out}");
    assert!(out.contains("renew lease") && out.contains("(6d, #1)"));
    assert!(out.contains("## Open loops (3)"));
    assert!(out.contains("- Waiting on the operator: 2"));
    assert!(out.contains("- Waiting on others: 1"));
    assert!(out.contains("## Chats (2)"));
    assert!(out.contains("- main · live · last seen"));
    assert!(out.contains("- project/acme · idle · last seen") && out.contains("handoff current"), "{out}");
    assert!(!out.contains("worker/gone"));
    assert!(out.contains("## Backlog (1 open)") && out.contains("BB-2026-10-20-01") && !out.contains("BB-2026-10-21-02"));
    assert!(out.contains(&format!("- acme-ai-pm: follow-up due since {} (applied)", days_ago(2))));
    assert!(!out.contains("later-co"));
    assert!(out.contains(&format!("- soon-post: {} (planned)", days_ahead(3))));
    assert!(!out.contains("far-post") && !out.contains("done-post"));
    assert!(out.contains("last written brief: 2026-10-12"));
}

#[test]
fn brief_marks_a_handoff_stale_when_files_moved_after_it() {
    let fx = brief_fixture();
    fx.write("system/sessions/project/acme/handoff.md", &handoff("2026-01-01T00:00", &["system/backlog.md"]));
    assert!(fx.ok(&["brief"]).contains("- project/acme · idle · last seen") );
    assert!(fx.ok(&["brief"]).contains("handoff stale"));
    fx.remove("system/sessions/project/acme/handoff.md");
    assert!(fx.ok(&["brief"]).contains("no handoff"));
}

#[test]
fn brief_json_carries_the_same_facts() {
    let fx = brief_fixture();
    let v: serde_json::Value = serde_json::from_str(&fx.ok(&["brief", "--json"])).unwrap();
    assert_eq!(v["date"], today());
    assert_eq!(v["due"][0]["text"], "renew lease");
    assert_eq!(v["open"]["total"], 3);
    assert_eq!(v["chats"].as_array().unwrap().len(), 2);
    assert_eq!(v["chats"][1]["status"], "idle");
    assert_eq!(v["backlog"]["open"], 1);
    assert_eq!(v["pipeline_followups"][0]["slug"], "acme-ai-pm");
    assert_eq!(v["studio_due"][0]["slug"], "soon-post");
    assert_eq!(v["last_brief"], "2026-10-12");
    assert!(v["index"].is_null());
}

#[test]
fn brief_write_saves_todays_brief() {
    let fx = brief_fixture();
    let out = fx.ok(&["brief", "--write"]);
    let rel = format!("workspace/life/briefs/{}.md", today());
    assert!(out.contains(&format!("(saved to {rel})")));
    let saved = fx.read(&rel);
    assert!(saved.starts_with(&format!("# Brief {}", today())));
    assert!(saved.contains("last written brief: 2026-10-12"), "computed before the write");
    assert!(fx.ok(&["brief"]).contains(&format!("last written brief: {}", today())));
}

#[test]
fn brief_on_an_empty_root_still_prints_the_skeleton() {
    let fx = Fx::new();
    let out = fx.ok(&["brief"]);
    assert!(out.contains("## Due loops (0)") && out.contains("## Open loops (0)"));
    assert!(!out.contains("## Chats") && !out.contains("## Backlog") && !out.contains("last written brief"));
}
