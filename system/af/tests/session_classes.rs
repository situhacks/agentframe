//! Session classes, context size from a transcript, the post-tool-use nudge, the pre-compact
//! instructions, and `af session context`.

mod common;
use afcore::reach::{context_tokens, last_context_tokens};
use afcore::sessions::{brief_class, class_of, default_class, SessionClass, CLASS_LIMITS};
use common::*;
use serde_json::{json, Value};

// ------------------------------------------------------------------ fixtures

/// One transcript line: an assistant record whose usage sums to input + read + create.
fn assistant(input: u64, read: u64, create: u64) -> String {
    json!({
        "type": "assistant",
        "message": {"role": "assistant", "usage": {"input_tokens": input, "cache_read_input_tokens": read,
            "cache_creation_input_tokens": create, "output_tokens": 77}}
    })
    .to_string()
}

fn sidechain(total: u64) -> String {
    json!({"type": "assistant", "isSidechain": true, "message": {"usage": {"input_tokens": total}}}).to_string()
}

fn write_transcript(fx: &Fx, rel: &str, lines: &[String]) -> String {
    fx.write(rel, &format!("{}\n", lines.join("\n")))
}

/// A transcript whose last context is `total` tokens.
fn at(fx: &Fx, total: u64) -> String {
    write_transcript(fx, "t/current.jsonl", &[assistant(1, 0, 0), assistant(2, total - 12, 10)])
}

fn register(fx: &Fx, name: &str, id: &str) {
    fx.ok(&["session", "register", name, "--session", id]);
}

