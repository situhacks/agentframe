"""Contract tests for `system/tools/push_due_loops.py`, the daily reminder push.

The push renders `af open due --json` as headings and bullets (never a table),
stays silent with nothing due, and logs each real run; no test touches the network.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import off_discord  # noqa: E402
import push_due_loops  # noqa: E402

_HANDOFF_WAIT = push_due_loops.HANDOFF_WAIT_S


def setUpModule():
    push_due_loops.HANDOFF_WAIT_S = 0  # real runs here have no sweep; WaitTests use their own clock


def tearDownModule():
    push_due_loops.HANDOFF_WAIT_S = _HANDOFF_WAIT


LOOPS = [
    {"n": 3, "section": "Waiting on others", "opened": "2026-10-15", "text": "refund from the shop",
     "nudge": "2026-10-25", "every": None, "from": "main", "age_days": 13},
    {"n": 1, "section": "Waiting on the operator", "opened": "2026-10-24", "text": "pay the card",
     "nudge": "2026-10-28", "every": "monthly", "from": "main", "age_days": 4},
]


class RenderTests(unittest.TestCase):
    def test_sections_are_headings_and_loops_are_bullets_with_their_period(self):
        text = push_due_loops.render(LOOPS, dt.date(2026, 10, 28), terms=[])
        self.assertEqual(
            text,
            "# 2 loops due · 2026-10-28\n\n## Waiting on others\n\n"
            "- **refund from the shop** · nudged 2026-10-25 · open 13d\n\n"
            "## Waiting on the operator\n\n"
            "- **pay the card** · nudged 2026-10-28 · every monthly · open 4d\n",
        )
        self.assertNotIn("|", text)


TODAY = dt.date(2026, 10, 28)
OVERDUE = [{"text": "pay the card", "show": "Pay the card", "due": "2026-10-25", "days": 3, "why": ["money"]}]


class SweepHandoffTests(unittest.TestCase):
    def test_the_sweeps_overdue_loops_lead_once_in_its_words(self):
        text = push_due_loops.render(LOOPS, TODAY, OVERDUE, [])
        self.assertTrue(text.startswith("# 2 loops due · 2026-10-28\n\n## Overdue, not confirmed\n\n"
                                        "- **Pay the card** · due 2026-10-25 · 3d over · money\n"))
        self.assertEqual(text.count("ay the card"), 1, "an overdue loop goes out once a day")
        self.assertIn("- **refund from the shop**", text)

    def test_a_handed_loop_no_longer_due_is_dropped(self):
        gone = [{"text": "closed since the sweep", "show": "Closed", "due": "2026-10-25", "days": 3, "why": []}]
        self.assertNotIn("Overdue", push_due_loops.render(LOOPS, TODAY, gone, []))

    def test_a_loop_the_sweep_kept_off_counts_without_its_words(self):
        hidden = [{"text": "pay the card", "show": None, "due": "2026-10-25", "days": 3, "why": ["money"]}]
        text = push_due_loops.render(LOOPS, TODAY, hidden, [])
        self.assertNotIn("pay the card", text.casefold())
        self.assertIn("- 1 more kept off Discord; see Do in AgentFrame", text)

    def test_only_todays_handoff_counts(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / push_due_loops.HANDOFF
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({"date": "2026-10-27", "items": OVERDUE}), encoding="utf-8")
            self.assertIsNone(push_due_loops.load_handoff(Path(d), TODAY))
            path.write_text(json.dumps({"date": "2026-10-28", "items": OVERDUE}), encoding="utf-8")
            self.assertEqual(push_due_loops.load_handoff(Path(d), TODAY), OVERDUE)
            path.write_text("not json", encoding="utf-8")
            self.assertIsNone(push_due_loops.load_handoff(Path(d), TODAY))
        self.assertIsNone(push_due_loops.load_handoff(Path("no-such-vault"), TODAY))


class OffDiscordTests(unittest.TestCase):
    SENSITIVE = [
        {"n": 4, "section": "Waiting on the operator", "opened": "2026-10-20", "text": "Book the GP appointment",
         "nudge": "2026-10-27", "every": None, "from": "main", "age_days": 8},
        {"n": 5, "section": "Waiting on the operator", "opened": "2026-10-20", "text": "Pick a therapist",
         "nudge": "2026-10-27", "every": None, "from": "main", "age_days": 8},
        {"n": 6, "section": "Waiting on the operator", "opened": "2026-10-20", "text": "Ask Robin about the cabin",
         "nudge": "2026-10-27", "every": None, "from": "main", "age_days": 8},
    ]

    def test_the_sensitive_categories_always_stay_off(self):
        text = push_due_loops.render(LOOPS + self.SENSITIVE, TODAY, None, [])
        self.assertTrue(text.startswith("# 5 loops due"))
        for gone in ("GP", "therapist"):
            self.assertNotIn(gone, text)
        self.assertIn("Ask Robin about the cabin", text)
        self.assertIn("- 2 more kept off Discord; see Do in AgentFrame", text)

    def test_with_no_terms_file_it_sends_only_counts(self):
        """Fails closed exactly as the board does: no loop text at all, only a count."""
        overdue = [{"text": "pay the card", "show": "Pay the card", "due": "2026-10-25", "days": 3, "why": ["money"]}]
        text = push_due_loops.render(LOOPS + self.SENSITIVE, TODAY, overdue, None)
        self.assertTrue(text.startswith("# 5 loops due · 2026-10-28\n"))
        self.assertIn("5 due, 1 overdue and not confirmed; open AgentFrame", text)
        for gone in ("Robin", "card", "refund", "therapist"):
            self.assertNotIn(gone, text)
        self.assertEqual(push_due_loops.overdue_section(overdue, None), ("", 1))

    def test_amounts_and_long_numbers_are_masked_in_the_push(self):
        loops = [dict(LOOPS[1], text="Send $1,250.00 rent to the landlord, ref 48213377"),
                 dict(LOOPS[0], text="Claim the 40 dollars back, card 4242 4242 by 2026-10-30")]
        text = push_due_loops.render(loops, TODAY, [{"text": loops[0]["text"], "show": "Rent $1,250 to the landlord",
                                                     "due": "2026-10-25", "days": 3, "why": ["money"]}], [])
        for gone in ("1,250", "48213377", "4242", "40 dollars"):
            self.assertNotIn(gone, text)
        self.assertIn("**Rent $… to the landlord**", text)
        self.assertIn("**Claim the $… back, card #### #### by 2026-10-30**", text)

    def test_his_private_terms_keep_more_off(self):
        text = push_due_loops.render(LOOPS + self.SENSITIVE, TODAY, None, ["robin"])
        self.assertNotIn("Robin", text)
        self.assertIn("- 3 more kept off Discord", text)

    def test_the_categories_are_whole_words(self):
        self.assertTrue(off_discord.is_private(("Refill the prescription",), None))
        self.assertTrue(off_discord.is_private(("change the account password",), None))
        self.assertTrue(off_discord.is_private(("Reset the card PIN",), None))
        for fine in ("Pin the board message", "Move the savings to Fernhollow Bank", "Spinning class", "Book the GPU order"):
            self.assertFalse(off_discord.is_private((fine,), None), fine)
        self.assertFalse(off_discord.is_private(("", None), []))

    def test_meds_and_health_appointments_stay_off(self):
        for text in ("Pick up the meds", "Ask about the medication", "Book the dentist", "Dental cleaning on 03-08",
                     "Call the doctor", "Clinic hours changed", "Eye appointment Thursday", "Skin exam next week",
                     "Flu shot at the pharmacy", "Annual check-up"):
            self.assertTrue(off_discord.is_private((text,), []), text)
        for fine in ("Book the hair appointment", "Eye up the new desk", "Exam the lab bench"):
            self.assertFalse(off_discord.is_private((fine,), []), fine)

    def test_masking_keeps_dates_and_masks_amounts_only_when_asked(self):
        line = "Pay $100.00 by 2026-03-20, account 12345678, 3/8, 2026-03"
        self.assertEqual(off_discord.mask(line, amounts=False), "Pay $100.00 by 2026-03-20, account ####, 3/8, 2026-03")
        self.assertEqual(off_discord.mask(line, amounts=True), "Pay $… by 2026-03-20, account ####, 3/8, 2026-03")
        self.assertEqual(off_discord.mask("Card 1234-5678 and 25 dollars", amounts=True), "Card #### and $…")
        self.assertEqual(off_discord.mask("Call at 9:30 on 03-08", amounts=True), "Call at 9:30 on 03-08")

    def test_the_terms_file_is_read_from_the_vault(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertIsNone(off_discord.private_terms(Path(d)))
            path = Path(d) / off_discord.TERMS_FILE
            path.parent.mkdir(parents=True)
            path.write_text("# a comment\nRobin\n\n  Cabin  \n", encoding="utf-8")
            self.assertEqual(off_discord.private_terms(Path(d)), ["robin", "cabin"])


class MainTests(unittest.TestCase):
    def run_main(self, loops, args):
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            log = Path(d) / "reminders-push.log"
            terms = Path(d) / off_discord.TERMS_FILE
            terms.parent.mkdir(parents=True)
            terms.write_text("# none yet\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"AF_ROOT": d}), \
                    mock.patch.object(push_due_loops, "due_loops", return_value=loops), \
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
            with mock.patch.dict(os.environ, {"AF_ROOT": d}), \
                    mock.patch.object(push_due_loops, "due_loops", return_value=LOOPS), \
                    mock.patch.object(push_due_loops, "LOG", log), \
                    mock.patch.object(push_due_loops.notify, "post", side_effect=TimeoutError("timed out")), \
                    mock.patch.object(push_due_loops.notify, "load_env",
                                      return_value={"REMINDERS_DISCORD_WEBHOOK": "https://example.invalid/hook"}), \
                    contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
                code = push_due_loops.main([])
            logged = log.read_text(encoding="utf-8")
        self.assertEqual(code, 2)
        self.assertIn(" · 2 due · notify exit 2: notify failed: message 1 of 1: timed out", logged)

    def test_a_run_reads_the_handoff_and_terms_from_the_vault(self):
        out = io.StringIO()
        today = dt.date.today()
        items = [{"text": "pay the card", "show": "Pay the card now", "due": "2026-10-25", "days": 3, "why": ["money"]}]
        with tempfile.TemporaryDirectory() as d:
            handoff = Path(d) / push_due_loops.HANDOFF
            handoff.parent.mkdir(parents=True)
            handoff.write_text(json.dumps({"date": today.isoformat(), "items": items}), encoding="utf-8")
            terms = Path(d) / off_discord.TERMS_FILE
            terms.parent.mkdir(parents=True)
            terms.write_text("shop\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"AF_ROOT": d}), \
                    mock.patch.object(push_due_loops, "due_loops", return_value=LOOPS), \
                    contextlib.redirect_stdout(out):
                code = push_due_loops.main(["--dry-run"])
        self.assertEqual(code, 0)
        self.assertIn("**Pay the card now** · due 2026-10-25", out.getvalue())
        self.assertNotIn("refund from the shop", out.getvalue())
        self.assertIn("1 more kept off Discord", out.getvalue())

    def test_a_real_run_posts_once_and_logs_the_count(self):
        code, _, posted, logged = self.run_main(LOOPS, [])
        self.assertEqual((code, posted), (0, 1))
        self.assertTrue(logged.endswith(" · 2 due · sent\n"), logged)

    def test_a_run_with_no_terms_file_sends_only_the_count(self):
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            with mock.patch.dict(os.environ, {"AF_ROOT": d}), \
                    mock.patch.object(push_due_loops, "due_loops", return_value=LOOPS), \
                    contextlib.redirect_stdout(out):
                self.assertEqual(push_due_loops.main(["--dry-run"]), 0)
        self.assertIn("2 due; open AgentFrame to see them", out.getvalue())
        self.assertNotIn("card", out.getvalue())


class WaitForSweepTests(unittest.TestCase):
    """At logon the sweep and the push can start together; the push waits for the sweep's handoff."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.marker = self.root / push_due_loops.SWEEP_RUNNING
        self.marker.parent.mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def clock(self, start, on_sleep=None):
        t = [start]
        naps = []

        def sleep(s):
            naps.append(s)
            t[0] += s
            if on_sleep:
                on_sleep(len(naps))
        return (lambda: t[0]), sleep, naps

    def test_no_marker_goes_at_once(self):
        now, sleep, naps = self.clock(1_000_000.0)
        self.assertTrue(push_due_loops.wait_for_sweep(self.root, now=now, sleep=sleep))
        self.assertEqual(naps, [])

    def test_it_waits_until_the_sweep_finishes(self):
        self.marker.write_text("x", encoding="utf-8")
        start = self.marker.stat().st_mtime
        now, sleep, naps = self.clock(start + 5, on_sleep=lambda n: n == 3 and self.marker.unlink())
        self.assertTrue(push_due_loops.wait_for_sweep(self.root, now=now, sleep=sleep))
        self.assertEqual(len(naps), 3)

    def test_it_gives_up_after_the_wait_and_ignores_a_stale_marker(self):
        self.marker.write_text("x", encoding="utf-8")
        start = self.marker.stat().st_mtime
        now, sleep, naps = self.clock(start + 5)
        self.assertFalse(push_due_loops.wait_for_sweep(self.root, now=now, sleep=sleep))
        self.assertLessEqual(sum(naps), push_due_loops.WAIT_S + push_due_loops.POLL_S)
        self.assertLess(push_due_loops.WAIT_S, 10 * 60, "inside the push task's 10-minute limit")
        now, sleep, naps = self.clock(start + push_due_loops.STALE_S + 1)
        self.assertFalse(push_due_loops.wait_for_sweep(self.root, now=now, sleep=sleep))
        self.assertEqual(naps, [])

    def test_a_dry_run_never_waits(self):
        with mock.patch.object(push_due_loops, "wait_for_sweep") as wait, \
                mock.patch.object(push_due_loops, "due_loops", return_value=[]):
            push_due_loops.main(["--dry-run"])
        wait.assert_not_called()

    def test_a_missing_handoff_gets_about_45_seconds(self):
        today = dt.date(2026, 3, 10)
        now, sleep, naps = self.clock(1_000_000.0)
        with mock.patch.object(push_due_loops, "HANDOFF_WAIT_S", _HANDOFF_WAIT):
            self.assertIsNone(push_due_loops.wait_for_handoff(self.root, today, now=now, sleep=sleep))
        self.assertEqual(sum(naps), _HANDOFF_WAIT)
        self.assertEqual(_HANDOFF_WAIT, 45)
        handoff = self.root / push_due_loops.HANDOFF
        items = [{"text": "x", "show": "x", "due": "2026-03-09", "days": 1, "why": ["money"]}]

        def arrives(n):
            if n == 2:
                handoff.write_text(json.dumps({"date": "2026-03-10", "items": items}), encoding="utf-8")
        now, sleep, naps = self.clock(1_000_000.0, on_sleep=arrives)
        with mock.patch.object(push_due_loops, "HANDOFF_WAIT_S", _HANDOFF_WAIT):
            self.assertEqual(push_due_loops.wait_for_handoff(self.root, today, now=now, sleep=sleep), items)
        self.assertEqual(len(naps), 2)


