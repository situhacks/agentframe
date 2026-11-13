//! Session classes, context size from a transcript, the post-tool-use nudge, the pre-compact
//! instructions, and `af session context`.

mod common;
use afcore::reach::{context_size, context_tokens, last_context_tokens, ContextSize};
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
        ("project/acme", SessionClass::Build),
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
    fx.ok(&["session", "register", "project/p", "--session", "s3", "--class", "side"]);
    assert_eq!(fx.read("system/sessions/project/p/brief.md"), "---\nclass: side\n---\njust text\n");
}

#[test]
fn register_class_on_a_crlf_brief_keeps_its_line_endings_and_body() {
    let fx = Fx::new();
    fx.write_bytes("drafts/b.md", b"---\r\ngoal: ship\r\n---\r\nline one\r\nline two\r\n");
    fx.ok(&["session", "register", "life/money", "--session", "s1", "--brief", "drafts/b.md", "--class", "build"]);
    let got = std::fs::read(fx.path("system/sessions/life/money/brief.md")).unwrap();
    assert_eq!(got, b"---\r\ngoal: ship\r\nclass: build\r\n---\r\nline one\r\nline two\r\n".to_vec());
    fx.ok(&["session", "register", "life/money", "--session", "s2", "--class", "side"]);
    let got = std::fs::read(fx.path("system/sessions/life/money/brief.md")).unwrap();
    assert_eq!(got, b"---\r\ngoal: ship\r\nclass: side\r\n---\r\nline one\r\nline two\r\n".to_vec());
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
fn a_jump_past_hard_says_hard_only_and_removing_the_markers_re_arms() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert!(tool_call(&fx, "s-build", "Read", 610_000).unwrap().contains("past this build chat's 600k limit"));
    assert_eq!(tool_call(&fx, "s-build", "Read", 615_000), None, "soft is not said after hard");
    fx.rmtree("system/logs/context-nudge/s-build");
    assert!(tool_call(&fx, "s-build", "Read", 615_000).is_some(), "no markers means nothing fired yet");
}

#[test]
fn what_was_said_is_a_marker_folder_per_session_id() {
    let fx = Fx::new();
    register(&fx, "system/a", "s-a");
    register(&fx, "system/b", "s-b");
    tool_call(&fx, "s-a", "Read", 410_000);
    assert!(fx.isfile("system/logs/context-nudge/s-a/soft"));
    assert!(!fx.exists("system/logs/context-nudge/s-b"));
    assert!(tool_call(&fx, "s-b", "Read", 410_000).is_some(), "another chat has its own markers");
    tool_call(&fx, "s-a", "Read", 705_000);
    let mut names: Vec<String> =
        std::fs::read_dir(fx.path("system/logs/context-nudge/s-a")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["hard-600000", "hard-650000", "hard-700000", "soft"], "a jump claims the lower buckets silently");
}

#[test]
fn a_partial_drop_fires_at_the_next_unclaimed_bucket_and_below_hard_re_arms_hard() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert!(tool_call(&fx, "s-build", "Read", 705_000).unwrap().contains("about 705k"));
    assert_eq!(tool_call(&fx, "s-build", "Read", 620_000), None, "bucket 600k was claimed on the way up");
    assert_eq!(tool_call(&fx, "s-build", "Read", 690_000), None);
    assert!(tool_call(&fx, "s-build", "Read", 750_000).is_some(), "the next unclaimed bucket");
    assert_eq!(tool_call(&fx, "s-build", "Read", 500_000), None, "between soft and hard: the hard markers go");
    assert!(!fx.exists("system/logs/context-nudge/s-build/hard-600000") && fx.isfile("system/logs/context-nudge/s-build/soft"));
    assert!(tool_call(&fx, "s-build", "Read", 600_000).unwrap().contains("past this build chat's 600k limit"), "600k fires again on the way back up");
}

