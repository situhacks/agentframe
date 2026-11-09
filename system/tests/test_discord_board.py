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
import time
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
OPEN_ID = "200000000000000001"
FOOD_ID = "200000000000000002"
PLAN_ID, DONE_ID = "300000000000000001", "300000000000000002"
NO_MENTIONS = {"parse": []}


class _Resp:
    def __init__(self, payload, raw=None):
        self._raw = raw if raw is not None else (b"" if payload is None else json.dumps(payload).encode("utf-8"))

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def read(self):
        return self._raw


class FakeDiscord:
    """Just enough of the Discord REST API: messages, pins, forum threads.

    `fail` holds one-shot rules (method, path regex, status, body) checked before
    routing; `raw` holds one-shot (method, path regex, bytes) non-JSON answers;
    `new_pin_status` makes the current pin route answer that status."""

    def __init__(self):
        self.next_id = 900000000000000000
        self.channels: set[str] = {OPEN_ID, FOOD_ID}
        self.messages: dict[str, dict] = {}
        self.pins: set[str] = set()
        self.threads: dict[str, dict] = {}
        self.calls: list[tuple[str, str, dict | None]] = []
        self.headers_seen: list[dict] = []
        self.rate_limit: list[float] = []
        self.fail: list[tuple[str, str, int, dict]] = []
        self.raw: list[tuple[str, str, bytes]] = []
        self.new_pin_status: int | None = None
        self.normalise_name = False

    def new_id(self) -> str:
        self.next_id += 1
        return str(self.next_id)

    @staticmethod
    def _error(url, status, body):
        return urllib.error.HTTPError(url, status, "err", email.message.Message(), io.BytesIO(json.dumps(body).encode()))

    def _take(self, rules, method, path):
        for rule in rules:
            if rule[0] == method and re.search(rule[1], path):
                rules.remove(rule)
                return rule
        return None

    def __call__(self, req, timeout=None):
        method = req.get_method()
        url = req.full_url
        path = url.removeprefix(discord_board.API)
        body = json.loads(req.data) if req.data else None
        self.calls.append((method, path, body))
        self.headers_seen.append(dict(req.header_items()))
        if self.rate_limit:
            raise self._error(url, 429, {"message": "slow down", "retry_after": self.rate_limit.pop(0)})
        rule = self._take(self.fail, method, path)
        if rule:
            raise self._error(url, rule[2], rule[3])
        rule = self._take(self.raw, method, path)
        if rule:
            return _Resp(None, raw=rule[2])
        unknown_message = self._error(url, 404, {"message": "Unknown Message", "code": 10008})
        unknown_channel = self._error(url, 404, {"message": "Unknown Channel", "code": 10003})
        seg = path.strip("/").split("/")
        if seg[0] != "channels" or seg[1] not in self.channels:
            raise unknown_channel
        chan = seg[1]
        if seg[2:] == ["messages"] and method == "POST":
            self._check_open(chan, url)
            mid = self.new_id()
            self.messages[mid] = {"channel": chan, "content": body["content"]}
            return _Resp({"id": mid})
        if len(seg) == 4 and seg[2] == "messages":
            msg = self.messages.get(seg[3])
            if not msg or msg["channel"] != chan:
                raise unknown_message
            if method == "PATCH":
                self._check_open(chan, url)
                msg["content"] = body["content"]
                return _Resp({"id": seg[3]})
            if method == "DELETE":
                del self.messages[seg[3]]
                self.pins.discard(seg[3])
                return _Resp(None)
        is_new_pin = len(seg) == 5 and seg[2:4] == ["messages", "pins"]
        is_old_pin = len(seg) == 4 and seg[2] == "pins"
        if (is_new_pin or is_old_pin) and method == "PUT":
            if is_new_pin and self.new_pin_status:
                raise self._error(url, self.new_pin_status, {"message": "no such route", "code": 0})
            if seg[-1] not in self.messages:
                raise unknown_message
            self.pins.add(seg[-1])
            return _Resp(None)
        if seg[2:] == ["threads"] and method == "POST":
            tid = self.new_id()
            name = body["name"].lower() if self.normalise_name else body["name"]
            self.threads[tid] = {"parent": chan, "name": name, "applied_tags": body.get("applied_tags", []), "archived": False}
            self.channels.add(tid)
            self.messages[tid] = {"channel": tid, "content": body["message"]["content"]}
            return _Resp({"id": tid})
        if len(seg) == 2 and chan in self.threads:
            t = self.threads[chan]
            if method == "GET":
                return _Resp({"id": chan, "name": t["name"], "applied_tags": t["applied_tags"], "thread_metadata": {"archived": t["archived"]}})
            if method == "PATCH":
                if t["archived"] and set(body) != {"archived"}:
                    raise self._error(url, 400, {"message": "Thread is archived"})
                if "archived" in body:
                    t["archived"] = body["archived"]
                if "name" in body:
                    t["name"] = body["name"].lower() if self.normalise_name else body["name"]
                if "applied_tags" in body:
                    t["applied_tags"] = body["applied_tags"]
                return _Resp({"id": chan})
        raise unknown_channel

    def _check_open(self, chan, url):
        if chan in self.threads and self.threads[chan]["archived"]:
            raise self._error(url, 400, {"message": "Thread is archived"})

    def delete_thread(self, tid):
        self.channels.discard(tid)
        self.threads.pop(tid)
        for mid in [m for m, v in self.messages.items() if v["channel"] == tid]:
            del self.messages[mid]

    def channel_contents(self, chan):
        return [m["content"] for _, m in sorted(self.messages.items()) if m["channel"] == chan]

    def count(self, method, pattern=""):
        return sum(1 for m, p, _ in self.calls if m == method and re.search(pattern, p))


