"""Contract tests for `system/tools/notify.py`, the shared Discord notifier.

Nothing here touches the network: the tests pin the credential names a channel
reads, the message split, and that a dry run or a missing credential sends nothing.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
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


class TestFlagTests(unittest.TestCase):
    """--test sends one line and reports the result; the HTTP call is always stubbed."""

    def run_test_flag(self, args: list[str], env_text: str | None, response=None, effect=None):
        out, err = io.StringIO(), io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            env = Path(d) / ".env"
            if env_text is not None:
                env.write_text(env_text, encoding="utf-8")
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err),                     mock.patch.dict(notify.os.environ, {}, clear=True),                     mock.patch.object(notify, "post", return_value=response, side_effect=effect) as post,                     mock.patch.object(notify, "test_text", return_value="AgentFrame notify test 2026-11-13 09:30"):
                code = notify.main(["-", "--test", "--env", str(env), *args])
        return code, out.getvalue(), err.getvalue(), post

    def test_the_bot_path_reports_the_message_id_from_the_response(self):
        code, out, _, post = self.run_test_flag(
            ["--channel", "reminders"], "DISCORD_BOT_TOKEN=t\nREMINDERS_DISCORD_CHANNEL_ID=42\n", response={"id": "9001", "content": "x"})
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "delivered, message id 9001")
        post.assert_called_once_with({"token": "t", "channel": "42"}, "AgentFrame notify test 2026-11-13 09:30")

    def test_a_webhook_reports_delivered(self):
        code, out, _, post = self.run_test_flag(
            ["--channel", "reminders"], "REMINDERS_DISCORD_WEBHOOK=https://example.invalid/hook\n", response={})
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "delivered")
        self.assertEqual(post.call_count, 1)

    def test_a_missing_target_exits_2_names_the_key_and_sends_nothing(self):
        code, out, err, post = self.run_test_flag(["--channel", "reminders"], "DISCORD_BOT_TOKEN=t\nOPEN_DISCORD_CHANNEL_ID=7\n")
        self.assertEqual(code, 2)
        self.assertIn("REMINDERS_DISCORD_CHANNEL_ID", err)
        self.assertEqual(out, "")
        self.assertEqual(post.call_count, 0)

    def test_a_bot_reply_without_an_id_is_a_failure_not_delivered(self):
        code, out, err, _ = self.run_test_flag(
            ["--channel", "reminders"], "DISCORD_BOT_TOKEN=t\nREMINDERS_DISCORD_CHANNEL_ID=42\n", response={})
        self.assertEqual(code, 2)
        self.assertNotIn("delivered", out)
        self.assertIn("no message id", err)

    def test_a_network_error_exits_2_and_says_nothing_was_delivered(self):
        code, out, err, _ = self.run_test_flag(
            ["--channel", "reminders"], "REMINDERS_DISCORD_WEBHOOK=https://example.invalid/hook\n", effect=TimeoutError("timed out"))
        self.assertEqual(code, 2)
        self.assertNotIn("delivered", out)
        self.assertIn("notify failed: timed out", err)

    def test_dry_run_prints_the_test_line_and_sends_nothing(self):
        code, out, _, post = self.run_test_flag(["--channel", "reminders", "--dry-run"], None)
        self.assertEqual(code, 0)
        self.assertIn("AgentFrame notify test 2026-11-13 09:30", out)
        self.assertEqual(post.call_count, 0)

    def test_the_line_names_the_minute(self):
        self.assertEqual(notify.test_text(dt.datetime(2026, 11, 13, 9, 5, 59)), "AgentFrame notify test 2026-11-13 09:05")

    def test_a_file_is_still_required_without_test(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as cm:
            notify.main(["--channel", "reminders"])
        self.assertEqual(cm.exception.code, 2)

    def test_post_returns_the_response_json_and_an_empty_dict_for_an_empty_body(self):
        class Resp:
            def __init__(self, body: bytes):
                self.body = body
            def read(self):
                return self.body
            def __enter__(self):
                return self
            def __exit__(self, *a):
                return False
        with mock.patch.object(notify.urllib.request, "urlopen", return_value=Resp(b'{"id": "5"}')):
            self.assertEqual(notify.post({"token": "t", "channel": "1"}, "hi"), {"id": "5"})
        with mock.patch.object(notify.urllib.request, "urlopen", return_value=Resp(b"")):
            self.assertEqual(notify.post({"webhook": "https://example.invalid/h"}, "hi"), {})


if __name__ == "__main__":
    unittest.main()
