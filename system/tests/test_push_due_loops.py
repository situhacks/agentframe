"""Contract tests for `system/tools/push_due_loops.py`, the daily reminder push.

The push renders `af open due --json` as headings and bullets (never a table),
stays silent with nothing due, and logs each real run; no test touches the network.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import push_due_loops  # noqa: E402

LOOPS = [
    {"n": 3, "section": "Waiting on others", "opened": "2026-10-15", "text": "refund from the shop",
     "nudge": "2026-10-25", "every": None, "from": "main", "age_days": 13},
    {"n": 1, "section": "Waiting on the operator", "opened": "2026-10-24", "text": "pay the card",
     "nudge": "2026-10-28", "every": "monthly", "from": "main", "age_days": 4},
]


class RenderTests(unittest.TestCase):
    def test_sections_are_headings_and_loops_are_bullets_with_their_period(self):
        text = push_due_loops.render(LOOPS, dt.date(2026, 10, 28))
        self.assertEqual(
            text,
            "# 2 loops due · 2026-10-28\n\n## Waiting on others\n\n"
            "- **refund from the shop** · nudged 2026-10-25 · open 13d\n\n"
            "## Waiting on the operator\n\n"
            "- **pay the card** · nudged 2026-10-28 · every monthly · open 4d\n",
        )
        self.assertNotIn("|", text)


class MainTests(unittest.TestCase):
    def run_main(self, loops, args):
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            log = Path(d) / "reminders-push.log"
            with mock.patch.object(push_due_loops, "due_loops", return_value=loops), \
                    mock.patch.object(push_due_loops, "LOG", log), \
                    mock.patch.object(push_due_loops.notify, "post") as post, \
                    mock.patch.object(push_due_loops.notify, "load_env",
                                      return_value={"REMINDERS_DISCORD_WEBHOOK": "https://example.invalid/hook"}), \
                    contextlib.redirect_stdout(out):
                code = push_due_loops.main(args)
            logged = log.read_text(encoding="utf-8") if log.exists() else ""
        return code, out.getvalue(), post.call_count, logged

    def test_dry_run_prints_the_due_loops_and_sends_and_logs_nothing(self):
        code, out, posted, logged = self.run_main(LOOPS, ["--dry-run"])
        self.assertEqual(code, 0)
        self.assertIn("**2 loops due", out)
        self.assertIn("- **pay the card** · nudged 2026-10-28 · every monthly", out)
        self.assertEqual((posted, logged), (0, ""))

    def test_nothing_due_is_silent_and_logged(self):
        code, out, posted, logged = self.run_main([], [])
        self.assertEqual((code, out, posted), (0, "", 0))
        self.assertTrue(logged.endswith(" · 0 due\n"), logged)

    def test_a_timeout_logs_a_line_instead_of_a_traceback(self):
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            log = Path(d) / "reminders-push.log"
            with mock.patch.object(push_due_loops, "due_loops", return_value=LOOPS), \
                    mock.patch.object(push_due_loops, "LOG", log), \
                    mock.patch.object(push_due_loops.notify, "post", side_effect=TimeoutError("timed out")), \
                    mock.patch.object(push_due_loops.notify, "load_env",
                                      return_value={"REMINDERS_DISCORD_WEBHOOK": "https://example.invalid/hook"}), \
                    contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
                code = push_due_loops.main([])
            logged = log.read_text(encoding="utf-8")
        self.assertEqual(code, 2)
        self.assertIn(" · 2 due · notify exit 2: notify failed: message 1 of 1: timed out", logged)

    def test_a_real_run_posts_once_and_logs_the_count(self):
        code, _, posted, logged = self.run_main(LOOPS, [])
        self.assertEqual((code, posted), (0, 1))
        self.assertTrue(logged.endswith(" · 2 due · sent\n"), logged)


if __name__ == "__main__":
    unittest.main()
