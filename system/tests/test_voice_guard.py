import json
import os
import tempfile
import unittest
from unittest.mock import patch

from system import af
from system.hooks import voice_guard as guard

VOICED = "---\nstatus: drafting\nvoice:\n  base_register: informal\n---\n"


def payload(command, event="PreToolUse", tool_response=None):
    p = {"hook_event_name": event, "tool_name": "Bash", "tool_input": {"command": command}, "cwd": str(guard.ROOT)}
    if tool_response is not None:
        p["tool_response"] = tool_response
    return p


def cursor_payload(command, event="beforeShellExecution"):
    return {"hook_event_name": event, "cursor_version": "test", "command": command, "cwd": str(guard.ROOT)}


class GuardFixture(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        projects = os.path.join(self._tmp.name, "workspace", "projects")
        self.cdir = os.path.join(projects, "vg-demo")
        os.makedirs(os.path.join(self.cdir, "essay"))
        self.addCleanup(self._tmp.cleanup)
        for name in ("PROJECTS", "PIPELINE", "STUDIO"):
            p = patch.object(af, name, os.path.join(self._tmp.name, "workspace", name.lower()))
            p.start()
            self.addCleanup(p.stop)
        patch.object(af, "PROJECTS", projects).start()

    def project(self, head_rel, plain_row=False):
        fm = [
            "name: vg-demo", "slug: vg-demo", "schema_version: 2026-07-19-v2", "created_at: 2026-09-01",
            "domain: marketing", "status: active", "current_phase: active", "flow: open-flow",
            "last_activity: 2026-09-01T10:00:00+00:00", "deliverables:", "  essay:",
            "    status: drafting", f"    file: {head_rel}",
        ]
        af.write(os.path.join(self.cdir, "project.md"), "---\n" + "\n".join(fm) + "\n---\n")

    def version(self, n, body, voiced=True):
        text = (VOICED if voiced else "---\nstatus: drafting\n---\n") + body
        af.write(os.path.join(self.cdir, "essay", f"essay-v{n}.md"), text)


class ReadyGateTests(GuardFixture):
    def test_hard_finding_denies_ready(self):
        self.version(1, "Clean.")
        self.version(2, "Your brain quietly checks out.")
        self.project("essay/essay-v2.md")
        out = guard.decide(payload("python system/af.py ready vg-demo essay"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("banned-tic", out["hookSpecificOutput"]["permissionDecisionReason"])
        self.assertIn("AF_VOICE_LINT=skip", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_hyphen_normalised_against_previous_version_denies(self):
        self.version(1, "Even the bad ones - right? More.")
        self.version(2, "Even the bad ones — right? More.")
        self.project("essay/essay-v2.md")
        out = guard.decide(payload("python system/af.py ready vg-demo essay/essay-v2.md"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("hyphen-normalised", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_clean_head_passes_silently(self):
        self.version(1, "A clean sentence. Another one.")
        self.project("essay/essay-v1.md")
        self.assertIsNone(guard.decide(payload("python system/af.py ready vg-demo essay")))

    def test_soft_findings_pass_with_context(self):
        self.version(1, "Not unreasonable, I'd say. Fine.")
        self.project("essay/essay-v1.md")
        out = guard.decide(payload("python system/af.py ready vg-demo essay"))
        self.assertNotIn("permissionDecision", out["hookSpecificOutput"])
        self.assertIn("litotes", out["hookSpecificOutput"]["additionalContext"])

    def test_operator_override_passes(self):
        self.version(1, "Your brain quietly checks out.")
        self.project("essay/essay-v1.md")
        self.assertIsNone(guard.decide(payload("AF_VOICE_LINT=skip python system/af.py ready vg-demo essay")))

    def test_non_voiced_head_is_ignored(self):
        self.version(1, "Your brain quietly checks out.", voiced=False)
        self.project("essay/essay-v1.md")
        self.assertIsNone(guard.decide(payload("python system/af.py ready vg-demo essay")))

    def test_unrelated_and_malformed_commands_pass(self):
        self.assertIsNone(guard.decide(payload("git status")))
        self.assertIsNone(guard.decide(payload("grep -rn ready system/af.py")))
        self.assertIsNone(guard.decide(payload("python system/af.py ready nope essay")))
        self.assertIsNone(guard.decide(payload("python system/af.py doctor vg-demo essay")))


class VersionContextTests(GuardFixture):
    def test_version_lints_the_closed_version_against_the_one_before(self):
        self.version(1, "Even the bad ones - right? More.")
        self.version(2, "Even the bad ones — right? More.")
        self.version(3, "Even the bad ones — right? More.")  # what af version just created
        self.project("essay/essay-v3.md")
        out = guard.decide(payload(
            "python system/af.py version vg-demo essay", event="PostToolUse",
            tool_response={"stdout": "af version: essay/essay-v2.md -> essay/essay-v3.md (head)"},
        ))
        ctx = out["hookSpecificOutput"]["additionalContext"]
        self.assertEqual(out["hookSpecificOutput"]["hookEventName"], "PostToolUse")
        self.assertIn("essay-v2.md vs essay-v1.md", ctx)
        self.assertIn("hyphen-normalised", ctx)
        self.assertIn("essay-v3.md now holds that text", ctx)
        self.assertIn("voice/README.md", ctx)

    def test_refused_version_says_nothing(self):
        self.version(1, "Fine.")
        self.project("essay/essay-v1.md")
        out = guard.decide(payload(
            "python system/af.py version vg-demo essay", event="PostToolUse",
            tool_response={"stdout": "af: ERROR: row 'essay' has no file pointer"},
        ))
        self.assertIsNone(out)

    def test_ready_in_post_tool_use_is_ignored(self):
        self.version(1, "Your brain quietly checks out.")
        self.project("essay/essay-v1.md")
        self.assertIsNone(guard.decide(payload("python system/af.py ready vg-demo essay", event="PostToolUse")))


class HarnessAdaptationTests(GuardFixture):
    def test_cursor_native_deny_shape(self):
        self.version(1, "Your brain quietly checks out.")
        self.project("essay/essay-v1.md")
        out = json.loads(guard.dispatch(json.dumps(cursor_payload("python system/af.py ready vg-demo essay")), ["--cursor-native", "--harness", "cursor"]))
        self.assertEqual(out["permission"], "deny")
        self.assertIn("banned-tic", out["agent_message"])

    def test_cursor_twin_without_native_flag_is_silent(self):
        self.version(1, "Your brain quietly checks out.")
        self.project("essay/essay-v1.md")
        self.assertEqual(guard.dispatch(json.dumps(cursor_payload("python system/af.py ready vg-demo essay")), []), "{}")

    def test_garbage_stdin_is_empty_json(self):
        self.assertEqual(guard.dispatch("not json", ["--harness", "claude"]), "{}")


if __name__ == "__main__":
    unittest.main()
