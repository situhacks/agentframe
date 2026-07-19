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


def cursor_payload(path, tool="Write", **tool_input):
    return {
        "hook_event_name": "preToolUse",
        "cursor_version": "test",
        "tool_name": tool,
        "tool_input": {"file_path": str(path), **tool_input},
        "cwd": str(guard.ROOT),
    }


def codex_payload(patch_text, cwd):
    return {
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch_text},
        "cwd": str(cwd),
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

    def write_version(self, number, status="drafting", body="body\n"):
        path = self.folder / f"copy-v{number}.md"
        path.write_text(
            f"---\nstatus: {status}\nlast_updated: 2026-07-14\n---\n\n{body}",
            encoding="utf-8",
        )
        return path

    def test_current_drafting_head_allows_surgical_edit(self):
        head = self.write_version(2)
        self.write_version(1)
        self.assertIsNone(guard.decide(payload(head)))

    def test_full_write_over_nonempty_head_body_is_denied(self):
        head = self.write_version(1)
        out = guard.decide(payload(head, tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        reason = out["hookSpecificOutput"]["permissionDecisionReason"]
        self.assertIn("surgical Edit", reason)
        self.assertIn("af.py version", reason)

    def test_full_write_into_empty_scaffold_head_is_allowed(self):
        head = self.write_version(1, body="")
        self.assertIsNone(guard.decide(payload(head, tool="Write")))

    def test_edit_to_nonempty_head_body_stays_allowed(self):
        head = self.write_version(1)
        self.assertIsNone(guard.decide(payload(head, tool="Edit")))

    def test_prior_version_is_denied(self):
        prior = self.write_version(1)
        self.write_version(2)
        out = guard.decide(payload(prior))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("immutable prior version", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_ready_head_full_write_is_denied_as_clobber(self):
        head = self.write_version(1, status="ready")
        out = guard.decide(payload(head, tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("Full-file Write would clobber", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_ready_head_allows_surgical_edit(self):
        head = self.write_version(1, status="ready")
        self.assertIsNone(guard.decide(payload(head, tool="Edit")))

    def test_published_head_denies_surgical_edit(self):
        head = self.write_version(1, status="published")
        out = guard.decide(payload(head, tool="Edit"))
        self.assertIn("published and immutable", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_unversioned_published_assembly_denies_surgical_edit(self):
        assembly = self.folder / "post-FINAL.md"
        assembly.write_text(
            "---\nstatus: published\nlast_updated: 2026-07-19\n---\n\nbody\n",
            encoding="utf-8",
        )
        out = guard.decide(payload(assembly, tool="Edit"))
        reason = out["hookSpecificOutput"]["permissionDecisionReason"]
        self.assertIn("published and immutable", reason)
        self.assertIn("new tracked edition", reason)

    def test_direct_new_version_is_denied(self):
        self.write_version(1)
        out = guard.decide(payload(self.folder / "copy-v2.md", tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("Do not hand-create", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_direct_v1_is_denied(self):
        out = guard.decide(payload(self.folder / "copy-v1.md", tool="Write"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("af.py draft", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_delete_of_versioned_head_is_denied(self):
        head = self.write_version(1)
        out = guard.decide(payload(head, tool="Delete"))
        self.assertIn("Do not delete", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_cursor_native_deny_uses_cursor_envelope(self):
        head = self.write_version(1, status="published")
        out = json.loads(guard.run(json.dumps(cursor_payload(head))))
        self.assertEqual(out["permission"], "deny")
        self.assertIn("published", out["agent_message"])

    def test_cursor_imported_claude_twin_is_suppressed(self):
        head = self.write_version(1, status="published")
        raw = json.dumps(cursor_payload(head))
        self.assertEqual(json.loads(guard.dispatch(raw, [])), {})
        native = json.loads(guard.dispatch(raw, ["--cursor-native"]))
        self.assertEqual(native["permission"], "deny")

    def test_cursor_edit_shaped_write_allows_drafting_head(self):
        head = self.write_version(1)
        p = cursor_payload(head, old_string="old", new_string="new")
        self.assertIsNone(guard.decide(p))

    def test_codex_patch_allows_current_drafting_head_update(self):
        head = self.write_version(1)
        rel = head.relative_to(self.root).as_posix()
        patch_text = f"*** Begin Patch\n*** Update File: {rel}\n@@\n-old\n+new\n*** End Patch"
        self.assertIsNone(guard.decide(codex_payload(patch_text, self.root)))

    def test_codex_patch_denies_prior_version_update(self):
        prior = self.write_version(1)
        self.write_version(2)
        rel = prior.relative_to(self.root).as_posix()
        patch_text = f"*** Begin Patch\n*** Update File: {rel}\n@@\n-old\n+new\n*** End Patch"
        out = guard.decide(codex_payload(patch_text, self.root))
        self.assertIn("immutable prior version", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_codex_patch_denies_direct_add(self):
        target = self.folder / "copy-v1.md"
        rel = target.relative_to(self.root).as_posix()
        patch_text = f"*** Begin Patch\n*** Add File: {rel}\n+body\n*** End Patch"
        out = guard.decide(codex_payload(patch_text, self.root))
        self.assertIn("af.py draft", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_codex_patch_denies_version_delete(self):
        head = self.write_version(1)
        rel = head.relative_to(self.root).as_posix()
        patch_text = f"*** Begin Patch\n*** Delete File: {rel}\n*** End Patch"
        out = guard.decide(codex_payload(patch_text, self.root))
        self.assertIn("Do not delete", out["hookSpecificOutput"]["permissionDecisionReason"])

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