BASE_STATE = {
    "guild_id": "100000000000000001",
    "channels": {
        "open": {"id": OPEN_ID, "type": "text", "tags": {}},
        "food": {"id": FOOD_ID, "type": "forum", "tags": {"Plan": PLAN_ID, "Done": DONE_ID}},
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
        self.lock = self.state_path.with_name("state.json.lock")
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
        self.assertFalse(self.lock.exists(), "a run always releases its lock")
        return code, out.getvalue(), err.getvalue()

    def board(self, text):
        return self.run_tool("board", "open", str(self.note), text=text)

    def post(self, text, *extra, title="Ramen"):
        return self.run_tool("post", "food", "ramen", str(self.note), "--title", title, *extra, text=text)

    def state(self):
        return json.loads(self.state_path.read_text(encoding="utf-8"))

    def board_ids(self):
        return self.state()["boards"]["open"]["messages"]

    @staticmethod
    def long_text(paragraphs):
        return "\n\n".join(f"Paragraph {i}. " + "word " * 150 for i in range(paragraphs))


class BoardTests(BoardToolCase):
    def test_first_run_posts_the_file_and_pins_it(self):
        code, out, _ = self.board("# Open\n\n- call the bank\n")
        self.assertEqual(code, 0, out)
        ids = self.board_ids()
        self.assertEqual(len(ids), 1)
        self.assertEqual(self.fake.messages[ids[0]]["content"], f"# Open\n\n- call the bank\n\n{STAMP}")
        self.assertEqual(self.fake.pins, {ids[0]})
        self.assertEqual(self.state()["boards"]["open"]["pinned"], ids[0])
        headers = self.fake.headers_seen[0]
        self.assertEqual(headers.get("User-agent"), "AgentFrame-board (local, 1.0)")
        self.assertEqual(headers.get("Authorization"), f"Bot {TOKEN}")

    def test_a_second_run_edits_the_same_message_and_does_not_repin(self):
        self.board("- one\n")
        first = self.board_ids()
        self.fake.calls.clear()
        code, _, _ = self.board("- one\n- two\n")
        self.assertEqual(code, 0)
        self.assertEqual(self.board_ids(), first)
        self.assertEqual(self.fake.count("POST"), 0)
        self.assertEqual(self.fake.count("PUT"), 0)
        self.assertEqual(self.fake.count("PATCH"), 1)
        self.assertEqual(self.fake.messages[first[0]]["content"], f"- one\n- two\n\n{STAMP}")

    def test_the_board_grows_and_shrinks_across_messages(self):
        self.board("short\n")
        first = self.board_ids()[0]
        self.board(self.long_text(6))
        ids = self.board_ids()
        self.assertGreaterEqual(len(ids), 3)
        self.assertEqual(ids[0], first)
        for mid in ids:
            self.assertLessEqual(len(self.fake.messages[mid]["content"]), discord_board.LIMIT)
        self.assertTrue(self.fake.messages[ids[-1]]["content"].endswith(STAMP))
        self.assertEqual(self.fake.count("PUT"), 1)

        self.fake.calls.clear()
        self.board("short again\n")
        self.assertEqual(self.board_ids(), [first])
        self.assertEqual(self.fake.count("DELETE"), len(ids) - 1)
        self.assertEqual(self.fake.channel_contents(OPEN_ID), [f"short again\n\n{STAMP}"])

    def test_a_board_deleted_by_hand_is_posted_fresh_and_repinned(self):
        self.board(self.long_text(4))
        old = self.board_ids()
        self.assertGreaterEqual(len(old), 2)
        del self.fake.messages[old[0]]
        self.fake.pins.discard(old[0])
        code, _, _ = self.board(self.long_text(4))
        self.assertEqual(code, 0)
        new = self.board_ids()
        self.assertEqual(len(new), len(old))
        self.assertNotIn(old[0], new)
        self.assertNotIn(old[1], self.fake.messages, "later messages are replaced to keep the order")
        self.assertEqual(self.fake.pins, {new[0]})
        self.assertEqual(self.state()["boards"]["open"]["pinned"], new[0])

    def test_board_refuses_a_forum_channel(self):
        code, _, err = self.run_tool("board", "food", str(self.note), text="x\n")
        self.assertEqual(code, 2)
        self.assertIn("forum channel", err)
        self.assertEqual(self.fake.calls, [])

    def test_a_missing_token_sends_nothing_and_prints_no_secret(self):
        self.env.write_text("", encoding="utf-8")
        with mock.patch.dict(os.environ, {"DISCORD_BOT_TOKEN": ""}):
            code, _, err = self.board("x\n")
        self.assertEqual(code, 2)
        self.assertIn("DISCORD_BOT_TOKEN", err)
        self.assertEqual(self.fake.calls, [])

    def test_errors_hide_ids(self):
        self.assertEqual(discord_board.redact("PATCH /channels/123456789012345678/messages/9"), "PATCH /channels/<id>/messages/9")


class ReviewFixTests(BoardToolCase):
    """One test per review item, numbered as the review numbered them."""

    def test_1_unknown_channel_stops_the_run_and_keeps_every_stored_id(self):
        self.board(self.long_text(4))
        stored = self.state()["boards"]["open"]
        self.fake.channels.discard(OPEN_ID)  # every call now answers 10003
        self.fake.calls.clear()
        code, _, err = self.board("new text\n")
        self.assertEqual(code, 2)
        self.assertIn("10003", err)
        self.assertEqual(self.state()["boards"]["open"], stored)
        self.assertEqual(self.fake.count("POST"), 0)

    def test_1_a_failure_midway_keeps_the_ids_not_proven_gone(self):
        self.board(self.long_text(4))
        stored = self.board_ids()
        self.fake.fail.append(("PATCH", f"/messages/{stored[1]}$", 404, {"message": "Unknown Channel", "code": 10003}))
        code, _, _ = self.board(self.long_text(4))
        self.assertEqual(code, 2)
        self.assertEqual(self.board_ids(), stored)

    def test_1_unknown_channel_on_a_thread_stops_and_keeps_the_post(self):
        self.post("a\n", "--tag", "Plan")
        item = self.state()["posts"]["food"]["ramen"]
        self.fake.delete_thread(item["thread"])
        self.fake.calls.clear()
        code, _, err = self.post("b\n")
        self.assertEqual(code, 2)
        self.assertIn("remove posts.food.ramen", err)
        self.assertEqual(self.state()["posts"]["food"]["ramen"], item)
        self.assertEqual(self.fake.count("POST"), 0)

    def test_2_a_failed_pin_is_retried_on_the_next_run(self):
        self.fake.fail.append(("PUT", "/pins/", 500, {"message": "oops"}))
        code, _, _ = self.board("x\n")
        self.assertEqual(code, 2)
        board = self.state()["boards"]["open"]
        self.assertEqual(len(board["messages"]), 1)
        self.assertIsNone(board["pinned"])
        code, _, _ = self.board("x\n")
        self.assertEqual(code, 0)
        self.assertEqual(self.state()["boards"]["open"]["pinned"], board["messages"][0])
        self.assertEqual(self.fake.pins, {board["messages"][0]})
        self.fake.calls.clear()
        self.board("x\n")
        self.assertEqual(self.fake.count("PUT"), 0)

    def test_3_pins_use_the_current_route_and_fall_back_on_404_or_405(self):
        self.board("x\n")
        self.assertEqual(self.fake.count("PUT", r"/channels/\d+/messages/pins/\d+$"), 1)
        self.assertEqual(self.fake.count("PUT", r"/channels/\d+/pins/\d+$"), 0)
        for status in (404, 405):
            with self.subTest(status=status):
                self.setUp()
                self.fake.new_pin_status = status
                code, _, err = self.board("x\n")
                self.assertEqual(code, 0, err)
                self.assertEqual(self.fake.count("PUT", r"/channels/\d+/pins/\d+$"), 1)
                self.assertEqual(self.fake.pins, set(self.board_ids()))

    def test_3_other_pin_errors_do_not_fall_back(self):
        self.fake.fail.append(("PUT", "/messages/pins/", 403, {"message": "Missing Permissions", "code": 50013}))
        code, _, err = self.board("x\n")
        self.assertEqual(code, 2)
        self.assertIn("403", err)
        self.assertEqual(self.fake.count("PUT", r"/channels/\d+/pins/\d+$"), 0)

    def test_4_a_fresh_lock_refuses_and_a_stale_one_is_taken_over(self):
        self.lock.write_text("pid 1\n", encoding="utf-8")
        self.note.write_text("x\n", encoding="utf-8")
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = discord_board.main(["board", "open", str(self.note)], opener=self.fake, today=TODAY)
        self.assertEqual(code, 2)
        self.assertIn("held by another run", err.getvalue())
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state(), BASE_STATE)
        self.assertTrue(self.lock.exists(), "a refused run leaves the other run's lock alone")

        old = time.time() - 11 * 60
        os.utime(self.lock, (old, old))
        code, _, err = self.board("x\n")  # run_tool also checks the lock is gone after
        self.assertEqual(code, 0, err)
        self.assertEqual(len(self.board_ids()), 1)

    def test_5_every_create_and_edit_sends_no_mentions(self):
        self.board("short\n")
        self.board(self.long_text(4))
        self.post(self.long_text(4), "--tag", "Plan")
        self.post(self.long_text(5))
        sends = [(m, p, b) for m, p, b in self.fake.calls
                 if m in ("POST", "PATCH") and (p.endswith("/threads") or re.search(r"/messages(/\d+)?$", p))]
        self.assertGreater(len(sends), 6)
        for method, path, body in sends:
            payload = body["message"] if path.endswith("/threads") else body
            self.assertEqual(payload.get("allowed_mentions"), NO_MENTIONS, f"{method} {path}")

    def test_6_a_rate_limit_waits_retry_after_and_retries(self):
        self.fake.rate_limit = [0.25]
        code, _, _ = self.board("- x\n")
        self.assertEqual(code, 0)
        self.assertEqual(self.sleeps, [0.35])
        self.assertEqual(len(self.board_ids()), 1)

    def test_6_a_wait_over_sixty_seconds_stops_clearly(self):
        self.fake.rate_limit = [120.0]
        code, _, err = self.board("- x\n")
        self.assertEqual(code, 2)
        self.assertEqual(self.sleeps, [])
        self.assertIn("rate limited for 120s, over the 60s cap", err)

    def test_6_rename_follows_the_stored_title_not_discords_normalised_name(self):
        self.fake.normalise_name = True
        self.post("a\n", title="Ramen Spot")
        tid = self.state()["posts"]["food"]["ramen"]["thread"]
        self.assertEqual(self.fake.threads[tid]["name"], "ramen spot")
        self.fake.calls.clear()
        self.post("b\n", title="Ramen Spot")
        self.assertEqual(self.fake.count("PATCH", rf"/channels/{tid}$"), 0)
        self.post("c\n", title="Ramen Spot, tried")
        self.assertIn(("PATCH", f"/channels/{tid}", {"name": "Ramen Spot, tried"}), self.fake.calls)
        self.assertEqual(self.state()["posts"]["food"]["ramen"]["title"], "Ramen Spot, tried")

    def test_7_a_starter_deleted_by_hand_creates_the_post_afresh(self):
        self.post(self.long_text(4), "--tag", "Plan")
        old = self.state()["posts"]["food"]["ramen"]
        del self.fake.messages[old["thread"]]
        code, _, err = self.post("fresh\n")
        self.assertEqual(code, 0, err)
        new = self.state()["posts"]["food"]["ramen"]
        self.assertNotEqual(new["thread"], old["thread"])
        self.assertEqual(new["messages"], [])
        self.assertEqual(self.fake.channel_contents(new["thread"]), [f"fresh\n\n{STAMP}"])
        self.assertEqual(self.fake.threads[new["thread"]]["applied_tags"], [PLAN_ID])

    def test_8_a_corrupt_state_file_fails_in_one_line(self):
        self.state_path.write_text("{not json", encoding="utf-8")
        code, out, err = self.board("x\n")
        self.assertEqual(code, 2)
        self.assertEqual(len(err.strip().splitlines()), 1)
        self.assertIn("is not valid JSON", err)
        self.assertNotIn("Traceback", err)
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state_path.read_text(encoding="utf-8"), "{not json")

    def test_8_a_non_json_success_body_fails_in_one_line(self):
        self.fake.raw.append(("POST", "/messages$", b"<html>gateway</html>"))
        code, _, err = self.board("x\n")
        self.assertEqual(code, 2)
        self.assertEqual(len(err.strip().splitlines()), 1)
        self.assertIn("not JSON", err)
        self.assertNotIn("Traceback", err)