class SecondReviewOffDiscordTests(unittest.TestCase):
    """Each off-Discord case the second review's probe found leaking."""

    def test_health_words_survive_disguise(self):
        for text in ("Dr. Patel appointment Tue", "Dr appt Thursday", "Doc appt", "mеds (cyrillic e)",
                     "ＭＥＤＳ", "den​tist", "psych appt", "MRI follow-up", "x-ray results",
                     "pick up Rx", "urgent care visit", "colonoscopy prep", "OB/GYN visit", "allergy shots",
                     "Renew the prescription at the pharmacy"):
            self.assertTrue(off_discord.is_private((text,), []), text)

    def test_identifying_numbers_are_masked_everywhere(self):
        for text, want in (("Card 4520 1234 5678", "Card ####"), ("Ref AB12345678", "Ref ####"),
                           ("Conf #A1234567", "Conf ####"), ("Phone 604-555-1234", "Phone ####"),
                           ("Phone (604) 555-1234", "Phone ####"), ("Acct 123 456 789", "Acct ####"),
                           ("acct 12345-67", "acct ####"), ("sin 123-456-789", "sin ####"),
                           ("due 2026-11-09, at 10:30, on 11-23", "due 2026-11-09, at 10:30, on 11-23")):
            self.assertEqual(off_discord.mask(text, amounts=False), want, text)
            self.assertEqual(off_discord.mask(text, amounts=True), want, text)

    def test_money_in_any_notation_is_masked_in_the_push_and_kept_on_the_board(self):
        for text in ("Pay $1,200.00", "Pay 1,200 CAD", "Pay CAD 1200", "Pay USD 1,200", "Pay 40 EUR", "Pay ¥5000",
                     "Pay C$40", "Pay ＄40", "Pay €40", "Pay 40 bucks"):
            self.assertEqual(off_discord.mask(text, amounts=True), "Pay $…", text)
            self.assertNotEqual(off_discord.mask(text, amounts=False), "Pay $…", text)
            self.assertNotIn("#", off_discord.mask(text, amounts=False), text)

    def test_the_push_keeps_a_disguised_health_loop_off(self):
        loops = [{"text": "Dr. Patel appt Tue", "section": "Errands", "nudge": "2026-11-08"}]
        text = push_due_loops.render(loops, dt.date(2026, 11, 9), [], [])
        self.assertNotIn("Patel", text)
        self.assertIn("1 more kept off Discord", text)
        self.assertNotIn("Patel", push_due_loops.render(loops, dt.date(2026, 11, 9), [], None))


if __name__ == "__main__":
    unittest.main()