fn hook(fx: &Fx, event: &str, payload: &Value) -> (i32, String) {
    let o = fx.af().args(["hook", event]).write_stdin(payload.to_string()).output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The hook's additionalContext, or None for `{}`.
fn nudge_text(out: &str) -> Option<String> {
    let v: Value = serde_json::from_str(out).unwrap();
    v.pointer("/hookSpecificOutput/additionalContext").map(|t| t.as_str().unwrap().to_string())
}

/// One post-tool-use call for `id` after writing a transcript of `total` tokens.
fn tool_call(fx: &Fx, id: &str, tool: &str, total: u64) -> Option<String> {
    let t = at(fx, total);
    let (code, out) = hook(fx, "post-tool-use", &json!({"session_id": id, "tool_name": tool, "transcript_path": t}));
    assert_eq!(code, 0);
    nudge_text(&out)
}

// ------------------------------------------------------------------ class

#[test]
fn class_defaults_follow_the_name() {
    for (name, want) in [
        ("af-main", SessionClass::Main),
        ("main", SessionClass::Main),
        ("life/finances", SessionClass::Side),
        ("project/acme", SessionClass::Side),
        ("worker/w1", SessionClass::Side),
        ("system/cut", SessionClass::Build),
    ] {
        assert_eq!(default_class(name), want, "{name}");
    }
}

#[test]
fn the_class_table_holds_each_window_and_threshold() {
    let rows: Vec<_> = CLASS_LIMITS.iter().map(|l| (l.class.name(), l.window, l.soft, l.hard)).collect();
    assert_eq!(
        rows,
        [
            ("main", 350_000, None, None),
            ("side", 300_000, None, None),
            ("build", 1_000_000, Some(400_000), Some(600_000)),
        ]
    );
}

#[test]
fn a_brief_class_line_overrides_the_default_and_a_bad_one_is_ignored() {
    let fx = Fx::new();
    let ctx = fx.ctx();
    assert_eq!(class_of(&ctx, "system/cut"), SessionClass::Build);
    fx.write("system/sessions/system/cut/brief.md", "---\ngoal: x\nclass: side\nwindow: 200k\n---\nbody\n");
    assert_eq!(brief_class(&ctx, "system/cut"), Some(SessionClass::Side));
    assert_eq!(class_of(&ctx, "system/cut"), SessionClass::Side);
    fx.write("system/sessions/system/cut/brief.md", "---\nclass: huge\n---\n");
    assert_eq!(class_of(&ctx, "system/cut"), SessionClass::Build);
}

#[test]
fn register_class_writes_the_brief_line_and_updates_it() {
    let fx = Fx::new();
    fx.write("drafts/b.md", "---\ngoal: ship\nwindow: 200k\n---\nbody\n");
    fx.ok(&["session", "register", "life/money", "--session", "s1", "--brief", "drafts/b.md", "--class", "build"]);
    let brief = fx.read("system/sessions/life/money/brief.md");
    assert_eq!(brief, "---\ngoal: ship\nwindow: 200k\nclass: build\n---\nbody\n");
    assert_eq!(class_of(&fx.ctx(), "life/money"), SessionClass::Build);
    // Re-registering with an existing brief and no --brief rewrites the one line.
    fx.ok(&["session", "register", "life/money", "--session", "s2", "--class", "main"]);
    assert_eq!(fx.read("system/sessions/life/money/brief.md"), "---\ngoal: ship\nwindow: 200k\nclass: main\n---\nbody\n");
    // A brief with no frontmatter gains a block.
    fx.write("system/sessions/project/p/brief.md", "just text\n");
    fx.ok(&["session", "register", "project/p", "--session", "s3", "--class", "build"]);
    assert_eq!(fx.read("system/sessions/project/p/brief.md"), "---\nclass: build\n---\njust text\n");
}

#[test]
fn register_class_refuses_an_unknown_value_and_a_missing_brief_before_writing_anything() {
    let fx = Fx::new();
    fx.write("drafts/b.md", "---\ngoal: x\n---\n");
    let err = fx.refused(&["session", "register", "system/cut", "--session", "s1", "--brief", "drafts/b.md", "--class", "huge"]);
    assert!(err.contains("main, side, build"), "{err}");
    let err = fx.refused(&["session", "register", "system/cut", "--session", "s1", "--class", "build"]);
    assert!(err.contains("--brief") && err.contains("brief"), "{err}");
    assert!(!fx.exists("system/sessions/registry.md"));
    assert!(!fx.exists("system/sessions/system/cut"));
}

#[test]
fn list_shows_the_class() {
    let fx = Fx::new();
    register(&fx, "af-main", "s0");
    register(&fx, "system/cut", "s1");
    let out = fx.af().env("PATH", "").args(["session", "list"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("| handoff | class |"), "{text}");
    assert!(text.contains("| system/sessions/af-main/handoff.md | main |"), "{text}");
    assert!(text.contains("| system/sessions/system/cut/handoff.md | build |"), "{text}");
    let out = fx.af().env("PATH", "").args(["session", "list", "--json"]).output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[1]["class"], "build");
}

// ------------------------------------------------------------------ context size

#[test]
fn context_is_the_last_main_chain_assistant_usage() {
    let fx = Fx::new();
    let p = write_transcript(
        &fx,
        "t/a.jsonl",
        &[assistant(5, 1000, 100), assistant(3, 20_000, 500), sidechain(900_000), "{\"type\":\"user\"}".into()],
    );
    assert_eq!(context_tokens(&p), Some(20_503));
}

#[test]
fn a_torn_final_line_is_tolerated() {
    let fx = Fx::new();
    let whole = assistant(1, 2, 3);
    let p = fx.write("t/torn.jsonl", &format!("{whole}\n{{\"type\":\"assistant\",\"message\":{{\"usage\":{{\"input_tok"));
    assert_eq!(context_tokens(&p), Some(6));
}

#[test]
fn no_usage_line_is_none() {
    let fx = Fx::new();
    let p = write_transcript(&fx, "t/none.jsonl", &["{\"type\":\"user\"}".into(), sidechain(5), "{\"type\":\"assistant\",\"message\":{}}".into()]);
    assert_eq!(context_tokens(&p), None);
    assert_eq!(context_tokens(&fx.p("t/missing.jsonl")), None);
    assert_eq!(last_context_tokens(""), None);
}

#[test]
fn an_all_zero_usage_stub_is_not_a_size() {
    let fx = Fx::new();
    let p = write_transcript(&fx, "t/stub.jsonl", &[assistant(10, 90_000, 0), assistant(0, 0, 0)]);
    assert_eq!(context_tokens(&p), Some(90_010));
}

#[test]
fn a_usage_line_beyond_the_first_tail_is_found_in_the_larger_one() {
    let fx = Fx::new();
    let filler = json!({"type": "user", "message": {"content": "x".repeat(1000)}}).to_string();
    let mut lines = vec![assistant(7, 700, 70)];
    lines.extend(std::iter::repeat(filler).take(1500)); // about 1.5 MB after it
    let p = write_transcript(&fx, "t/long.jsonl", &lines);
    assert_eq!(context_tokens(&p), Some(777));
}

// ------------------------------------------------------------------ the nudge

#[test]
fn the_nudge_sequence_on_a_build_chat() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert_eq!(tool_call(&fx, "s-build", "Read", 399_000), None);
    let soft = tool_call(&fx, "s-build", "Read", 400_000).expect("soft at 400k");
    assert_eq!(
        soft,
        "Context: about 400k tokens; this build chat's soft limit is 400k. At the next stable point, bring handoff.md and the plan up to date so a fresh context could continue, then consider compacting. Not urgent: finish work that benefits from what is loaded."
    );
    assert_eq!(tool_call(&fx, "s-build", "Read", 420_000), None);
    let hard = tool_call(&fx, "s-build", "Read", 600_000).expect("hard at 600k");
    assert_eq!(
        hard,
        "Context: about 600k tokens, past this build chat's 600k limit. At the next natural stopping point: write the current state, decisions and next step into handoff.md and the plan, file anything that exists only in this conversation, then compact before starting new large work. If you cannot compact yourself, tell the operator in one line that this chat is ready for /compact."
    );
    assert_eq!(tool_call(&fx, "s-build", "Read", 620_000), None);
    assert!(tool_call(&fx, "s-build", "Read", 650_000).expect("hard again").contains("about 650k tokens"));
    assert_eq!(tool_call(&fx, "s-build", "Read", 150_000), None, "a compaction re-arms");
    assert!(tool_call(&fx, "s-build", "Read", 400_000).expect("soft again").contains("soft limit"));
}