class PostTests(BoardToolCase):
    def test_forum_create_then_edit_with_a_tag_and_title_change(self):
        code, _, err = self.post("Try the miso.\n", "--tag", "Plan")
        self.assertEqual(code, 0, err)
        item = self.state()["posts"]["food"]["ramen"]
        tid = item["thread"]
        self.assertEqual(item["messages"], [])
        self.assertEqual(self.fake.threads[tid]["applied_tags"], [PLAN_ID])
        self.assertEqual(self.fake.threads[tid]["parent"], FOOD_ID)
        self.assertEqual(self.fake.messages[tid]["content"], f"Try the miso.\n\n{STAMP}")

        self.fake.calls.clear()
        code, _, err = self.post("Miso was great.\n", "--tag", "done", title="Ramen, tried")
        self.assertEqual(code, 0, err)
        self.assertIn(("PATCH", f"/channels/{tid}/messages/{tid}", {"content": f"Miso was great.\n\n{STAMP}", "allowed_mentions": NO_MENTIONS}), self.fake.calls)
        self.assertEqual(self.fake.threads[tid]["name"], "Ramen, tried")
        self.assertEqual(self.fake.threads[tid]["applied_tags"], [DONE_ID])
        self.assertEqual(self.fake.count("POST"), 0)
        item = self.state()["posts"]["food"]["ramen"]
        self.assertEqual((item["thread"], item["title"], item["tags"]), (tid, "Ramen, tried", ["Done"]))

    def test_an_unchanged_title_and_no_tag_leave_the_thread_alone(self):
        self.post("a\n", "--tag", "Plan")
        tid = self.state()["posts"]["food"]["ramen"]["thread"]
        self.fake.calls.clear()
        self.post("b\n")
        self.assertEqual(self.fake.count("PATCH", rf"/channels/{tid}$"), 0)
        self.assertEqual(self.fake.threads[tid]["applied_tags"], [PLAN_ID])

    def test_an_archived_thread_is_unarchived_before_the_edit(self):
        self.post("a\n")
        tid = self.state()["posts"]["food"]["ramen"]["thread"]
        self.fake.threads[tid]["archived"] = True
        code, _, err = self.post("b\n")
        self.assertEqual(code, 0, err)
        self.assertFalse(self.fake.threads[tid]["archived"])
        self.assertEqual(self.fake.messages[tid]["content"], f"b\n\n{STAMP}")

    def test_a_long_post_overflows_into_thread_messages_and_shrinks_back(self):
        self.post(self.long_text(5))
        item = self.state()["posts"]["food"]["ramen"]
        tid = item["thread"]
        self.assertGreaterEqual(len(item["messages"]), 1)
        for mid in item["messages"]:
            self.assertEqual(self.fake.messages[mid]["channel"], tid)
        extra = item["messages"]
        self.post("short\n")
        self.assertEqual(self.state()["posts"]["food"]["ramen"]["messages"], [])
        for mid in extra:
            self.assertNotIn(mid, self.fake.messages)
        self.assertEqual(self.fake.channel_contents(tid), [f"short\n\n{STAMP}"])

    def test_an_unknown_tag_fails_and_lists_the_valid_ones(self):
        before = self.state_path.read_bytes()
        code, _, err = self.post("a\n", "--tag", "Someday")
        self.assertEqual(code, 2)
        self.assertIn("Someday", err)
        self.assertIn("Valid tags: Done, Plan", err)
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state_path.read_bytes(), before)


class DryRunAndStateTests(BoardToolCase):
    def test_dry_run_sends_nothing_and_writes_nothing(self):
        self.board("- one\n")
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
            self.assertIsNone(re.search(r"\d{15,}", out), "a dry run prints no ids")
        self.assertEqual(self.fake.calls, [])
        self.assertEqual(self.state_path.read_bytes(), before)
        self.assertEqual(self.state_path.stat().st_mtime_ns, mtime)

    def test_other_state_keys_are_preserved(self):
        self.board("- one\n")
        self.post("a\n", "--tag", "Plan")
        state = self.state()
        for key in ("guild_id", "channels", "events"):
            self.assertEqual(state[key], BASE_STATE[key])
        self.assertEqual(set(state), set(BASE_STATE))

    def test_the_env_channel_id_wins_over_state(self):
        self.env.write_text(f"DISCORD_BOT_TOKEN={TOKEN}\nOPEN_DISCORD_CHANNEL_ID=200000000000000999\n", encoding="utf-8")
        self.fake.channels.add("200000000000000999")
        self.board("x\n")
        self.assertEqual(self.fake.calls[0][1], "/channels/200000000000000999/messages")


if __name__ == "__main__":
    unittest.main()
