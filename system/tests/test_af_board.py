import contextlib
import datetime as dt
import io
import json
import os
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

from system import af
from system import board


TZ = dt.datetime.now().astimezone().tzinfo


def when(text):
    return dt.datetime.fromisoformat(text).replace(tzinfo=TZ)


def row(name, state="working", session_id="a5b92fc8-136c-4ee3-8736-6d3dc5344056", kind="background"):
    return {"id": session_id[:8], "kind": kind, "state": state, "name": name, "sessionId": session_id,
            "cwd": "C:\\vault"}


class BoardModuleTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.home = self.root / "home"
        (self.home / "projects" / "c--vault").mkdir(parents=True)
        board.init(self.root)
        self.now = when("2026-09-09T12:00:00")
        self.addCleanup(self._tmp.cleanup)

    # helpers -----------------------------------------------------------
    def load(self):
        return board.load(self.root)

    def add(self, project="joyce-hair-pilot", deliverable="booking-page-copy", by="orchestrator", now=None):
        b = self.load()
        card = board.add(self.root, b, project=project, deliverable=deliverable, by=by, now=now or self.now)
        board.save(self.root, b)
        return card.id

    def dispatch(self, card_id, rows=None, session=None, force=False):
        b = self.load()
        board.dispatch(b, card_id, rows=rows, session=session, force=force)
        board.save(self.root, b)

    def sync(self, rows=None, now=None, roster_available=None):
        b = self.load()
        events = board.sync(self.root, b, now=now or self.now, rows=rows, roster_available=roster_available,
                            home=self.home)
        board.save(self.root, b)
        return events, self.load()

    def touch_transcript(self, prefix, age_days):
        path = self.home / "projects" / "c--vault" / f"{prefix}-136c-4ee3-8736-6d3dc5344056.jsonl"
        path.write_text("{}", encoding="utf-8")
        stamp = (self.now - dt.timedelta(days=age_days)).timestamp()
        os.utime(path, (stamp, stamp))

    # grammar -----------------------------------------------------------
    def test_init_writes_a_valid_empty_board_and_folders(self):
        p = board.paths(self.root)
        self.assertTrue(p["board"].is_file())
        for key in ("tasks", "receipts", "applied", "archive"):
            self.assertTrue(p[key].is_dir(), key)
        text = p["board"].read_text(encoding="utf-8")
        self.assertIn("auto_close_after: 7d", text)
        parsed, errors = board.parse(text)
        self.assertEqual(errors, [])
        self.assertEqual(board.render(parsed), text)

    def test_parse_reports_grammar_errors_without_aborting(self):
        text = board.render(board.empty_board()) + "\n".join([
            "## Inbox",
            "- [ ] T-2026-09-09-01 · p · d · @p/d",
        ]) + "\n"
        text = text.replace("## Queued\n", "## Queued\n\n- [x] T-2026-09-09-02 · p · d · @p/d\n"
                            "- [ ] T-2026-09-09-03 · p · d\n"
                            "- [ ] T-2026-09-09-04 · p · d · @p/d · colour: red\n", 1)
        text = text.replace("## Needs you\n", "## Needs you\n\n- [ ] T-2026-09-09-05 · p · d · @p/d\n", 1)
        parsed, errors = board.parse(text)
        joined = "\n".join(errors)
        self.assertIn("unknown lane 'Inbox'", joined)
        self.assertIn("[x] outside Done", joined)
        self.assertIn("needs id · project · deliverable · @owner", joined)
        self.assertIn("unknown token 'colour: red'", joined)
        self.assertIn("without ask", joined)
        with self.assertRaises(board.BoardError):
            board.paths(self.root)["board"].write_text(text, encoding="utf-8")
            board.load(self.root)

    def test_field_values_never_carry_the_separator(self):
        b = self.load()
        card = board.add(self.root, b, project="p", deliverable="d", note="a · b\nc", now=self.now)
        self.assertEqual(card.fields["note"], "a - b c")
        board.save(self.root, b)
        self.assertEqual(board.parse(board.render(self.load()))[1], [])

    # add / dispatch ------------------------------------------------------
    def test_add_sequences_ids_per_day_and_scaffolds_a_brief(self):
        first = self.add()
        second = self.add(deliverable="gbp-listing-fix")
        self.assertEqual((first, second), ("T-2026-09-09-01", "T-2026-09-09-02"))
        b = self.load()
        card = b.find(first)
        self.assertEqual(card.lane, "Queued")
        self.assertEqual(card.owner, "joyce-hair-pilot/booking-page-copy")
        brief = board.paths(self.root)["dir"] / card.fields["brief"]
        self.assertTrue(brief.is_file())
        body = brief.read_text(encoding="utf-8")
        self.assertIn("id: T-2026-09-09-01", body)
        self.assertIn("receipt: workspace/board/receipts/T-2026-09-09-01.result.json", body)

    def test_add_rejects_bad_slugs_and_by_values(self):
        b = self.load()
        with self.assertRaises(board.BoardError):
            board.add(self.root, b, project="Joyce Hair", deliverable="d", now=self.now)
        with self.assertRaises(board.BoardError):
            board.add(self.root, b, project="p", deliverable="d", by="robot", now=self.now)

    def test_dispatch_enforces_one_worker_per_project_and_resolves_session_from_roster(self):
        first = self.add()
        second = self.add(deliverable="gbp-listing-fix")
        self.dispatch(first, rows=[row("joyce-hair-pilot/booking-page-copy")])
        b = self.load()
        self.assertEqual(b.find(first).lane, "In progress")
        self.assertEqual(b.find(first).fields["session"], "a5b92fc8")
        with self.assertRaises(board.BoardError):
            board.dispatch(b, second)
        board.dispatch(b, second, force=True)
        self.assertEqual(b.find(second).lane, "In progress")

    # receipts ------------------------------------------------------------
    def test_done_receipt_moves_card_to_needs_you_review_and_consumes_the_file(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="joyce-hair-pilot/booking-page-copy", status="done",
                            summary="Wrote the copy.", outputs=["workspace/projects/joyce-hair-pilot/x.md"])
        events, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy")])
        card = b.find(cid)
        self.assertEqual(card.lane, "Needs you")
        self.assertEqual(card.fields["ask"], "review")
        self.assertEqual(card.fields["reason"], "Wrote the copy.")
        self.assertTrue(card.fields["waiting_since"].startswith("2026-09-09T12:00:00"))
        self.assertTrue(card.fields["receipt"].startswith("receipts/applied/"))
        self.assertFalse((board.paths(self.root)["receipts"] / f"{cid}.result.json").exists())
        self.assertIn(f"{cid}: done receipt -> Needs you (review)", events)

    def test_blocked_receipt_moves_card_to_needs_you_input_with_the_operator_action(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="blocked", summary="No address.",
                            operator_action="send the new office address")
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        card = b.find(cid)
        self.assertEqual((card.lane, card.fields["ask"]), ("Needs you", "input"))
        self.assertEqual(card.fields["reason"], "blocked: send the new office address")

    # staleness -------------------------------------------------------------
    def test_orchestrator_worker_idle_without_receipt_needs_you_immediately(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        card = b.find(cid)
        self.assertEqual((card.lane, card.fields["ask"]), ("Needs you", "input"))
        self.assertIn("worker idle with no receipt", card.fields["reason"])

    def test_orchestrator_worker_gone_from_roster_needs_you_immediately(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        _, b = self.sync(rows=[])
        self.assertIn("session gone", b.find(cid).fields["reason"])

    def test_without_a_roster_the_time_rule_applies_to_everyone(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        self.touch_transcript("a5b92fc8", age_days=1)
        _, b = self.sync(rows=None)
        self.assertEqual(b.find(cid).lane, "In progress")
        self.touch_transcript("a5b92fc8", age_days=4)
        _, b = self.sync(rows=None)
        self.assertEqual(b.find(cid).lane, "Needs you")
        self.assertIn("idle 4d", b.find(cid).fields["reason"])

    def test_human_chat_gets_the_grace_period_then_needs_you(self):
        cid = self.add(by="human")
        self.dispatch(cid, session="a5b92fc8")
        self.touch_transcript("a5b92fc8", age_days=2)
        events, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        self.assertEqual(b.find(cid).lane, "In progress")
        self.assertEqual(events, [])
        self.touch_transcript("a5b92fc8", age_days=5)
        events, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        card = b.find(cid)
        self.assertEqual(card.lane, "Needs you")
        self.assertEqual(card.fields["reason"], "idle 5d with no receipt. resume, close, or drop?")

    def test_human_chat_working_in_roster_is_never_stale(self):
        cid = self.add(by="human")
        self.dispatch(cid, session="a5b92fc8")
        self.touch_transcript("a5b92fc8", age_days=30)
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="working")])
        self.assertEqual(b.find(cid).lane, "In progress")

    def test_needs_input_card_returns_to_in_progress_when_the_worker_works_again(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        events, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="working")])
        card = b.find(cid)
        self.assertEqual(card.lane, "In progress")
        self.assertNotIn("ask", card.fields)
        self.assertNotIn("waiting_since", card.fields)
        self.assertIn(f"{cid}: worker resumed -> In progress", events)

    def test_review_card_does_not_auto_return(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="done", summary="ok")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy")])
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="working")])
        self.assertEqual(b.find(cid).lane, "Needs you")

    # auto-drop / archive / reopen -------------------------------------------
    def test_seven_days_in_needs_you_drops_the_card_to_the_archive_and_reopen_restores_it(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="done", summary="ok")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        later = self.now + dt.timedelta(days=6, hours=23)
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")], now=later)
        self.assertEqual(b.find(cid).lane, "Needs you")
        later = self.now + dt.timedelta(days=7, hours=1)
        events, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")], now=later)
        self.assertIsNone(b.find(cid))
        self.assertIn(f"{cid}: 7d in Needs you -> dropped (archive)", events)
        p = board.paths(self.root)
        archive = (p["archive"] / "2026-09.md").read_text(encoding="utf-8")
        self.assertIn(f"- [x] {cid} · joyce-hair-pilot · booking-page-copy", archive)
        self.assertIn("outcome: dropped", archive)
        self.assertIn("note: dropped after 7d in Needs you", archive)
        self.assertTrue((p["archive"] / "2026-09" / f"{cid}.task.md").is_file())
        self.assertFalse((p["tasks"] / f"{cid}.task.md").exists())
        self.assertEqual(list(p["applied"].glob(f"{cid}.*")), [])
        # reopen brings it back to Queued with its brief
        b = self.load()
        card = board.reopen(self.root, b, cid, now=later)
        board.save(self.root, b)
        b = self.load()
        self.assertEqual(b.find(cid).lane, "Queued")
        self.assertTrue((p["tasks"] / f"{cid}.task.md").is_file())
        self.assertNotIn(cid, (p["archive"] / "2026-09.md").read_text(encoding="utf-8"))
        self.assertEqual(card.fields["note"], "reopened 2026-09-16")

    def test_auto_close_off_never_drops(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="done", summary="ok")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        b = self.load()
        b.meta["auto_close_after"] = "off"
        board.save(self.root, b)
        _, b = self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")],
                         now=self.now + dt.timedelta(days=60))
        self.assertEqual(b.find(cid).lane, "Needs you")

    def test_approve_close_and_sweep(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="done", summary="ok")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        b = self.load()
        with self.assertRaises(board.BoardError):
            board.close  # closing is allowed; approving an input card is not
            board.approve(b, "T-2026-09-09-99")
        card = board.approve(b, cid)
        self.assertEqual((card.lane, card.done, card.state_label()), ("Done", False, "closing"))
        board.save(self.root, b)
        b = self.load()
        board.close(b, cid)
        board.save(self.root, b)
        self.assertEqual(self.load().find(cid).state_label(), "closed")
        events, b = self.sync(rows=[])
        self.assertIsNone(b.find(cid))
        self.assertIn(f"{cid}: closed -> archive", events)
        archive = (board.paths(self.root)["archive"] / "2026-09.md").read_text(encoding="utf-8")
        self.assertIn("outcome: closed", archive)

    def test_approve_refuses_an_input_card(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        with self.assertRaises(board.BoardError):
            board.approve(self.load(), cid)

    def test_return_moves_back_to_in_progress_and_appends_followup_to_the_brief(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        board.write_receipt(self.root, cid, session="s", status="done", summary="ok")
        self.sync(rows=[row("joyce-hair-pilot/booking-page-copy", state="idle")])
        b = self.load()
        card = board.return_(b, cid, "Shorter headline, keep the CTA.", self.root)
        board.save(self.root, b)
        self.assertEqual(card.lane, "In progress")
        self.assertNotIn("ask", card.fields)
        brief = board.paths(self.root)["dir"] / card.fields["brief"]
        self.assertIn("## Follow-up", brief.read_text(encoding="utf-8"))
        self.assertIn("Shorter headline", brief.read_text(encoding="utf-8"))

    def test_drop_archives_immediately(self):
        cid = self.add()
        b = self.load()
        board.drop(self.root, b, cid, now=self.now, note="not doing this")
        board.save(self.root, b)
        self.assertIsNone(self.load().find(cid))
        archive = (board.paths(self.root)["archive"] / "2026-09.md").read_text(encoding="utf-8")
        self.assertIn("outcome: dropped", archive)
        self.assertIn("note: not doing this", archive)

    def test_ids_never_collide_with_archived_cards(self):
        cid = self.add()
        b = self.load()
        board.drop(self.root, b, cid, now=self.now)
        board.save(self.root, b)
        self.assertEqual(self.add(), "T-2026-09-09-02")

    # doctor / snapshot ------------------------------------------------------
    def test_issues_flag_held_queued_and_long_waits(self):
        cid = self.add(now=self.now - dt.timedelta(days=5))
        b = self.load()
        notes = board.issues(self.root, b, now=self.now)
        self.assertTrue(any(f"{cid}: queued 5d" in n for n in notes))

    def test_snapshot_joins_roster_state(self):
        cid = self.add()
        self.dispatch(cid, session="a5b92fc8")
        snap = board.snapshot(self.root, self.load(), rows=[row("joyce-hair-pilot/booking-page-copy")], now=self.now)
        lane = next(l for l in snap["lanes"] if l["name"] == "In progress")
        self.assertEqual(lane["cards"][0]["state"], "working")
        self.assertEqual(lane["cards"][0]["session_id"], "a5b92fc8-136c-4ee3-8736-6d3dc5344056")
        self.assertEqual(snap["waiting_on_you"], 0)
        self.assertEqual(snap["open"], 1)

    def test_launch_background_parses_the_session_id(self):
        cid = self.add()
        b = self.load()
        card = b.find(cid)
        seen = {}

        def runner(argv):
            seen["argv"] = argv
            # Windows consoles can mangle the separators; the id must still parse.
            return "Starting background service\xe2\x80\xa6\nbackgrounded \xc2\xb7 a84640d9 \xc2\xb7 joyce-hair-pilot/booking-page-copy\n  claude attach a84640d9\n"

        with patch.object(board, "claude_bin", return_value="claude"):
            prefix = board.launch_background(self.root, card, model="sonnet", runner=runner)
        self.assertEqual(prefix, "a84640d9")
        self.assertIn("--bg", seen["argv"])
        self.assertIn("joyce-hair-pilot/booking-page-copy", seen["argv"])
        self.assertIn("sonnet", seen["argv"])
        self.assertIn(cid, seen["argv"][-1])


class BoardCliTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self._root_patch = patch.object(af, "ROOT", self.root)
        self._root_patch.start()
        self.addCleanup(self._root_patch.stop)
        self.addCleanup(self._tmp.cleanup)

    def run_cmd(self, fn, **kwargs):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            fn(types.SimpleNamespace(**kwargs))
        return out.getvalue()

    def test_init_add_dispatch_list_json(self):
        self.assertIn("ready", self.run_cmd(af.cmd_board_init))
        out = self.run_cmd(af.cmd_board_add, project="meetcap", deliverable="live-call-trial", by="human",
                           owner=None, goal="", done_when="", note="")
        self.assertIn("T-", out)
        cid = out.split("af board add: ")[1].split(" ")[0]
        self.run_cmd(af.cmd_board_dispatch, card_id=cid, session="abcdef12", launch=False, model=None,
                     force=False, no_roster=True)
        listing = json.loads(self.run_cmd(af.cmd_board_list, json=True, no_roster=True))
        lane = next(l for l in listing["lanes"] if l["name"] == "In progress")
        self.assertEqual(lane["cards"][0]["id"], cid)
        self.assertEqual(lane["cards"][0]["session_prefix"], "abcdef12")
        self.assertFalse(listing["roster_available"])

    def test_sync_quiet_without_a_board_is_silent(self):
        self.assertEqual(self.run_cmd(af.cmd_board_sync, quiet=True, no_roster=True), "")

    def test_managed_run_gate_blocks_transitions_but_not_sync_or_list(self):
        with patch.dict(os.environ, {"AGENTFRAME_MANAGED_RUN": "1"}):
            with self.assertRaises(SystemExit):
                with contextlib.redirect_stderr(io.StringIO()):
                    af.check_mode_gate("board", types.SimpleNamespace(board_cmd="approve"))
            af.check_mode_gate("board", types.SimpleNamespace(board_cmd="sync"))
            af.check_mode_gate("board", types.SimpleNamespace(board_cmd="list"))


class BoardHookTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        board.init(self.root)
        self.addCleanup(self._tmp.cleanup)

    def test_stop_backstop_writes_a_blocked_receipt_only_for_orchestrator_workers(self):
        from system.hooks import board_guard
        b = board.load(self.root)
        card = board.add(self.root, b, project="p", deliverable="d", now=when("2026-09-09T12:00:00"))
        human = board.add(self.root, b, project="q", deliverable="e", by="human", now=when("2026-09-09T12:00:00"))
        board.dispatch(b, card.id, session="a5b92fc8")
        board.dispatch(b, human.id, session="a84640d9")
        board.save(self.root, b)
        written = board_guard.backstop_receipt(self.root, "a5b92fc8-136c-4ee3-8736-6d3dc5344056",
                                               {"last_assistant_message": "I could not find the address."})
        self.assertEqual(written, card.id)
        receipt = board.read_receipt(board.paths(self.root)["receipts"] / f"{card.id}.result.json")
        self.assertEqual(receipt["status"], "blocked")
        self.assertEqual(receipt["auto"], "stop-hook")
        self.assertIsNone(board_guard.backstop_receipt(self.root, "a84640d9-8382-4d14-a095-a14248326708", {}))
        # a second stop does not overwrite an existing receipt
        self.assertIsNone(board_guard.backstop_receipt(self.root, "a5b92fc8-136c-4ee3-8736-6d3dc5344056", {}))


if __name__ == "__main__":
    unittest.main()
