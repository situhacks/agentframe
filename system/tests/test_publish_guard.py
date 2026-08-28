import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from system.hooks import publish_guard as guard


TRACKER = """---
name: Demo
slug: demo
deliverables:
  substack-essay-11:
    status: {status}
    file: posts/essay-v19.md
    substack_draft: {draft_id}
  other-row:
    status: drafting
    file: posts/other-v1.md
---

# Demo
"""


def payload(draft_id=100000001, tool="mcp__substack__update_draft", **fields):
    tool_input = {"draft_id": draft_id}
    tool_input.update(fields)
    return {
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": tool_input,
    }


class PublishGuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        os.makedirs(self.root / "workspace" / "projects" / "demo")
        self.patcher = patch.object(guard, "ROOT", self.root)
        self.patcher.start()

    def tearDown(self):
        self.patcher.stop()
        self.tmp.cleanup()

    def write_tracker(self, status="drafting", draft_id=100000001):
        path = self.root / "workspace" / "projects" / "demo" / "project.md"
        path.write_text(
            TRACKER.format(status=status, draft_id=draft_id), encoding="utf-8"
        )

    # --- the failure this guard exists for -------------------------------
    def test_push_to_draft_row_below_ready_is_denied(self):
        self.write_tracker(status="drafting")
        out = guard.decide(payload(body="the essay"))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        reason = out["hookSpecificOutput"]["permissionDecisionReason"]
        self.assertIn("substack-essay-11", reason)
        self.assertIn("af.py ready", reason)

    def test_title_only_push_is_also_gated(self):
        self.write_tracker(status="drafting")
        self.assertIsNotNone(guard.decide(payload(title="New title")))

    # --- everything the guard must NOT block -----------------------------
    def test_ready_row_passes(self):
        self.write_tracker(status="ready")
        self.assertIsNone(guard.decide(payload(body="the essay")))

    def test_published_row_passes(self):
        self.write_tracker(status="published")
        self.assertIsNone(guard.decide(payload(body="the essay")))

    def test_metadata_only_update_passes(self):
        # An audience or settings change carries no copy, so there is nothing to gate.
        self.write_tracker(status="drafting")
        self.assertIsNone(guard.decide(payload(audience="everyone")))

    def test_untracked_draft_id_passes(self):
        # A draft no tracker claims is not AgentFrame's to gate; doctor backstops it.
        self.write_tracker(status="drafting", draft_id=999)
        self.assertIsNone(guard.decide(payload(body="the essay")))

    def test_create_draft_is_out_of_scope(self):
        # No join key exists before the draft does. Documented gap, not an oversight.
        self.write_tracker(status="drafting")
        p = payload(tool="mcp__substack__create_draft", body="the essay")
        self.assertIsNone(guard.decide(p))

    def test_unrelated_tool_passes(self):
        self.write_tracker(status="drafting")
        self.assertIsNone(guard.decide(payload(tool="Edit", body="x")))

    # --- payload shapes --------------------------------------------------
    def test_float_draft_id_still_joins(self):
        self.write_tracker(status="drafting")
        self.assertIsNotNone(guard.decide(payload(draft_id=100000001.0, body="x")))

    def test_missing_draft_id_fails_open(self):
        self.write_tracker(status="drafting")
        p = {
            "hook_event_name": "PreToolUse",
            "tool_name": "mcp__substack__update_draft",
            "tool_input": {"body": "the essay"},
        }
        self.assertIsNone(guard.decide(p))

    def test_malformed_payload_emits_empty_json(self):
        self.assertEqual(guard.dispatch("not json", []), "{}")

    def test_cursor_native_envelope(self):
        self.write_tracker(status="drafting")
        p = payload(body="the essay")
        p["cursor_version"] = "test"
        out = json.loads(guard.dispatch(json.dumps(p), ["--cursor-native"]))
        self.assertEqual(out["permission"], "deny")
        # The imported Claude twin must no-op rather than deny twice.
        self.assertEqual(json.loads(guard.dispatch(json.dumps(p), [])), {})


if __name__ == "__main__":
    unittest.main()
