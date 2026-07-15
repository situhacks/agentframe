import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from system.hooks import version_guard as guard


def payload(path, tool="Edit"):
    return {
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": {"file_path": str(path)},
        "cwd": str(guard.ROOT),
    }


class VersionGuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.patch_root = patch.object(guard, "ROOT", self.root)
        self.patch_root.start()
        self.addCleanup(self.patch_root.stop)
        self.addCleanup(self.tmp.cleanup)
        self.folder = self.root / "workspace" / "projects" / "demo" / "draft"
        self.folder.mkdir(parents=True)

    def write_version(self, number, status="drafting"):
        path = self.folder / f"copy-v{number}.md"
        path.write_text(
            f"---\nstatus: {status}\nlast_updated: 2026-07-14\n---\n\nbody\n",
            encoding="utf-8",
        )
        return path

    def test_current_drafting_head_allows_surgical_edit(self):
        head = self.write_version(2)
        self.write_version(1)
        self.assertIsNone(guard.decide(payload(head)))

    def test_prior_version_is_denied(self):
        prior = self.write_version(1)
        self.write_version(2)
        out = guard.decide(payload(prior))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("immutable prior version", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_locked_head_is_denied(self):
        head = self.write_version(1, status="locked")
        out = guard.decide(payload(head, tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("unlock/version", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_direct_new_version_is_denied(self):
        self.write_version(1)
        out = guard.decide(payload(self.folder / "copy-v2.md", tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("Do not hand-create", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_direct_v1_is_denied(self):
        out = guard.decide(payload(self.folder / "copy-v1.md", tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("af.py draft", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_non_versioned_and_non_project_files_pass(self):
        self.assertIsNone(guard.decide(payload(self.folder / "post-FINAL.md")))
        self.assertIsNone(guard.decide(payload(self.root / "README-v1.md")))

    def test_invalid_payload_fails_open(self):
        self.assertIsNone(guard.run("not json"))
        self.assertIsNone(guard.run(json.dumps({"tool_name": "Edit"})))


class VersionGuardWiringTests(unittest.TestCase):
    def test_tracked_claude_settings_wires_guard(self):
        settings = json.loads((guard.ROOT / ".claude" / "settings.json").read_text(encoding="utf-8"))
        entries = settings["hooks"]["PreToolUse"]
        commands = [hook["command"] for entry in entries for hook in entry["hooks"]]
        self.assertTrue(any("version_guard.py" in command for command in commands))


if __name__ == "__main__":
    unittest.main()