#[test]
fn a_jump_past_hard_says_hard_only_and_a_corrupt_state_file_means_nothing_fired() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert!(tool_call(&fx, "s-build", "Read", 610_000).unwrap().contains("past this build chat's 600k limit"));
    assert_eq!(tool_call(&fx, "s-build", "Read", 615_000), None, "soft is not said after hard");
    fx.write("system/logs/context-nudge-s-build.json", "{not json");
    assert!(tool_call(&fx, "s-build", "Read", 615_000).is_some());
}

#[test]
fn the_state_is_one_small_file_per_session_id() {
    let fx = Fx::new();
    register(&fx, "system/a", "s-a");
    register(&fx, "system/b", "s-b");
    tool_call(&fx, "s-a", "Read", 410_000);
    assert!(fx.isfile("system/logs/context-nudge-s-a.json"));
    assert!(!fx.exists("system/logs/context-nudge-s-b.json"));
    assert!(tool_call(&fx, "s-b", "Read", 410_000).is_some(), "another chat has its own state");
    let leftovers: Vec<_> = std::fs::read_dir(fx.path("system/logs")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
    assert!(leftovers.is_empty());
}

#[test]
fn a_brief_class_override_turns_the_nudge_on_for_a_side_chat() {
    let fx = Fx::new();
    fx.write("drafts/b.md", "---\ngoal: x\n---\n");
    fx.ok(&["session", "register", "project/big", "--session", "s-big", "--brief", "drafts/b.md", "--class", "build"]);
    assert!(tool_call(&fx, "s-big", "Read", 450_000).is_some());
}

#[test]
fn no_nudge_for_side_main_or_unregistered_chats() {
    let fx = Fx::new();
    register(&fx, "af-main", "s-main");
    register(&fx, "life/money", "s-life");
    register(&fx, "project/acme", "s-proj");
    register(&fx, "worker/w", "s-work");
    for id in ["s-main", "s-life", "s-proj", "s-work", "s-nobody"] {
        let t = at(&fx, 900_000);
        let (code, out) = hook(&fx, "post-tool-use", &json!({"session_id": id, "tool_name": "Read", "transcript_path": t}));
        assert_eq!((code, out.as_str()), (0, "{}"), "{id}");
    }
    let state = std::fs::read_dir(fx.path("system/logs")).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().starts_with("context-nudge"));
    assert!(!state, "no state file for a chat that is never nudged");
}

#[test]
fn an_unreadable_transcript_or_a_missing_session_id_is_silent() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    let cfg = fx.path("cfg");
    for payload in [
        json!({"session_id": "s-build", "tool_name": "Read", "transcript_path": fx.p("t/gone.jsonl")}),
        json!({"tool_name": "Read"}),
        json!({"session_id": "s-build", "tool_name": "Read", "agent_id": "sub-1", "transcript_path": at(&fx, 900_000)}),
    ] {
        let o = fx.af().env("CLAUDE_CONFIG_DIR", &cfg).args(["hook", "post-tool-use"]).write_stdin(payload.to_string()).output().unwrap();
        assert_eq!(o.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&o.stdout), "{}");
    }
}

#[test]
fn without_a_transcript_path_the_transcript_is_found_by_session_id() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    write_transcript(&fx, "cfg/projects/p1/s-build.jsonl", &[assistant(1, 449_000, 0)]);
    let o = fx
        .af()
        .env("CLAUDE_CONFIG_DIR", fx.path("cfg"))
        .args(["hook", "post-tool-use"])
        .write_stdin(json!({"session_id": "s-build", "tool_name": "Edit"}).to_string())
        .output()
        .unwrap();
    assert!(nudge_text(&String::from_utf8_lossy(&o.stdout)).unwrap().contains("about 449k tokens"));
}

