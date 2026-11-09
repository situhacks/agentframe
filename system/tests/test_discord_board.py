"""Contract tests for `system/tools/discord_board.py`, the in-place board editor.

No network: a fake Discord answers the bot's HTTP calls in memory. Every id,
token and tag here is invented.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import email.message
import io
import json
import os
import re
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import discord_board  # noqa: E402

TOKEN = "fake-token-never-printed"
TODAY = dt.date(2026, 1, 2)
STAMP = "_Updated 2026-01-02_"


class _Resp:
    def __init__(self, payload):
        self._raw = b"" if payload is None else json.dumps(payload).encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def read(self):
        return self._raw


class FakeDiscord:
    """Just enough of the Discord REST API: messages, pins, forum threads."""

    def __init__(self):
        self.next_id = 900000000000000000
        self.messages: dict[str, dict] = {}
        self.pins: set[str] = set()
        self.threads: dict[str, dict] = {}
        self.calls: list[tuple[str, str, dict | None]] = []
        self.rate_limit_next = 0
        self.headers_seen: list[dict] = []

    def new_id(self) -> str:
        self.next_id += 1
        return str(self.next_id)

    def _error(self, url, code, body):
        hdrs = email.message.Message()
        return urllib.error.HTTPError(url, code, "err", hdrs, io.BytesIO(json.dumps(body).encode()))

    def __call__(self, req, timeout=None):
        method = req.get_method()
        path = req.full_url.removeprefix(discord_board.API)
        body = json.loads(req.data) if req.data else None
        self.calls.append((method, path, body))
        self.headers_seen.append(dict(req.header_items()))
        if self.rate_limit_next:
            self.rate_limit_next -= 1
            raise self._error(req.full_url, 429, {"message": "slow down", "retry_after": 0.25})
        nf = self._error(req.full_url, 404, {"message": "Unknown Message"})
        seg = path.strip("/").split("/")
        if seg[0] != "channels":
            raise nf
        chan = seg[1]
        if len(seg) == 3 and seg[2] == "messages" and method == "POST":
            self._check_open(chan, req)
            mid = self.new_id()
            self.messages[mid] = {"channel": chan, "content": body["content"]}
            return _Resp({"id": mid})
        if len(seg) == 4 and seg[2] == "messages":
            mid = seg[3]
            msg = self.messages.get(mid)
            if not msg or msg["channel"] != chan:
                raise nf
            if method == "PATCH":
                self._check_open(chan, req)
                msg["content"] = body["content"]
                return _Resp({"id": mid})
            if method == "DELETE":
                del self.messages[mid]
                self.pins.discard(mid)
                return _Resp(None)
        if len(seg) == 4 and seg[2] == "pins" and method == "PUT":
            if seg[3] not in self.messages:
                raise nf
            self.pins.add(seg[3])
            return _Resp(None)
        if len(seg) == 3 and seg[2] == "threads" and method == "POST":
            tid = self.new_id()
            self.threads[tid] = {"parent": chan, "name": body["name"], "applied_tags": body.get("applied_tags", []), "archived": False}
            self.messages[tid] = {"channel": tid, "content": body["message"]["content"]}
            return _Resp({"id": tid})
        if len(seg) == 2 and chan in self.threads:
            t = self.threads[chan]
            if method == "GET":
                return _Resp({"id": chan, "name": t["name"], "applied_tags": t["applied_tags"], "thread_metadata": {"archived": t["archived"]}})
            if method == "PATCH":
                if t["archived"] and set(body) != {"archived"}:
                    raise self._error(req.full_url, 400, {"message": "Thread is archived"})
                if "archived" in body:
                    t["archived"] = body["archived"]
                for k in ("name", "applied_tags"):
                    if k in body:
                        t[k] = body[k]
                return _Resp({"id": chan})
        raise nf

    def _check_open(self, chan, req):
        if chan in self.threads and self.threads[chan]["archived"]:
            raise self._error(req.full_url, 400, {"message": "Thread is archived"})

    def channel_contents(self, chan):
        return [m["content"] for mid, m in sorted(self.messages.items()) if m["channel"] == chan]

    def count(self, method, fragment=""):
        return sum(1 for m, p, _ in self.calls if m == method and fragment in p)


BASE_STATE = {
    "guild_id": "100000000000000001",
    "channels": {
        "open": {"id": "200000000000000001", "type": "text", "tags": {}},
        "food": {"id": "200000000000000002", "type": "forum", "tags": {"Plan": "300000000000000001", "Done": "300000000000000002"}},
    },
    "events": {"some-event": "400000000000000001"},
    "boards": {},
    "posts": {},
}


class BoardToolCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.env = self.root / "system" / "tools" / ".env"
        self.state_path = self.root / "workspace" / "life" / "systems" / "discord" / "state.json"
        self.env.parent.mkdir(parents=True)
        self.state_path.parent.mkdir(parents=True)
        self.env.write_text(f"DISCORD_BOT_TOKEN={TOKEN}\n", encoding="utf-8")
        self.state_path.write_text(json.dumps(BASE_STATE, indent=2), encoding="utf-8")
        self.note = self.root / "note.md"
        self.fake = FakeDiscord()
        self.sleeps: list[float] = []
        patcher = mock.patch.dict(os.environ, {"AF_ROOT": str(self.root)})
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(self.tmp.cleanup)

    def run_tool(self, *args, text=None):
        if text is not None:
            self.note.write_text(text, encoding="utf-8")
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = discord_board.main(list(args), opener=self.fake, sleep=self.sleeps.append, today=TODAY)
        self.assertNotIn(TOKEN, out.getvalue() + err.getvalue())
        return code, out.getvalue(), err.getvalue()

    def state(self):
        return json.loads(self.state_path.read_text(encoding="utf-8"))

    @staticmethod
    def long_text(paragraphs):
        return "\n\n".join(f"Paragraph {i}. " + "word " * 150 for i in range(paragraphs))


class BoardTests(BoardToolCase):
    def test_first_run_posts_the_file_and_pins_it(self):
        code, out, _ = self.run_tool("board", "open", str(self.note), text="# Open\n\n- call the bank\n")
        self.assertEqual(code, 0, out)
        ids = self.state()["boards"]["open"]
        self.assertEqual(len(ids), 1)
        self.assertEqual(self.fake.messages[ids[0]]["content"], f"# Open\n\n- call the bank\n\n{STAMP}")
        self.assertEqual(self.fake.pins, {ids[0]})
        headers = self.fake.headers_seen[0]
        self.assertEqual(headers.get("User-agent"), "AgentFrame-board (local, 1.0)")
        self.assertEqual(headers.get("Authorization"), f"Bot {TOKEN}")

    def test_a_second_run_edits_the_same_message_and_does_not_repin(self):
        self.run_tool("board", "open", str(self.note), text="- one\n")
        first = self.state()["boards"]["open"]
        self.fake.calls.clear()
        code, _, _ = self.run_tool("board", "open", str(self.note), text="- one\n- two\n")
        self.assertEqual(code, 0)
        self.assertEqual(self.state()["boards"]["open"], first)
        self.assertEqual(self.fake.count("POST"), 0)
        self.assertEqual(self.fake.count("PUT"), 0)
        self.assertEqual(self.fake.count("PATCH"), 1)
        self.assertEqual(self.fake.messages[first[0]]["content"], f"- one\n- two\n\n{STAMP}")

    def test_the_board_grows_and_shrinks_across_messages(self):
        self.run_tool("board", "open", str(self.note), text="short\n")
        first = self.state()["boards"]["open"][0]
        self.run_tool("board", "open", str(self.note), text=self.long_text(6))
        ids = self.state()["boards"]["open"]
        self.assertGreaterEqual(len(ids), 3)
        self.assertEqual(ids[0], first)
        for mid in ids:
            self.assertLessEqual(len(self.fake.messages[mid]["content"]), discord_board.LIMIT)
        self.assertTrue(self.fake.messages[ids[-1]]["content"].endswith(STAMP))
        self.assertEqual(self.fake.count("PUT"), 1)

        self.fake.calls.clear()
        self.run_tool("board", "open", str(self.note), text="short again\n")
        self.assertEqual(self.state()["boards"]["open"], [first])
        self.assertEqual(self.fake.count("DELETE"), len(ids) - 1)
        self.assertEqual(self.fake.channel_contents("200000000000000001"), [f"short again\n\n{STAMP}"])

    def test_a_board_deleted_by_hand_is_posted_fresh_and_repinned(self):
        self.run_tool("board", "open", str(self.note), text=self.long_text(4))
        old = self.state()["boards"]["open"]
        self.assertGreaterEqual(len(old), 2)
        del self.fake.messages[old[0]]
        self.fake.pins.discard(old[0])
        code, _, _ = self.run_tool("board", "open", str(self.note), text=self.long_text(4))
        self.assertEqual(code, 0)
        new = self.state()["boards"]["open"]
        self.assertEqual(len(new), len(old))
        self.assertNotIn(old[0], new)
        self.assertNotIn(old[1], self.fake.messages, "later messages are replaced to keep the order")
        self.assertEqual(self.fake.pins, {new[0]})

    def test_a_rate_limit_waits_retry_after_and_retries(self):
        self.fake.rate_limit_next = 1
        code, _, _ = self.run_tool("board", "open", str(self.note), text="- x\n")
        self.assertEqual(code, 0)
        self.assertEqual(len(self.sleeps), 1)
        self.assertAlmostEqual(self.sleeps[0], 0.35)
        self.assertEqual(len(self.state()["boards"]["open"]), 1)

    def test_board_refuses_a_forum_channel(self):
        code, _, err = self.run_tool("board", "food", str(self.note), text="x\n")
        self.assertEqual(code, 2)
        self.assertIn("forum channel", err)
        self.assertEqual(self.fake.calls, [])

    def test_a_missing_token_sends_nothing_and_prints_no_secret(self):
        self.env.write_text("", encoding="utf-8")
        with mock.patch.dict(os.environ, {"DISCORD_BOT_TOKEN": ""}):
            code, _, err = self.run_tool("board", "open", str(self.note), text="x\n")
        self.assertEqual(code, 2)
        self.assertIn("DISCORD_BOT_TOKEN", err)
        self.assertEqual(self.fake.calls, [])

    def test_errors_hide_ids(self):
        self.assertEqual(discord_board.redact("PATCH /channels/123456789012345678/messages/9"), "PATCH /channels/<id>/messages/9")


class PostTests(BoardToolCase):
    def test_forum_create_then_edit_with_a_tag_and_title_change(self):
        code, out, err = self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", "--tag", "Plan", text="Try the miso.\n")
        self.assertEqual(code, 0, err)
        item = self.state()["posts"]["food"]["ramen"]
        tid = item["thread"]
        self.assertEqual(item["messages"], [])
        self.assertEqual(self.fake.threads[tid]["applied_tags"], ["300000000000000001"])
        self.assertEqual(self.fake.threads[tid]["parent"], "200000000000000002")
        self.assertEqual(self.fake.messages[tid]["content"], f"Try the miso.\n\n{STAMP}")

        self.fake.calls.clear()
        code, out, err = self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen, tried", "--tag", "done", text="Miso was great.\n")
        self.assertEqual(code, 0, err)
        self.assertIn(("PATCH", f"/channels/{tid}/messages/{tid}", {"content": f"Miso was great.\n\n{STAMP}"}), self.fake.calls)
        self.assertEqual(self.fake.threads[tid]["name"], "Ramen, tried")
        self.assertEqual(self.fake.threads[tid]["applied_tags"], ["300000000000000002"])
        self.assertEqual(self.fake.count("POST"), 0)
        item = self.state()["posts"]["food"]["ramen"]
        self.assertEqual((item["thread"], item["title"], item["tags"]), (tid, "Ramen, tried", ["Done"]))

    def test_an_unchanged_title_and_no_tag_leave_the_thread_alone(self):
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", "--tag", "Plan", text="a\n")
        tid = self.state()["posts"]["food"]["ramen"]["thread"]
        self.fake.calls.clear()
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", text="b\n")
        self.assertNotIn("PATCH", [m for m, p, _ in self.fake.calls if p == f"/channels/{tid}"])
        self.assertEqual(self.fake.threads[tid]["applied_tags"], ["300000000000000001"])

    def test_an_archived_thread_is_unarchived_before_the_edit(self):
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", text="a\n")
        tid = self.state()["posts"]["food"]["ramen"]["thread"]
        self.fake.threads[tid]["archived"] = True
        code, _, err = self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", text="b\n")
        self.assertEqual(code, 0, err)
        self.assertFalse(self.fake.threads[tid]["archived"])
        self.assertEqual(self.fake.messages[tid]["content"], f"b\n\n{STAMP}")

    def test_a_long_post_overflows_into_thread_messages_and_shrinks_back(self):
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", text=self.long_text(5))
        item = self.state()["posts"]["food"]["ramen"]
        tid = item["thread"]
        self.assertGreaterEqual(len(item["messages"]), 1)
        for mid in item["messages"]:
            self.assertEqual(self.fake.messages[mid]["channel"], tid)
        extra = item["messages"]
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", text="short\n")
        self.assertEqual(self.state()["posts"]["food"]["ramen"]["messages"], [])
        for mid in extra:
            self.assertNotIn(mid, self.fake.messages)
        self.assertEqual(self.fake.channel_contents(tid), [f"short\n\n{STAMP}"])

    def test_an_unknown_tag_fails_and_lists_the_valid_ones(self):
        before = self.state_path.read_bytes()
        code, _, err = self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", "--tag", "Someday", text="a\n")
        self.assertEqual(code, 2)
        self.assertIn("Someday", err)
        self.assertIn("Valid tags: Done, Plan", err)
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state_path.read_bytes(), before)


class DryRunAndStateTests(BoardToolCase):
    def test_dry_run_sends_nothing_and_writes_nothing(self):
        self.run_tool("board", "open", str(self.note), text="- one\n")
        before = self.state_path.read_bytes()
        mtime = self.state_path.stat().st_mtime_ns
        self.fake.calls.clear()
        self.env.write_text("", encoding="utf-8")  # a dry run needs no token
        for args in (("board", "open", str(self.note)),
                     ("post", "food", "ramen", str(self.note), "--title", "Ramen", "--tag", "Plan")):
            code, out, _ = self.run_tool(*args, "--dry-run", text=self.long_text(4))
            self.assertEqual(code, 0)
            self.assertIn("dry run: nothing sent, state unchanged.", out)
            self.assertIn(STAMP, out)
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state_path.read_bytes(), before)
        self.assertEqual(self.state_path.stat().st_mtime_ns, mtime)
        self.assertIsNone(re.search(r"\d{15,}", out), "a dry run prints no ids")

    def test_other_state_keys_are_preserved(self):
        self.run_tool("board", "open", str(self.note), text="- one\n")
        self.run_tool("post", "food", "ramen", str(self.note), "--title", "Ramen", "--tag", "Plan", text="a\n")
        state = self.state()
        for key in ("guild_id", "channels", "events"):
            self.assertEqual(state[key], BASE_STATE[key])
        self.assertEqual(set(state), set(BASE_STATE))

    def test_the_env_channel_id_wins_over_state(self):
        self.env.write_text(f"DISCORD_BOT_TOKEN={TOKEN}\nOPEN_DISCORD_CHANNEL_ID=200000000000000999\n", encoding="utf-8")
        self.run_tool("board", "open", str(self.note), text="x\n")
        self.assertEqual(self.fake.calls[0][1], "/channels/200000000000000999/messages")


if __name__ == "__main__":
    unittest.main()
