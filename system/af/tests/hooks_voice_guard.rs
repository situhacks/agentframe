//! Port of system/tests/test_voice_guard.py.

mod common;

use afcore::hooks::{voice_guard, HookCtx};
use afcore::pyjson::{dumps, PyVal};
use common::Fx;
use serde_json::{json, Value};

const VOICED: &str = "---\nstatus: drafting\nvoice:\n  base_register: informal\n---\n";

struct G {
    fx: Fx,
    h: HookCtx,
}

impl G {
    fn new() -> G {
        let fx = Fx::new();
        fx.write("AGENTS.md", "# root\n");
        fx.mkdir("system");
        fx.mkdir("workspace/projects/vg-demo/essay");
        let h = HookCtx::new(&fx.root());
        G { fx, h }
    }

    fn project(&self, head_rel: &str) {
        let fm = [
            "name: vg-demo",
            "slug: vg-demo",
            "schema_version: 2026-07-19-v2",
            "created_at: 2026-09-01",
            "domain: marketing",
            "status: active",
            "current_phase: active",
            "last_activity: 2026-09-01T10:00:00+00:00",
            "deliverables:",
            "  essay:",
            "    status: drafting",
            &format!("    file: {head_rel}"),
        ];
        self.fx.write("workspace/projects/vg-demo/project.md", &format!("---\n{}\n---\n", fm.join("\n")));
    }

    fn version(&self, n: u32, body: &str, voiced: bool) {
        let fm = if voiced { VOICED } else { "---\nstatus: drafting\n---\n" };
        self.fx.write(&format!("workspace/projects/vg-demo/essay/essay-v{n}.md"), &format!("{fm}{body}"));
    }

    fn payload(&self, command: &str, event: &str, tool_response: Option<Value>) -> Value {
        let mut p = json!({"hook_event_name": event, "tool_name": "Bash", "tool_input": {"command": command}, "cwd": self.h.root});
        if let Some(r) = tool_response {
            p["tool_response"] = r;
        }
        p
    }

    fn decide(&self, command: &str) -> Option<Value> {
        self.decide_ev(command, "PreToolUse", None)
    }

    fn decide_ev(&self, command: &str, event: &str, resp: Option<Value>) -> Option<Value> {
        voice_guard::decide(&self.h, &self.payload(command, event, resp)).map(|v: PyVal| serde_json::from_str(&dumps(&v)).unwrap())
    }
}

fn out(v: &Option<Value>) -> &Value {
    &v.as_ref().expect("expected hook output")["hookSpecificOutput"]
}

// ReadyGateTests

#[test]
fn test_hard_finding_denies_ready() {
    let g = G::new();
    g.version(1, "Clean.", true);
    g.version(2, "Your brain quietly checks out.", true);
    g.project("essay/essay-v2.md");
    let o = g.decide("python system/af.py ready vg-demo essay");
    assert_eq!(out(&o)["permissionDecision"], "deny");
    let r = out(&o)["permissionDecisionReason"].as_str().unwrap();
    assert!(r.contains("banned-tic"));
    assert!(r.contains("AF_VOICE_LINT=skip"));
}

#[test]
fn test_hyphen_normalised_against_previous_version_denies() {
    let g = G::new();
    g.version(1, "Even the bad ones - right? More.", true);
    g.version(2, "Even the bad ones — right? More.", true);
    g.project("essay/essay-v2.md");
    let o = g.decide("python system/af.py ready vg-demo essay/essay-v2.md");
    assert_eq!(out(&o)["permissionDecision"], "deny");
    assert!(out(&o)["permissionDecisionReason"].as_str().unwrap().contains("hyphen-normalised"));
}

#[test]
fn test_clean_head_passes_silently() {
    let g = G::new();
    g.version(1, "A clean sentence. Another one.", true);
    g.project("essay/essay-v1.md");
    assert!(g.decide("python system/af.py ready vg-demo essay").is_none());
}

