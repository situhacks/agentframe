import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def load_json(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


class HarnessHookWiringTests(unittest.TestCase):
    def test_claude_reference_wires_all_guards(self):
        config = load_json(".claude/settings.json")
        commands = [
            hook["command"]
            for groups in config["hooks"].values()
            for group in groups
            for hook in group["hooks"]
        ]
        self.assertTrue(any("version_guard.py" in command for command in commands))
        self.assertTrue(any("ppt_master_guard.py" in command for command in commands))
        self.assertTrue(any("autonomy_guard.py" in command for command in commands))
        self.assertTrue(any("publish_guard.py" in command for command in commands))
        self.assertIn("SessionStart", config["hooks"])
        # The publish gate is useless unless it sits on the MCP call that ships copy.
        matchers = [group["matcher"] for group in config["hooks"]["PreToolUse"]]
        self.assertIn("mcp__substack__update_draft", matchers)

    def test_cursor_native_wires_all_guards(self):
        config = load_json(".cursor/hooks.json")
        self.assertEqual(config["version"], 1)
        commands = [
            hook["command"]
            for groups in config["hooks"].values()
            for hook in groups
        ]
        self.assertTrue(any("version_guard.py" in command for command in commands))
        self.assertTrue(any("ppt_master_guard.py" in command for command in commands))
        self.assertTrue(any("autonomy_guard.py" in command for command in commands))
        self.assertTrue(any("publish_guard.py" in command for command in commands))
        self.assertTrue(all("--cursor-native" in command for command in commands))
        self.assertIn("beforeShellExecution", config["hooks"])
        self.assertIn("sessionStart", config["hooks"])
        self.assertIn(
            "confirm_ui",
            config["hooks"]["beforeShellExecution"][0]["matcher"],
        )

    def test_codex_native_wires_all_guards_with_windows_commands(self):
        config = load_json(".codex/hooks.json")
        handlers = [
            hook
            for groups in config["hooks"].values()
            for group in groups
            for hook in group["hooks"]
        ]
        commands = [hook["command"] for hook in handlers]
        self.assertTrue(any("version_guard.py" in command for command in commands))
        self.assertTrue(any("ppt_master_guard.py" in command for command in commands))
        self.assertTrue(any("autonomy_guard.py" in command for command in commands))
        self.assertTrue(all(hook.get("commandWindows") for hook in handlers))
        self.assertTrue(all("git rev-parse --show-toplevel" in command for command in commands))
        self.assertIn("SessionStart", config["hooks"])

    def test_local_harness_files_stay_ignored_by_default(self):
        patterns = (ROOT / ".gitignore").read_text(encoding="utf-8")
        for harness in ("claude", "cursor", "codex"):
            self.assertIn(f".{harness}/*", patterns)
        self.assertIn("!.claude/settings.json", patterns)
        self.assertIn("!.cursor/hooks.json", patterns)
        self.assertIn("!.codex/hooks.json", patterns)

    def test_guard_entrypoints_emit_valid_empty_json_when_no_rule_matches(self):
        payload = json.dumps(
            {
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": "git status"},
                "cwd": str(ROOT),
            }
        )
        for script in ("version_guard.py", "ppt_master_guard.py", "publish_guard.py"):
            result = subprocess.run(
                [sys.executable, str(ROOT / "system" / "hooks" / script)],
                input=payload,
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertEqual(json.loads(result.stdout), {})


if __name__ == "__main__":
    unittest.main()
