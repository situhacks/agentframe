import json
import os
import tempfile
import unittest

from system import af
from system.hooks import ppt_master_guard as guard


def payload(command, cwd, event="PreToolUse"):
    return {
        "hook_event_name": event,
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "cwd": cwd,
    }


def cursor_shell_payload(command, cwd, event="beforeShellExecution"):
    return {
        "hook_event_name": event,
        "cursor_version": "test",
        "command": command,
        "cwd": cwd,
    }


class TestStagingGuard(unittest.TestCase):
    def setUp(self):
        self.skill_dir = os.path.join(guard.ROOT, "system", "skills", "ppt-master")

    def test_init_without_dir_in_skill_cwd_denied(self):
        p = payload("python3 scripts/project_manager.py init demo_deck --format 16:9", self.skill_dir)
        out = guard.decide(p)
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("--dir", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_init_with_dir_inside_skill_denied(self):
        p = payload(
            f'python scripts/project_manager.py init demo --dir "{self.skill_dir}\\projects"',
            guard.ROOT,
        )
        out = guard.decide(p)
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")

    def test_init_with_workspace_dir_allowed(self):
        target = os.path.join(guard.ROOT, "workspace", "projects", "demo", "phase-4-production", "decks")
        p = payload(f'python scripts/project_manager.py init demo --dir "{target}"', self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_init_with_tmp_dir_allowed(self):
        p = payload('python scripts/project_manager.py init demo --dir "C:\\tmp\\decks"', self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_non_init_subcommand_ignored(self):
        p = payload("python scripts/project_manager.py info projects/demo", self.skill_dir)
        self.assertIsNone(guard.decide(p))

    def test_unrelated_command_ignored(self):
        self.assertIsNone(guard.decide(payload("git status", guard.ROOT)))

    def test_cursor_before_shell_uses_native_deny_envelope(self):
        p = cursor_shell_payload(
            "python scripts/project_manager.py init demo",
            self.skill_dir,
        )
        out = json.loads(guard.run(json.dumps(p)))
        self.assertEqual(out["permission"], "deny")
        self.assertIn("--dir", out["agent_message"])

    def test_cursor_imported_claude_twin_is_suppressed(self):
        p = cursor_shell_payload(
            "python scripts/project_manager.py init demo",
            self.skill_dir,
        )
        raw = json.dumps(p)
        self.assertEqual(json.loads(guard.dispatch(raw, [])), {})
        native = json.loads(guard.dispatch(raw, ["--cursor-native"]))
        self.assertEqual(native["permission"], "deny")


class TestConfirmationCommandShapes(unittest.TestCase):
    def setUp(self):
        self.cwd = os.path.join(guard.ROOT, "workspace", "projects", "demo")

    def project_for(self, suffix):
        command = f"python scripts/confirm_ui/server.py decks/demo {suffix}"
        return guard._confirm_project_path(guard._tokens(command), guard.Path(self.cwd))

    def test_vendor_wait_shapes_are_recognized(self):
        # Vendor 52e85a0 accepts only --wait-stage {stage1, final} (default final);
        # the pre-refresh "stage2" wait no longer exists upstream.
        for suffix in (
            "--daemon --wait",
            "--wait-only --wait-stage stage1",
            "--wait-only --wait-stage final",
            "--wait-only",
        ):
            with self.subTest(suffix=suffix):
                self.assertIsNotNone(self.project_for(suffix))

    def test_retired_and_unknown_wait_stages_are_not_confirmation_waits(self):
        for suffix in ("--wait-only --wait-stage stage2", "--wait-only --wait-stage bogus"):
            with self.subTest(suffix=suffix):
                self.assertIsNone(self.project_for(suffix))

    def test_shutdown_and_plain_daemon_are_not_confirmation_waits(self):
        for suffix in ("--shutdown", "--daemon"):
            with self.subTest(suffix=suffix):
                self.assertIsNone(self.project_for(suffix))


class TestExportGuard(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.proj = self.tmp.name
        os.makedirs(os.path.join(self.proj, "svg_output"))

    def tearDown(self):
        self.tmp.cleanup()

    def export_payload(self, extra="", event="PreToolUse"):
        cmd = f'python3 scripts/svg_to_pptx.py "{self.proj}" --no-notes{extra}'
        return payload(cmd, os.path.dirname(self.proj), event=event)

    def test_export_is_not_gated_at_pretooluse(self):
        # The paragraph-split lint was retired at the 52e85a0 refresh: the vendor
        # checker now reports those runs itself, and AgentFrame's "fix before
        # export" stance is an overlay rule. Export must pass through untouched
        # unless a sealed confirmation contract applies.
        self.assertIsNone(guard.decide(self.export_payload()))

    def test_post_export_promotion_reminder(self):
        out = guard.decide(self.export_payload(event="PostToolUse"))
        ctx = out["hookSpecificOutput"]["additionalContext"]
        self.assertIn("promote", ctx.lower())
        self.assertIn("exports/", ctx)

    def test_post_unrelated_command_ignored(self):
        self.assertIsNone(guard.decide(payload("git status", guard.ROOT, event="PostToolUse")))

    def test_cursor_post_tool_uses_native_context_envelope(self):
        p = self.export_payload(event="postToolUse")
        p["cursor_version"] = "test"
        out = json.loads(guard.run(json.dumps(p)))
        self.assertIn("promote", out["additional_context"].lower())


class TestGuardMain(unittest.TestCase):
    def test_main_emits_json_on_deny(self):
        skill_dir = os.path.join(guard.ROOT, "system", "skills", "ppt-master")
        p = payload("python scripts/project_manager.py init demo", skill_dir)
        result = guard.run(json.dumps(p))
        self.assertIsNotNone(result)
        parsed = json.loads(result)
        self.assertEqual(parsed["hookSpecificOutput"]["permissionDecision"], "deny")

    def test_main_silent_on_pass(self):
        self.assertIsNone(guard.run(json.dumps(payload("git status", str(guard.ROOT)))))

    def test_main_silent_on_garbage(self):
        self.assertIsNone(guard.run("not json"))


class TestDoctorStrayDecks(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        self.skill = os.path.join(self.root, "system", "skills", "ppt-master")
        os.makedirs(self.skill)

    def tearDown(self):
        self.tmp.cleanup()

    def test_stray_project_dir_is_issue(self):
        os.makedirs(os.path.join(self.skill, "projects", "demo", "exports"))
        issues = af.ppt_master_stray_issues(self.root)
        self.assertTrue(any("projects" in i for i in issues))

    def test_stray_pptx_is_issue(self):
        target = os.path.join(self.skill, "templates", "decks")
        os.makedirs(target)
        with open(os.path.join(target, "deck_20260709.pptx"), "wb") as f:
            f.write(b"")
        issues = af.ppt_master_stray_issues(self.root)
        self.assertTrue(any("deck_20260709.pptx" in i for i in issues))

    def test_clean_tree_no_issues(self):
        self.assertEqual(af.ppt_master_stray_issues(self.root), [])

    def test_missing_skill_dir_no_issues(self):
        self.assertEqual(af.ppt_master_stray_issues(os.path.join(self.root, "nope")), [])


if __name__ == "__main__":
    unittest.main()
