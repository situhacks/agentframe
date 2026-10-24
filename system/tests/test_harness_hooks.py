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
        self.assertTrue(any("voice_guard.py" in command for command in commands))
        self.assertTrue(any("publish_guard.py" in command for command in commands))
        self.assertIn("SessionStart", config["hooks"])
        # The publish gate is useless unless it sits on the MCP call that ships copy.
        matchers = [group["matcher"] for group in config["hooks"]["PreToolUse"]]
        self.assertIn("mcp__substack__update_draft", matchers)

    def test_guard_entrypoints_emit_valid_empty_json_when_no_rule_matches(self):
        payload = json.dumps(
            {
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": "git status"},
                "cwd": str(ROOT),
            }
        )
        for script in ("version_guard.py", "ppt_master_guard.py", "publish_guard.py", "voice_guard.py"):
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
