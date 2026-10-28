"""Contract tests for `system/tools/notify.py`, the shared Discord notifier.

Nothing here touches the network: the tests pin the credential names a channel
reads, the message split, and that a dry run or a missing credential sends nothing.
"""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import notify  # noqa: E402


class TargetTests(unittest.TestCase):
    def test_a_channel_prefixes_the_channel_and_webhook_names(self):
        env = {"DISCORD_BOT_TOKEN": "t", "REMINDERS_DISCORD_CHANNEL_ID": "42"}
        self.assertEqual(notify.target_from_env(env, "reminders"), {"token": "t", "channel": "42"})
        self.assertIsNone(notify.target_from_env(env, "groceries"))
        self.assertIsNone(notify.target_from_env(env))

    def test_the_bot_wins_over_a_webhook_and_a_webhook_serves_alone(self):
        env = {"DISCORD_BOT_TOKEN": "t", "DISCORD_CHANNEL_ID": "1", "DISCORD_WEBHOOK": "https://x"}
        self.assertEqual(notify.target_from_env(env), {"token": "t", "channel": "1"})
        self.assertEqual(notify.target_from_env({"GROCERIES_DISCORD_WEBHOOK": "https://x"}, "groceries"), {"webhook": "https://x"})


class ComposeTests(unittest.TestCase):
    def test_the_first_heading_becomes_a_bold_title(self):
        self.assertEqual(notify.compose("# Due today\n\n- pay rent\n", None), ["**Due today**\n\n- pay rent"])

    def test_a_long_text_splits_under_the_limit_on_paragraphs(self):
        text = "\n\n".join(f"- item {i} " + "x" * 80 for i in range(60))
        parts = notify.compose(text, "List")
        self.assertGreater(len(parts), 1)
        self.assertTrue(all(len(p) <= notify.LIMIT for p in parts))
        self.assertEqual("\n\n".join(parts), "**List**\n\n" + text)


class MainTests(unittest.TestCase):
    def run_main(self, args: list[str]) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err), mock.patch.object(notify, "post") as post:
            code = notify.main(args)
        self.posted = post.call_count
        return code, out.getvalue(), err.getvalue()

    def test_dry_run_prints_and_sends_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "n.md"
            f.write_text("# Due\n\n- pay rent\n", encoding="utf-8")
            code, out, _ = self.run_main([str(f), "--dry-run", "--channel", "reminders"])
        self.assertEqual(code, 0)
        self.assertIn("--- message 1 of 1", out)
        self.assertIn("**Due**", out)
        self.assertEqual(self.posted, 0)

    def test_missing_credentials_name_the_variables_and_send_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "n.md"
            f.write_text("- x\n", encoding="utf-8")
            with mock.patch.dict(notify.os.environ, {}, clear=True):
                code, _, err = self.run_main([str(f), "--channel", "reminders", "--env", str(Path(d) / ".env")])
        self.assertEqual(code, 2)
        self.assertIn("REMINDERS_DISCORD_CHANNEL_ID", err)
        self.assertIn("REMINDERS_DISCORD_WEBHOOK", err)
        self.assertEqual(self.posted, 0)

    def test_credentials_from_the_env_file_post_each_part(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "n.md"
            f.write_text("- x\n", encoding="utf-8")
            env = Path(d) / ".env"
            env.write_text("REMINDERS_DISCORD_WEBHOOK=https://example.invalid/hook\n", encoding="utf-8")
            code, out, _ = self.run_main([str(f), "--channel", "reminders", "--env", str(env)])
        self.assertEqual(code, 0)
        self.assertIn("sent 1 of 1 message(s)", out)
        self.assertEqual(self.posted, 1)

    def test_a_timeout_is_reported_the_rest_still_go_and_the_exit_is_non_zero(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "n.md"
            f.write_text("\n\n".join("- " + "x" * 1000 for _ in range(3)), encoding="utf-8")
            env = Path(d) / ".env"
            env.write_text("REMINDERS_DISCORD_WEBHOOK=https://example.invalid/hook\n", encoding="utf-8")
            out, err = io.StringIO(), io.StringIO()
            effects = [None, TimeoutError("timed out"), None]
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err), \
                    mock.patch.object(notify, "post", side_effect=effects) as post, \
                    mock.patch.object(notify.time, "sleep"):
                code = notify.main([str(f), "--channel", "reminders", "--env", str(env)])
        self.assertEqual(code, 2)
        self.assertEqual(post.call_count, 3, "a failed message does not stop the next")
        self.assertIn("notify failed: message 2 of 3: timed out", err.getvalue())
        self.assertIn("sent 2 of 3 message(s)", out.getvalue())


if __name__ == "__main__":
    unittest.main()