#[test]
fn eight_processes_at_once_say_the_soft_nudge_exactly_once() {
    let fx = Fx::new();
    for round in 0..5 {
        let id = format!("s-par-{round}");
        register(&fx, &format!("system/par{round}"), &id);
        let t = at(&fx, 410_000);
        let payload = json!({"session_id": id, "tool_name": "Read", "transcript_path": t}).to_string();
        let gate = std::sync::Barrier::new(8);
        let outs: Vec<String> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..8)
                .map(|_| {
                    sc.spawn(|| {
                        let mut cmd = fx.af();
                        cmd.args(["hook", "post-tool-use"]).write_stdin(payload.clone());
                        gate.wait();
                        String::from_utf8_lossy(&cmd.output().unwrap().stdout).into_owned()
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let said = outs.iter().filter(|o| nudge_text(o).is_some()).count();
        assert_eq!(said, 1, "round {round}: {outs:?}");
    }
}

fn boundary() -> String {
    json!({"type": "system", "subtype": "compact_boundary", "compactMetadata": {"trigger": "auto", "preTokens": 450000}}).to_string()
}

#[test]
fn a_real_shaped_compaction_re_arms_both_tiers() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    let call = |lines: &[String]| {
        let t = write_transcript(&fx, "t/real.jsonl", lines);
        let (code, out) = hook(&fx, "post-tool-use", &json!({"session_id": "s-build", "tool_name": "Read", "transcript_path": t}));
        assert_eq!(code, 0);
        nudge_text(&out)
    };
    let before = vec![assistant(1, 449_000, 0)];
    assert!(call(&before).is_some());
    assert_eq!(call(&before), None);
    let mut after = before.clone();
    after.extend([boundary(), json!({"type": "user", "message": {"content": "summary"}}).to_string(), assistant(3, 78_000, 2_000)]);
    assert_eq!(call(&after), None, "about 80k after the boundary");
    assert!(call(&before).is_some(), "the next climb to 449k is a new first time");
}

#[test]
fn a_boundary_after_the_last_reply_makes_the_size_unknown_and_nothing_fires() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    let p = write_transcript(&fx, "t/just-compacted.jsonl", &[assistant(1, 482_000, 0), boundary(), json!({"type": "user", "message": {"content": "summary"}}).to_string()]);
    assert_eq!(context_tokens(&p), None, "the 482k line describes the context before the compaction");
    assert_eq!(context_size(&p), ContextSize::AfterCompact);
    let (code, out) = hook(&fx, "post-tool-use", &json!({"session_id": "s-build", "tool_name": "Read", "transcript_path": p}));
    assert_eq!((code, nudge_text(&out)), (0, None), "no nudge on a stale number");
    assert!(!fx.exists("system/logs/context-nudge/s-build"), "and no marker claimed");
    // The first reply after the boundary measures the new context; a boundary before the last reply changes nothing.
    let mut later = std::fs::read_to_string(&p).unwrap();
    later.push_str(&format!("{}\n", assistant(3, 78_000, 1_000)));
    let p = fx.write("t/replied.jsonl", &later);
    assert_eq!(context_tokens(&p), Some(79_003));
    assert!(matches!(context_size(&p), ContextSize::Known(_)));
}

#[test]
fn a_boundary_found_in_text_or_a_sidechain_boundary_is_not_a_compaction() {
    let fx = Fx::new();
    let said = json!({"type": "assistant", "message": {"content": "a compact_boundary line", "usage": {"input_tokens": 5_000}}}).to_string();
    let p = write_transcript(&fx, "t/talk.jsonl", &[assistant(1, 9_000, 0), said]);
    assert_eq!(context_tokens(&p), Some(5_000), "an assistant line that mentions the word is still an assistant line");
    let side = json!({"type": "system", "subtype": "compact_boundary", "isSidechain": true}).to_string();
    let p = write_transcript(&fx, "t/side.jsonl", &[assistant(1, 9_000, 0), side]);
    assert_eq!(context_tokens(&p), Some(9_001));
}

#[test]
fn a_boundary_beyond_the_first_tail_still_makes_the_size_unknown() {
    let fx = Fx::new();
    let filler = json!({"type": "user", "message": {"content": "x".repeat(1000)}}).to_string();
    let mut lines = vec![assistant(7, 700, 70), boundary()];
    lines.extend(std::iter::repeat(filler).take(1500));
    let p = write_transcript(&fx, "t/long-compacted.jsonl", &lines);
    assert_eq!(context_size(&p), ContextSize::AfterCompact);
}

#[test]
fn post_compact_re_arms_at_once_and_prints_nothing() {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    assert!(tool_call(&fx, "s-build", "Read", 650_000).is_some());
    assert!(fx.exists("system/logs/context-nudge/s-build"));
    let (code, out) = hook(&fx, "post-compact", &json!({"session_id": "s-build", "trigger": "auto", "compact_summary": "..."}));
    assert_eq!((code, out.as_str()), (0, ""));
    assert!(!fx.exists("system/logs/context-nudge/s-build"));
    assert!(tool_call(&fx, "s-build", "Read", 650_000).is_some(), "fires again without a measurement below soft");
    fx.mkdir("system/logs/context-nudge/s-other");
    assert_eq!(hook(&fx, "post-compact", &json!({"session_id": "s-other"})), (0, String::new()));
    assert!(fx.exists("system/logs/context-nudge/s-other"), "an unregistered chat is left alone");
    let o = fx.af().args(["hook", "post-compact"]).write_stdin("not json").output().unwrap();
    assert_eq!((o.status.code(), o.stdout.len()), (Some(0), 0));
}

#[test]
fn a_brief_class_override_turns_the_nudge_on_for_a_side_chat() {
    let fx = Fx::new();
    fx.write("drafts/b.md", "---\ngoal: x\n---\n");
    fx.ok(&["session", "register", "life/big", "--session", "s-big", "--brief", "drafts/b.md", "--class", "build"]);
    assert!(tool_call(&fx, "s-big", "Read", 450_000).is_some());
}

#[test]
fn no_nudge_for_side_main_or_unregistered_chats() {
    let fx = Fx::new();
    register(&fx, "af-main", "s-main");
    register(&fx, "life/money", "s-life");
    fx.write("system/sessions/project/acme/brief.md", "---\nclass: side\n---\n");
    register(&fx, "project/acme", "s-proj");
    register(&fx, "worker/w", "s-work");
    for id in ["s-main", "s-life", "s-proj", "s-work", "s-nobody"] {
        let t = at(&fx, 900_000);
        let (code, out) = hook(&fx, "post-tool-use", &json!({"session_id": id, "tool_name": "Read", "transcript_path": t}));
        assert_eq!((code, out.as_str()), (0, "{}"), "{id}");
    }
    assert!(!fx.exists("system/logs/context-nudge"), "no markers for a chat that is never nudged");
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

    fx.rmtree("system/logs/context-nudge/s-build");
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

fn assistant_model(model: &str, total: u64) -> String {
    json!({"type": "assistant", "message": {"model": model, "usage": {"input_tokens": 1, "cache_read_input_tokens": total - 1}}}).to_string()
}

fn context_fixture() -> Fx {
    let fx = Fx::new();
    register(&fx, "system/cut", "s-build");
    register(&fx, "life/money", "s-life");
    register(&fx, "project/none", "s-none");
    write_transcript(&fx, "cfg/projects/p1/s-build.jsonl", &[assistant_model("claude-opus-5-5[1m]", 412_345)]);
    write_transcript(&fx, "cfg/projects/p1/s-life.jsonl", &[assistant_model("claude-sonnet-5-5", 120_001)]);
    fx.write(
        "cfg/settings.json",
        r#"{"modelSettings": {"claude-opus-5-5": {"autoCompactWindow": 400000}, "claude-fable-5-1": {"autoCompactWindow": 700000}}}"#,
    );
    fx
}

fn context_out(fx: &Fx, args: &[&str]) -> String {
    let o = fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).args(args).output().unwrap();
    String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n")
}

#[test]
fn session_context_for_one_chat_prints_class_tokens_window_thresholds_and_auto_compact() {
    let fx = context_fixture();
    assert_eq!(
        context_out(&fx, &["session", "context", "system/cut"]),
        "name: system/cut\nclass: build\ncontext: 412k tokens (412345)\nwindow: 1000k\nsoft: 400k\nhard: 600k\n\
auto-compact: 400k (user settings, modelSettings.claude-opus-5-5)\n\
auto-compact at 400k comes before this chat's 600k hard nudge; raise it with /autocompact in that chat or in user settings\n"
    );
    assert_eq!(
        context_out(&fx, &["session", "context", "life/money"]),
        "name: life/money\nclass: side\ncontext: 120k tokens (120001)\nwindow: 300k\nsoft: none\nhard: none\nauto-compact: default (not set for claude-sonnet-5-5)\n"
    );
    let out = context_out(&fx, &["session", "context", "project/none"]);
    assert!(out.contains("context: unknown") && out.ends_with("auto-compact: unknown\n"), "{out}");
    assert_eq!(fx.af().env("CLAUDE_CONFIG_DIR", fx.path("cfg")).args(["session", "context", "system/ghost"]).output().unwrap().status.code(), Some(1));
}

#[test]
fn session_context_says_the_size_is_unknown_until_the_next_reply_after_a_compaction() {
    let fx = context_fixture();
    write_transcript(&fx, "cfg/projects/p1/s-build.jsonl", &[assistant_model("claude-opus-5-5[1m]", 483_000), boundary()]);
    let out = context_out(&fx, &["session", "context", "system/cut"]);
    assert!(out.contains("context: unknown until its next reply"), "{out}");
    assert!(!out.contains("483"), "{out}");
    assert!(out.ends_with("auto-compact: unknown\n"), "{out}");
    let table = context_out(&fx, &["session", "context"]);
    assert!(table.contains("| system/cut | build | unknown | 400k | 600k | unknown |"), "{table}");
    let mut text = fx.read("cfg/projects/p1/s-build.jsonl");
    text.push_str(&format!("{}\n", assistant_model("claude-opus-5-5[1m]", 79_000)));
    fx.write("cfg/projects/p1/s-build.jsonl", &text);
    assert!(context_out(&fx, &["session", "context", "system/cut"]).contains("context: 79k tokens (79000)"));
}

#[test]
fn an_auto_compact_window_past_the_hard_threshold_adds_no_warning_and_a_missing_settings_file_is_default() {
    let fx = context_fixture();
    write_transcript(&fx, "cfg/projects/p1/s-build.jsonl", &[assistant_model("claude-fable-5-1", 100_000)]);
    let out = context_out(&fx, &["session", "context", "system/cut"]);
    assert!(out.ends_with("auto-compact: 700k (user settings, modelSettings.claude-fable-5-1)\n"), "{out}");
    fx.remove("cfg/settings.json");
    let out = context_out(&fx, &["session", "context", "system/cut"]);
    assert!(out.ends_with("auto-compact: default (not set for claude-fable-5-1)\n"), "{out}");
}

#[test]
fn session_context_never_writes_the_user_settings() {
    let fx = context_fixture();
    let before = fx.read("cfg/settings.json");
    context_out(&fx, &["session", "context"]);
    context_out(&fx, &["session", "context", "system/cut"]);
    assert_eq!(fx.read("cfg/settings.json"), before);
}

#[test]
fn session_context_with_no_name_tables_the_chats_that_have_a_transcript() {
    let fx = context_fixture();
    assert_eq!(
        context_out(&fx, &["session", "context"]),
        "| name | class | context | soft | hard | auto-compact |\n|---|---|---|---|---|---|\n| system/cut | build | 412k | 400k | 600k | 400k |\n| life/money | side | 120k | none | none | default |\n"
    );
}