#[test]
fn a_non_shell_tool_gets_the_nudge_but_none_of_the_shell_only_work() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    fx.mkdir("proj/svg_output");
    fx.write_bytes("proj/exports/deck_20260827_120000.pptx", b"pptx");
    let command = format!("python3 scripts/svg_to_pptx.py \"{}\" --no-notes", fx.p("proj"));
    let t = at(&fx, 500_000);
    let mut payload = json!({
        "session_id": "s-build", "hook_event_name": "PostToolUse", "tool_name": "Bash",
        "tool_input": {"command": command}, "cwd": fx.root(), "transcript_path": t,
    });
    let (_, out) = hook(&fx, "post-tool-use", &payload);
    let both = nudge_text(&out).unwrap();
    assert!(both.to_lowercase().contains("promote") && both.contains("Context: about 500k"), "Bash gets the export reminder and the nudge: {both}");

    fx.remove("system/logs/context-nudge-s-build.json");
    payload["tool_name"] = json!("Read");
    let (code, out) = hook(&fx, "post-tool-use", &payload);
    assert_eq!(code, 0);
    let only = nudge_text(&out).unwrap();
    assert!(only.starts_with("Context: about 500k") && !only.to_lowercase().contains("promote"), "Read gets the nudge alone: {only}");
}

// ------------------------------------------------------------------ pre-compact

#[test]
fn pre_compact_prints_the_standing_instructions_for_a_registered_chat() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    let (code, out) = hook(&fx, "pre-compact", &json!({"session_id": "s-build", "trigger": "auto", "custom_instructions": null}));
    assert_eq!(code, 0);
    assert!(out.contains("\"system/cut\"") && out.contains("role builder"), "{out}");
    assert!(out.contains("system/sessions/system/cut/"), "{out}");
    for want in ["brief.md", "handoff.md", "plan", "current step", "decisions", "open questions", "standing instructions", "files touched", "Drop:", "dead ends"] {
        assert!(out.to_lowercase().contains(&want.to_lowercase()), "{want}: {out}");
    }
    assert!(!out.trim_start().starts_with('{'), "plain text, not JSON");
    assert!(out.len() < 10_000);
}

#[test]
fn pre_compact_is_silent_for_an_unregistered_chat_and_a_bad_payload() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert_eq!(hook(&fx, "pre-compact", &json!({"session_id": "s-other"})), (0, String::new()));
    let o = fx.af().args(["hook", "pre-compact"]).write_stdin("not json").output().unwrap();
    assert_eq!((o.status.code(), o.stdout.len()), (Some(0), 0));
}

// ------------------------------------------------------------------ af session context

fn context_fixture() -> Fx {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    register(&fx, "life/money", "s-life");
    register(&fx, "project/none", "s-none");
    write_transcript(&fx, "cfg/projects/p1/s-build.jsonl", &[assistant(1, 412_000, 344)]);
    write_transcript(&fx, "cfg/projects/p1/s-life.jsonl", &[assistant(1, 120_000, 0)]);
    fx
}

#[test]
fn session_context_for_one_chat_prints_class_tokens_window_and_thresholds() {
    let fx = context_fixture();
    let run = |args: &[&str]| fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).args(args).output().unwrap();
    let out = String::from_utf8_lossy(&run(&["session", "context", "system/cut"]).stdout).replace("\r\n", "\n");
    assert_eq!(out, "name: system/cut\nclass: build\ncontext: 412k tokens (412345)\nwindow: 1000k\nsoft: 400k\nhard: 600k\n");
    let out = String::from_utf8_lossy(&run(&["session", "context", "life/money"]).stdout).replace("\r\n", "\n");
    assert_eq!(out, "name: life/money\nclass: side\ncontext: 120k tokens (120001)\nwindow: 300k\nsoft: none\nhard: none\n");
    let out = String::from_utf8_lossy(&run(&["session", "context", "project/none"]).stdout).into_owned();
    assert!(out.contains("context: unknown"), "{out}");
    assert_eq!(run(&["session", "context", "system/ghost"]).status.code(), Some(1));
}

#[test]
fn session_context_with_no_name_tables_the_chats_that_have_a_transcript() {
    let fx = context_fixture();
    let o = fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).args(["session", "context"]).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n");
    assert_eq!(
        out,
        "| name | class | context | soft | hard |\n|---|---|---|---|---|\n| system/cut | build | 412k | 400k | 600k |\n| life/money | side | 120k | none | none |\n"
    );
}
