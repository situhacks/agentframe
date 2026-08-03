import contextlib
import io
import json
import os
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

from system import af
from system.hooks import autonomy_guard as guard


PROJECT_MD = """---
name: Hook Test
slug: hook-test
schema_version: 2026-07-19-v2
created_at: 2026-08-03
domain: project-mgmt
status: active
current_phase: active
flow: open-flow
last_activity: 2026-08-03T10:00:00+00:00
deliverables:
---

# Hook Test
"""


class AutonomyGuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.projects = self.root / "workspace" / "projects"
        self.project = self.projects / "hook-test"
        (self.project / "knowledge").mkdir(parents=True)
        (self.project / "project.md").write_text(PROJECT_MD, encoding="utf-8")
        self.patches = [
            patch.object(af, "ROOT", str(self.root)),
            patch.object(af, "PROJECTS", str(self.projects)),
            patch.object(guard, "ROOT", self.root),
        ]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        self.addCleanup(self.tmp.cleanup)

    def quiet(self, fn, args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return fn(args)

    def start_run(self):
        self.quiet(
            af.cmd_autonomy_init,
            types.SimpleNamespace(project="hook-test", run_id="work", level="assisted"),
        )
        path = self.project / "knowledge" / "autonomy" / "work.md"
        fm, body = af.split_fm(path.read_text(encoding="utf-8"), str(path))
        values = {
            "goal": af.yaml_quote("Build"),
            "done_when": af.yaml_quote("Checks pass"),
            "context_sources": "[project.md]",
            "allowed_paths": "[workspace/projects/hook-test/build]",
            "verification": "[pytest]",
        }
        for key, value in values.items():
            fm = af.set_scalar(fm, key, value, str(path))
        path.write_text(af.join_fm(fm, body), encoding="utf-8")
        self.quiet(
            af.cmd_autonomy_start,
            types.SimpleNamespace(
                project="hook-test",
                run_id="work",
                resume_reason=None,
                session_binding="codex:session-123",
            ),
        )
        return path

    def test_exact_valid_session_receives_complete_pin(self):
        self.start_run()
        context = guard.context_for(
            {"session_id": "session-123", "hook_event_name": "SessionStart"},
            "codex",
        )
        self.assertIn("AGENTFRAME BOUNDED AUTONOMY PIN", context)
        self.assertIn('"goal": "Build"', context)
        self.assertIn("prohibited_effects", context)

    def test_unbound_session_receives_key_without_authority(self):
        context = guard.context_for(
            {"session_id": "session-999", "hook_event_name": "SessionStart"},
            "codex",
        )
        self.assertIn("codex:session-999", context)
        self.assertIn("not autonomy authority", context)
        self.assertNotIn("BOUNDED AUTONOMY PIN", context)

    def test_drift_suppresses_authority_pin(self):
        path = self.start_run()
        fm, body = af.split_fm(path.read_text(encoding="utf-8"), str(path))
        fm = af.set_scalar(fm, "goal", af.yaml_quote("Mutated"), str(path))
        path.write_text(af.join_fm(fm, body), encoding="utf-8")
        context = guard.context_for(
            {"session_id": "session-123", "hook_event_name": "SessionStart"},
            "codex",
        )
        self.assertIn("contract hash drift", context)
        self.assertNotIn("BOUNDED AUTONOMY PIN", context)

    def test_cursor_dispatch_uses_native_context_envelope(self):
        raw = json.dumps(
            {
                "hook_event_name": "sessionStart",
                "cursor_version": "test",
                "session_id": "cursor-session",
            }
        )
        self.assertEqual(json.loads(guard.dispatch(raw, ["--harness", "codex"])), {})
        output = json.loads(
            guard.dispatch(raw, ["--cursor-native", "--harness", "cursor"])
        )
        self.assertIn("cursor:cursor-session", output["additional_context"])


if __name__ == "__main__":
    unittest.main()