#[test]
fn test_soft_findings_pass_with_context() {
    let g = G::new();
    g.version(1, "Not unreasonable, I'd say. Fine.", true);
    g.project("essay/essay-v1.md");
    let o = g.decide("python system/af.py ready vg-demo essay");
    assert!(out(&o).get("permissionDecision").is_none());
    assert!(out(&o)["additionalContext"].as_str().unwrap().contains("litotes"));
}

#[test]
fn test_operator_override_passes() {
    let g = G::new();
    g.version(1, "Your brain quietly checks out.", true);
    g.project("essay/essay-v1.md");
    assert!(g.decide("AF_VOICE_LINT=skip python system/af.py ready vg-demo essay").is_none());
}

#[test]
fn test_non_voiced_head_is_ignored() {
    let g = G::new();
    g.version(1, "Your brain quietly checks out.", false);
    g.project("essay/essay-v1.md");
    assert!(g.decide("python system/af.py ready vg-demo essay").is_none());
}

#[test]
fn test_unrelated_and_malformed_commands_pass() {
    let g = G::new();
    assert!(g.decide("git status").is_none());
    assert!(g.decide("grep -rn ready system/af.py").is_none());
    assert!(g.decide("python system/af.py ready nope essay").is_none());
    assert!(g.decide("python system/af.py doctor vg-demo essay").is_none());
}

// VersionContextTests

#[test]
fn test_version_lints_the_closed_version_against_the_one_before() {
    let g = G::new();
    g.version(1, "Even the bad ones - right? More.", true);
    g.version(2, "Even the bad ones — right? More.", true);
    g.version(3, "Even the bad ones — right? More.", true);
    g.project("essay/essay-v3.md");
    let o = g.decide_ev(
        "python system/af.py version vg-demo essay",
        "PostToolUse",
        Some(json!({"stdout": "af version: essay/essay-v2.md -> essay/essay-v3.md (head)"})),
    );
    let ctx = out(&o)["additionalContext"].as_str().unwrap().to_string();
    assert_eq!(out(&o)["hookEventName"], "PostToolUse");
    assert!(ctx.contains("essay-v2.md vs essay-v1.md"));
    assert!(ctx.contains("hyphen-normalised"));
    assert!(ctx.contains("essay-v3.md now holds that text"));
    assert!(ctx.contains("voice/README.md"));
    assert!(ctx.contains("voice bundle vg-demo essay --tier core"));
    assert!(ctx.contains("already in your context"));
}

#[test]
fn test_refused_version_says_nothing() {
    let g = G::new();
    g.version(1, "Fine.", true);
    g.project("essay/essay-v1.md");
    let o = g.decide_ev(
        "python system/af.py version vg-demo essay",
        "PostToolUse",
        Some(json!({"stdout": "af: ERROR: row 'essay' has no file pointer"})),
    );
    assert!(o.is_none());
}

#[test]
fn test_ready_in_post_tool_use_is_ignored() {
    let g = G::new();
    g.version(1, "Your brain quietly checks out.", true);
    g.project("essay/essay-v1.md");
    assert!(g.decide_ev("python system/af.py ready vg-demo essay", "PostToolUse", None).is_none());
}

// HarnessAdaptationTests

#[test]
fn test_garbage_stdin_is_empty_json() {
    let g = G::new();
    let o = g.fx.af().args(["hook", "pre-tool-use"]).write_stdin("not json").output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&o.stdout), "{}");
}

// Beyond the pytest file: the command shapes the tokenizer settles.

#[test]
fn test_af_py_must_follow_an_interpreter() {
    let g = G::new();
    g.version(1, "Your brain quietly checks out.", true);
    g.project("essay/essay-v1.md");
    assert!(g.decide("cat system/af.py ready vg-demo essay").is_none());
    assert!(g.decide("python3.exe C:/x/system/af.py ready vg-demo essay").is_some());
    assert!(g.decide("system/af.py ready vg-demo essay").is_some());
    // posix shlex eats unquoted backslashes, so a bare Windows path never matches
    assert!(g.decide("python C:\\x\\system\\af.py ready vg-demo essay").is_none());
    assert!(g.decide("python \"C:\\x\\system\\af.py\" ready vg-demo essay").is_some());
}
