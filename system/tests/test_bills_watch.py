"""Contract tests for `system/tools/bills_watch.py`, the bills watch.

Card emails are parsed by label into records, records fold into statements, and the
statements drive the Bills loops: raised the day the figures land, moved to 5 and 1 days
before due, closed on payment evidence. Every card, domain, amount and id here is invented;
no test touches Gmail, Claude, af or the network.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
import os
import re
import sys
import tempfile
import time
import unittest
from decimal import Decimal as D
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import bills_watch as bw  # noqa: E402

_SANDBOX = tempfile.TemporaryDirectory()


def setUpModule():
    # No test may touch his real Claude project folder or wait out the retry pause.
    bw.PROJECT_DIR = Path(_SANDBOX.name) / "projects" / "watch"
    bw.RETRY_WAIT = 0


def tearDownModule():
    _SANDBOX.cleanup()

CARDS = {"Northwind Rewards": ["northwind-bank.example"], "Harbour Card": ["harbourbank.example"],
         "Quiet Card": ["quietbank.example"]}
NW = "Northwind Rewards"


def msg(mid, date, sender, subject, body):
    return {"id": mid, "date": f"{date}T16:00:00Z", "sender": sender, "subject": subject, "htmlBody": body}


STATEMENT_READY = msg("m-01", "2026-03-05", "alerts@notice.northwind-bank.example",
                      "Your Online Statement is Now Ready",
                      "<html><head><style>p{margin:0}</style></head><body><p>Your Statement is now ready to view "
                      "for your Northwind Rewards Card.</p></body></html>")
BALANCE_1 = msg("m-02", "2026-03-09", "alerts@notice.northwind-bank.example", "Your Balance Update",
                "<p>Here is the latest balance as of Mar 9, 2026</p><p>New balance on last statement: CA$1,204.50 "
                "closing date Mar 4, 2026</p><p>Recent credits: CA$40.00 since Mar 5, 2026</p>"
                "<p>Recent debits: CA$310.00 since Mar 5, 2026</p><p>Current balance: CA$1,474.50</p>"
                "<p>Minimum amount due: CA$&#8203;25.00 Mar 25, 2026</p>")
REMINDER = msg("m-03", "2026-03-19", "alerts@notice.northwind-bank.example", "Payment Due Reminder",
               "<p>The payment due date is approaching.</p><p>Minimum payment due CA$25.00</p>"
               "<p>Statement balance CA$1,204.50</p><p>Please pay by 2026-03-25</p>")
MISSED = msg("m-04", "2026-03-28", "alerts@notice.northwind-bank.example", "Missed Payment",
             "<p>we haven't yet received the minimum payment that's due on your Account.</p>"
             "<p>Payment was due on 2026-03-25</p><p>New balance on last statement CA$1,204.50</p>"
             "<p>Closing date 2026-03-04</p><p>Payment due CA$25.00</p>")
BALANCE_PAID = msg("m-05", "2026-03-30", "alerts@notice.northwind-bank.example", "Your Balance Update",
                   "<p>New balance on last statement: CA$1,204.50 closing date Mar 4, 2026</p>"
                   "<p>Recent credits: CA$1,244.50 since Mar 5, 2026</p><p>Current balance: CA$600.00</p>")
HARBOUR = msg("m-06", "2026-03-10", "statements@e.harbourbank.example", "Your statement is available",
              "<p>Card ending in: 0000</p><p>Minimum payment: $0.00 Payment due date:April 2, 2026 "
              "Amount due: $0.00</p>")


def records(*messages):
    return [r for m in messages if (r := bw.record_from_message(m, CARDS))]


class ParseTests(unittest.TestCase):
    def test_clean_text_drops_markup_styles_and_invisible_characters(self):
        text = bw.clean_text("<head><style>a{}</style></head><p>Due&nbsp;CA$‌5.00</p><!-- x -->")
        self.assertEqual(text, "Due CA$5.00")

    def test_subjects_classify_and_offers_and_enrolments_do_not(self):
        cases = {
            "Missed Payment": "missed", "Late fee charged": "missed", "Payment Due Reminder": "reminder",
            "Payment received, thank you": "payment", "Your statement is available": "statement",
            "Your Online Statement is Now Ready": "statement", "Your Balance Update": "balance",
            "Special offer: Transfer balances & save on interest!": None,
            "Electronic Payment Program Enrollment Confirmation": None,
            "Presale tickets for your statement piece": None, "Your username": None,
        }
        for subject, kind in cases.items():
            self.assertEqual(bw.classify(subject), kind, subject)

    def test_balance_update_yields_closing_due_balance_minimum_and_credits(self):
        r = bw.record_from_message(BALANCE_1, CARDS)
        self.assertEqual((r.card, r.kind, r.date), (NW, "balance", dt.date(2026, 3, 9)))
        self.assertEqual(r.fields, {
            "closing": dt.date(2026, 3, 4), "due": dt.date(2026, 3, 25), "statement": D("1204.50"),
            "minimum": D("25.00"), "current": D("1474.50"), "credits": D("40.00"), "since": dt.date(2026, 3, 5)})

    def test_reminder_reads_pay_by_and_not_the_minimum_as_the_balance(self):
        f = bw.record_from_message(REMINDER, CARDS).fields
        self.assertEqual(f, {"due": dt.date(2026, 3, 25), "statement": D("1204.50"), "minimum": D("25.00")})

    def test_missed_payment_reads_the_amount_past_due_as_the_minimum(self):
        f = bw.record_from_message(MISSED, CARDS).fields
        self.assertEqual(f["minimum"], D("25.00"))
        self.assertEqual((f["due"], f["closing"], f["statement"]),
                         (dt.date(2026, 3, 25), dt.date(2026, 3, 4), D("1204.50")))

    def test_statement_email_with_long_dates_and_no_space_after_the_colon(self):
        r = bw.record_from_message(HARBOUR, CARDS)
        self.assertEqual((r.card, r.kind), ("Harbour Card", "statement"))
        self.assertEqual(r.fields, {"due": dt.date(2026, 4, 2), "statement": D("0.00"), "minimum": D("0.00")})

    def test_sender_matches_a_card_domain_or_subdomain_only(self):
        self.assertEqual(bw.card_for("a@notice.northwind-bank.example", CARDS), NW)
        self.assertEqual(bw.card_for("Bank <a@northwind-bank.example>", CARDS), NW)
        self.assertIsNone(bw.card_for("a@e.email-northwind-bank.example", CARDS))
        self.assertIsNone(bw.record_from_message(msg("x", "2026-03-01", "a@other.example", "Missed Payment", ""), CARDS))

    def test_a_record_line_round_trips(self):
        for m in (BALANCE_1, REMINDER, MISSED, HARBOUR, STATEMENT_READY):
            r = bw.record_from_message(m, CARDS)
            self.assertEqual(bw.parse_record(bw.record_line(r)), r)


class StatementTests(unittest.TestCase):
    def test_the_cycle_folds_into_one_statement_paid_when_credits_cover_the_balance(self):
        sts = bw.statements(records(STATEMENT_READY, BALANCE_1, REMINDER, MISSED, BALANCE_PAID))
        self.assertEqual(len(sts), 1)
        s = sts[0]
        self.assertEqual((s.closing, s.due, s.balance, s.minimum), (dt.date(2026, 3, 4), dt.date(2026, 3, 25),
                                                                     D("1204.50"), D("25.00")))
        self.assertEqual(s.ready, dt.date(2026, 3, 5))
        self.assertEqual([r.id for r in s.missed], ["m-04"])
        self.assertEqual((s.paid_on, s.paid_basis), (dt.date(2026, 3, 30), "credits since closing cover the balance"))
        self.assertEqual(s.ids, ["m-01", "m-02", "m-03", "m-04", "m-05"])

    def test_credits_that_cover_only_the_minimum_are_not_payment(self):
        # Refunds count as credits: $40 of them already covers a $25 minimum before any payment.
        s = bw.statements(records(BALANCE_1, REMINDER))[0]
        self.assertIsNone(s.paid_on)

    def test_a_payment_email_pays_the_statement_whose_balance_it_matches_before_the_nearest_due(self):
        april = msg("m-07", "2026-04-08", "alerts@notice.northwind-bank.example", "Your Balance Update",
                    "<p>New balance on last statement: CA$300.00 closing date Apr 4, 2026</p>"
                    "<p>Minimum amount due: CA$10.00 Apr 25, 2026</p>")
        paid = msg("m-08", "2026-04-10", "alerts@notice.northwind-bank.example", "Payment received",
                   "<p>Payment amount: $1,204.50</p>")
        sts = bw.statements(records(BALANCE_1, april, paid))
        self.assertEqual([(s.due, s.paid_on) for s in sts],
                         [(dt.date(2026, 3, 25), dt.date(2026, 4, 10)), (dt.date(2026, 4, 25), None)])

    def test_a_payment_below_the_minimum_pays_nothing(self):
        small = msg("m-09", "2026-03-12", "alerts@notice.northwind-bank.example", "Payment received",
                    "<p>Payment amount: $5.00</p>")
        self.assertIsNone(bw.statements(records(BALANCE_1, small))[0].paid_on)

    def test_a_figureless_statement_email_merges_with_its_reminder_and_the_payment_pays_it(self):
        # Review finding 1: the reminder names no closing date, so the two used to stay apart and
        # the payment credited the empty, older statement while the real one nagged.
        reminder = msg("m-03", "2026-03-19", "alerts@notice.northwind-bank.example", "Payment Due Reminder",
                       "<p>Minimum payment due CA$25.00</p><p>Statement balance CA$1,204.50</p>"
                       "<p>Please pay by 2026-03-25</p>")
        paid = msg("m-10", "2026-03-22", "alerts@notice.northwind-bank.example", "Payment received",
                   "<p>Payment amount: $1,204.50</p>")
        sts = bw.statements(records(STATEMENT_READY, reminder, paid))
        self.assertEqual(len(sts), 1, "no orphan statement")
        self.assertEqual((sts[0].ready, sts[0].due, sts[0].paid_on, sts[0].paid_basis),
                         (dt.date(2026, 3, 5), dt.date(2026, 3, 25), dt.date(2026, 3, 22), "payment email"))
        self.assertEqual(bw.plan(sts, {}, set(), dt.date(2026, 3, 23)), [])

    def test_a_payment_credits_the_statement_with_the_nearest_due_date_not_the_oldest(self):
        # An older figure-less statement email too far back to merge stays an empty statement;
        # the payment must still credit the real one.
        out = bw.statements([
            bw.Record(dt.date(2026, 2, 1), NW, "statement", "m-12"),
            bw.Record(dt.date(2026, 3, 9), NW, "balance", "m-13",
                      {"closing": dt.date(2026, 3, 4), "due": dt.date(2026, 3, 25),
                       "statement": D("1204.50"), "minimum": D("25.00")}),
            bw.Record(dt.date(2026, 3, 22), NW, "payment", "m-11", {"paid": D("1204.50")})])
        self.assertEqual([(s.start, s.paid_on) for s in out],
                         [(dt.date(2026, 2, 1), None), (dt.date(2026, 3, 4), dt.date(2026, 3, 22))])

    def test_a_missed_payment_email_with_no_figures_still_raises_its_loop(self):
        # Review finding 6: a missed-payment email that states no amount or date was dropped.
        bare = msg("m-14", "2026-03-28", "alerts@notice.northwind-bank.example", "Missed Payment",
                   "<p>We have not received your payment. Please pay now.</p>")
        r = bw.record_from_message(bare, CARDS)
        self.assertEqual((r.kind, r.fields), ("missed", {}))
        sts = bw.statements([r])
        self.assertEqual(len(sts), 1)
        acts = bw.plan(sts, {}, set(), dt.date(2026, 3, 28))
        self.assertEqual([(a.verb, a.nudge) for a in acts], [("add", dt.date(2026, 3, 28))])
        self.assertEqual(acts[0].text, "Northwind Rewards missed a payment: statement of 2026-03-28, a payment is "
                                       "overdue; pay it now (missed-payment email 2026-03-28)")
        self.assertTrue(bw.MISSED_RE.match(acts[0].text))
        # with a statement already known, the bare email joins it
        sts = bw.statements(records(BALANCE_1) + [r])
        self.assertEqual(len(sts), 1)
        self.assertEqual([m.id for m in sts[0].missed], ["m-14"])

    def test_a_zero_statement_is_nothing_due(self):
        s = bw.statements(records(HARBOUR))[0]
        self.assertTrue(s.nothing_due)
        self.assertEqual(bw.status(s, {}, set()), "nothing due")


def unpaid(due=dt.date(2026, 3, 25)):
    return bw.Statement(card=NW, closing=dt.date(2026, 3, 4), due=due, balance=D("1204.50"), minimum=D("25.00"),
                        first=dt.date(2026, 3, 9))


PAY = "Pay Northwind Rewards by 2026-03-25: statement of 2026-03-04, $1,204.50, minimum $25.00"
PAY_KEY = "Pay Northwind Rewards by 2026-03-25: statement of 2026-03-04"
MISSED_LOOP = ("Northwind Rewards missed a payment: statement of 2026-03-04, $25.00 was due 2026-03-25; "
          "pay it now (missed-payment email 2026-03-28)")


def simulate(loops: dict, acts: list) -> dict:
    """Apply loop changes to an in-memory open.md, the way af open would."""
    loops = dict(loops)
    for a in acts:
        if a.verb == "add":
            loops[a.text] = a.nudge
        elif a.verb == "redate":
            loops[a.text] = a.nudge
        else:
            loops.pop(a.text)
    return loops


class TimingTests(unittest.TestCase):
    def test_next_nudge_walks_five_days_then_one_day_then_stays_due(self):
        due = dt.date(2026, 3, 25)
        cases = [
            (dt.date(2026, 3, 9), dt.date(2026, 3, 9), None),     # pushed today: stay
            (dt.date(2026, 3, 10), dt.date(2026, 3, 9), dt.date(2026, 3, 20)),
            (dt.date(2026, 3, 20), dt.date(2026, 3, 20), None),
            (dt.date(2026, 3, 21), dt.date(2026, 3, 20), dt.date(2026, 3, 24)),
            (dt.date(2026, 3, 23), dt.date(2026, 3, 9), dt.date(2026, 3, 24)),  # missed runs skip ahead
            (dt.date(2026, 3, 25), dt.date(2026, 3, 24), None),   # past one day before: due each morning
            (dt.date(2026, 3, 30), dt.date(2026, 3, 24), None),
        ]
        for today, nudge, want in cases:
            self.assertEqual(bw.next_nudge(today, due, nudge), want, (today, nudge))

    def test_a_new_statement_raises_its_pay_loop_nudged_today(self):
        acts = bw.plan([unpaid()], {}, set(), dt.date(2026, 3, 9))
        self.assertEqual([(a.verb, a.text, a.nudge) for a in acts], [("add", PAY, dt.date(2026, 3, 9))])

    def test_an_open_pay_loop_moves_to_five_days_before(self):
        acts = bw.plan([unpaid()], {PAY: dt.date(2026, 3, 9)}, {PAY_KEY}, dt.date(2026, 3, 10))
        self.assertEqual([(a.verb, a.text, a.nudge) for a in acts], [("redate", PAY, dt.date(2026, 3, 20))])

    def test_a_moved_due_date_replaces_the_loop_and_leaves_no_orphan(self):
        # Review finding 2: the loop was keyed on its due date, so a new due date raised a second one.
        loops = {PAY: dt.date(2026, 3, 20)}
        acts = bw.plan([unpaid(due=dt.date(2026, 3, 26))], loops, {PAY_KEY}, dt.date(2026, 3, 21))
        self.assertEqual([a.verb for a in acts], ["close", "add"])
        self.assertEqual(acts[0].outcome, "due date moved to 2026-03-26")
        after = simulate(loops, acts)
        self.assertEqual(list(after), ["Pay Northwind Rewards by 2026-03-26: statement of 2026-03-04, "
                                       "$1,204.50, minimum $25.00"])
        self.assertEqual(list(after.values()), [dt.date(2026, 3, 21)], "it was due, so it stays due")
        self.assertEqual(bw.plan([unpaid(due=dt.date(2026, 3, 26))], after, {PAY_KEY}, dt.date(2026, 3, 21)), [])

    def test_a_due_date_moved_before_the_loop_pushed_keeps_its_stage(self):
        self.assertEqual(bw.moved_nudge(dt.date(2026, 3, 12), dt.date(2026, 3, 26), dt.date(2026, 3, 20)),
                         dt.date(2026, 3, 21))
        self.assertEqual(bw.moved_nudge(dt.date(2026, 3, 22), dt.date(2026, 3, 26), dt.date(2026, 3, 24)),
                         dt.date(2026, 3, 25))

    def test_payment_closes_the_pay_and_missed_loops_and_raises_nothing(self):
        s = unpaid()
        s.paid_on, s.paid_basis = dt.date(2026, 3, 22), "payment email"
        acts = bw.plan([s], {PAY: dt.date(2026, 3, 20), MISSED_LOOP: dt.date(2026, 3, 28)}, set(), dt.date(2026, 3, 30))
        self.assertEqual([(a.verb, a.text) for a in acts], [("close", PAY), ("close", MISSED_LOOP)])
        self.assertTrue(all("payment email" in a.outcome for a in acts))

    def test_a_loop_he_closed_is_not_raised_again_and_reads_as_marked_paid(self):
        self.assertEqual(bw.plan([unpaid()], {}, {PAY_KEY}, dt.date(2026, 3, 21)), [])
        self.assertEqual(bw.status(unpaid(), {}, {PAY_KEY}), "marked paid in chat")

    def test_a_fresh_missed_payment_raises_its_own_loop_and_an_old_one_does_not(self):
        s = unpaid()
        s.missed = [bw.Record(dt.date(2026, 3, 28), NW, "missed", "m-04")]
        loops, raised = {PAY: dt.date(2026, 3, 24)}, {PAY_KEY}
        acts = bw.plan([s], loops, raised, dt.date(2026, 3, 28))
        self.assertEqual([(a.verb, a.text, a.nudge) for a in acts], [("add", MISSED_LOOP, dt.date(2026, 3, 28))])
        self.assertEqual(bw.plan([s], loops, raised, dt.date(2026, 4, 6)), [])

    def test_history_older_than_the_horizon_raises_no_loop(self):
        self.assertEqual(bw.plan([unpaid(due=dt.date(2026, 1, 25))], {}, set(), dt.date(2026, 3, 9)), [])

    def test_a_gmail_failure_raises_one_loop_and_the_next_good_read_closes_it(self):
        acts = bw.plan([], {}, set(), dt.date(2026, 3, 9), bw.Trouble(gmail="the Gmail connector is needs-auth"))
        self.assertEqual(len(acts), 1)
        self.assertTrue(acts[0].text.startswith(bw.GMAIL_PREFIX))
        self.assertNotIn(" · ", acts[0].text)
        open_loops = {acts[0].text: dt.date(2026, 3, 9)}
        self.assertEqual(bw.plan([], open_loops, set(), dt.date(2026, 3, 10), bw.Trouble(gmail="again")), [])
        self.assertEqual([a.verb for a in bw.plan([], open_loops, set(), dt.date(2026, 3, 11))], ["close"])

    def test_a_refused_tool_raises_its_own_loop_once_and_never_says_reconnect(self):
        trouble = bw.Trouble(refused=[("t-9", "mcp__claude_ai_Questrade__create_order_instruction")])
        acts = bw.plan([], {}, set(), dt.date(2026, 3, 9), trouble)
        self.assertEqual(len(acts), 1)
        self.assertTrue(acts[0].text.startswith(f"{bw.REFUSED_PREFIX} and skipped card email thread t-9"))
        self.assertNotIn("reconnect", acts[0].text.lower())
        self.assertEqual(bw.plan([], {acts[0].text: dt.date(2026, 3, 9)}, set(), dt.date(2026, 3, 9), trouble), [])

    def test_watch_loops_reads_only_its_own_lines(self):
        open_md = ("# Open loops\n\n## Bills\n\n"
                   f"- [ ] 2026-03-09 · {PAY} · nudge 2026-03-20 · from: bills-watch\n"
                   "- [ ] 2026-03-01 · Pay the rent · nudge 2026-03-28 · every monthly · from: main\n")
        self.assertEqual(bw.watch_loops(open_md), {PAY: dt.date(2026, 3, 20)})


def stream(*events, gmail="connected", session="0a1b2c3d-0000-4000-8000-000000000001", denials=()):
    init = {"type": "system", "subtype": "init", "session_id": session,
            "mcp_servers": [{"name": "claude.ai Gmail", "status": gmail, "source": "claudeai"}],
            "tools": list(bw.READ_TOOLS)}
    done = {"type": "result", "subtype": "success", "is_error": False,
            "permission_denials": [{"tool_name": t} for t in denials]}
    return [json.dumps(e) for e in (init, *events, done)]


def call(tid, name, payload_text, is_error=False):
    return [{"type": "assistant", "message": {"content": [{"type": "tool_use", "id": tid, "name": name, "input": {}}]}},
            {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": tid, "is_error": is_error,
                                                      "content": [{"type": "text", "text": payload_text}]}]}}]


SEARCH, GET = bw.READ_TOOLS
ORDER = "mcp__claude_ai_Questrade__create_order_instruction"


class StreamTests(unittest.TestCase):
    def test_payloads_come_inline_or_from_the_saved_file(self):
        with tempfile.TemporaryDirectory() as d:
            saved = Path(d) / "toolu_x.txt"
            saved.write_text(json.dumps({"id": "t-2", "messages": [BALANCE_1]}), encoding="utf-8")
            lines = stream(*call("a", SEARCH, '{"threads": []}'),
                           *call("b", GET, f"<persisted-output>\nOutput too large (56KB). Full output saved to: "
                                           f"{saved}\n\nPreview (first 2KB):\n{{\"id\""))
            got = bw.parse_stream(lines)
        self.assertEqual([(n, p.get("id")) for n, p in got.payloads], [(SEARCH, None), (GET, "t-2")])
        self.assertEqual((got.refused, got.unreadable), ([], 0))

    def test_a_saved_file_named_in_new_wording_is_still_found(self):
        with tempfile.TemporaryDirectory() as d:
            saved = Path(d) / "result.json"
            saved.write_text(json.dumps({"id": "t-3", "messages": []}), encoding="utf-8")
            got = bw.parse_stream(stream(*call("b", GET, f"Result stored at {saved} for later reading")))
        self.assertEqual([p["id"] for _, p in got.payloads], ["t-3"])

    def test_a_disconnected_connector_fails_the_run(self):
        with self.assertRaisesRegex(bw.GmailError, "needs-auth"):
            bw.parse_stream(stream(gmail="needs-auth"))

    def test_a_refused_tool_is_reported_not_raised(self):
        got = bw.parse_stream(stream(*call("c", ORDER, "Permission denied", is_error=True), denials=[ORDER]))
        self.assertEqual(got.refused, [ORDER])

    def test_an_unreadable_result_is_counted(self):
        got = bw.parse_stream(stream(*call("b", GET, "Output saved somewhere it no longer says")))
        self.assertEqual((got.payloads, got.unreadable), ([], 1))

    def test_select_fetches_unseen_unskipped_card_threads_with_a_bill_subject(self):
        results = [{"threads": [
            {"id": "t-1", "messages": [{"id": "m-1", "sender": "a@notice.northwind-bank.example", "subject": "Missed Payment"}]},
            {"id": "t-2", "messages": [{"id": "m-2", "sender": "a@notice.northwind-bank.example", "subject": "Presale tickets"}]},
            {"id": "t-3", "messages": [{"id": "m-3", "sender": "a@other.example", "subject": "Your statement is available"}]},
            {"id": "t-4", "messages": [{"id": "m-4", "sender": "a@harbourbank.example", "subject": "Your statement is available"}]},
            {"id": "t-5", "messages": [{"id": "m-5", "sender": "a@harbourbank.example", "subject": "Payment Due Reminder"}]},
        ]}]
        self.assertEqual(bw.select(results, CARDS, seen={"m-4"}, skipped={"t-5"}), ["t-1"])

    def test_search_query_names_every_card_domain_the_bill_subjects_and_the_window(self):
        q = bw.search_query(CARDS, dt.date(2026, 3, 1))
        self.assertEqual(q, "(from:harbourbank.example OR from:northwind-bank.example OR from:quietbank.example) "
                            "subject:(statement OR payment OR balance OR due OR missed OR overdue OR late OR received) "
                            "after:2026/03/01")

    def test_the_gmail_subject_filter_admits_every_kind_classify_keeps(self):
        for subject in ("Missed Payment", "Late fee charged", "Past due notice", "Payment Due Reminder",
                        "Payment received", "Your statement is available", "Your Balance Update"):
            self.assertIsNotNone(bw.classify(subject))
            self.assertTrue(any(t in subject.lower() for t in bw.SUBJECT_TERMS), subject)


def fake_gmail(threads, *, refuse=(), unreadable=(), search_payload=None, log=None):
    """A stand-in for claude(): one search run, then one run per thread."""
    def run(prompt, max_turns, model):
        if log is not None:
            log.append(prompt)
        if "search_threads" in prompt:
            listed = [{"id": t["id"], "messages": [{"id": m["id"], "sender": m["sender"], "subject": m["subject"]}
                                                  for m in t["messages"]]} for t in threads]
            body = json.dumps(search_payload if search_payload is not None else {"threads": listed})
            return stream(*call("s", SEARCH, body))
        tid = re.search(r'threadId "([^"]+)"', prompt).group(1)
        if tid in refuse:
            return stream(*call("g", GET, json.dumps(next(t for t in threads if t["id"] == tid))),
                          *call("x", ORDER, "Permission denied", is_error=True), denials=[ORDER])
        if tid in unreadable:
            return stream(*call("g", GET, "Output parked in a place this wording no longer names"))
        return stream(*call("g", GET, json.dumps(next(t for t in threads if t["id"] == tid))))
    return run


class FetchTests(unittest.TestCase):
    def test_a_full_page_with_no_next_token_is_flagged(self):
        threads = [{"id": f"t-{i}", "messages": [{"id": f"m-{i}", "sender": "a@harbourbank.example",
                                                  "subject": "Card news"}]} for i in range(bw.PAGE)]
        self.assertTrue(bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=fake_gmail(threads)).full)
        self.assertFalse(bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=fake_gmail(threads[:3])).full)

    def test_one_run_per_thread_so_a_refusal_skips_only_its_thread(self):
        threads = [{"id": "t-1", "messages": [BALANCE_1]}, {"id": "t-2", "messages": [REMINDER]}]
        prompts = []
        got = bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=fake_gmail(threads, refuse={"t-1"}, log=prompts))
        self.assertEqual(len(prompts), 3)
        self.assertEqual(got.refused, [("t-1", ORDER)])
        self.assertEqual([m["id"] for m in got.messages], ["m-03"])

    def test_an_empty_search_is_a_quiet_day_and_a_changed_shape_is_a_failure(self):
        quiet = bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=fake_gmail([], search_payload={}))
        self.assertEqual(quiet.messages, [])
        with self.assertRaisesRegex(bw.FetchError, "format"):
            bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=fake_gmail([], search_payload={"items": []}))

    def test_a_transient_missing_connector_is_retried_once(self):
        calls = []

        def flaky(prompt, max_turns, model):
            calls.append(prompt)
            if len(calls) == 1:
                return [json.dumps({"type": "system", "subtype": "init", "mcp_servers": []}),
                        json.dumps({"type": "result", "subtype": "success"})]
            return stream(*call("s", SEARCH, "{}"))
        self.assertEqual(bw.fetch(CARDS, set(), set(), dt.date(2026, 3, 1), run=flaky).messages, [])
        self.assertEqual(len(calls), 2)

    def test_each_run_deletes_its_saved_tool_results_and_old_leftovers_but_not_memory(self):
        # Review finding 5: saved tool results hold whole email bodies.
        session = "0a1b2c3d-0000-4000-8000-000000000001"
        bw.PROJECT_DIR.mkdir(parents=True, exist_ok=True)
        own = bw.PROJECT_DIR / session / "tool-results"
        own.mkdir(parents=True)
        (own / "toolu_1.txt").write_text("<html>an email</html>", encoding="utf-8")
        old = bw.PROJECT_DIR / "9f8e7d6c-0000-4000-8000-000000000002"
        old.mkdir()
        os.utime(old, (time.time() - 7200, time.time() - 7200))
        recent = bw.PROJECT_DIR / "9f8e7d6c-0000-4000-8000-000000000003"
        recent.mkdir()
        memory = bw.PROJECT_DIR / "memory"
        memory.mkdir(exist_ok=True)
        os.utime(memory, (time.time() - 7200, time.time() - 7200))
        bw._run(lambda *a: stream(*call("s", SEARCH, "{}"), session=session), "", 1, "")
        self.assertEqual(sorted(p.name for p in bw.PROJECT_DIR.iterdir()), ["9f8e7d6c-0000-4000-8000-000000000003",
                                                                               "memory"])

    def test_the_claude_run_is_dont_ask_with_every_other_connector_denied(self):
        seen = {}

        def fake_run(args, **kw):
            seen["args"] = args
            return mock.Mock(returncode=0, stdout="", stderr="")
        with mock.patch.object(bw.shutil, "which", return_value="claude"), \
                mock.patch.object(bw.subprocess, "run", side_effect=fake_run), \
                mock.patch.object(bw, "RUN_DIR", Path(_SANDBOX.name) / "run"):
            bw.claude("prompt", 4, "model")
        args = seen["args"]
        self.assertEqual(args[args.index("--permission-mode") + 1], "dontAsk")
        self.assertEqual(args[args.index("--tools") + 1], "")
        self.assertEqual(args[args.index("--setting-sources") + 1], "local")
        self.assertEqual(args[args.index("--allowedTools") + 1].split(","), list(bw.READ_TOOLS))
        denied = args[args.index("--disallowedTools") + 1].split(",")
        for server in ("mcp__claude_ai_Questrade", "mcp__claude_ai_Google_Calendar", "mcp__claude_ai_Claude_Docs",
                       "mcp__claude_ai_Spotify", "mcp__substack", "mcp__claude-in-chrome"):
            self.assertIn(server, denied)
        self.assertIn("mcp__claude_ai_Gmail__send_message", denied)
        self.assertFalse(set(bw.READ_TOOLS) & set(denied))

    def test_the_batch_task_runs_it_first_and_its_limit_holds_every_step(self):
        import watch_batch as wb
        self.assertEqual(wb.MORNING[0][:2], ("bills", "bills_watch.py"), "the loop watch skips Bills loops, so these come first")
        ps1 = (ROOT / "system" / "tools" / "register-watch-batch.ps1").read_text(encoding="utf-8")
        m = re.search(r"-ExecutionTimeLimit \(New-TimeSpan -(Hours|Minutes) (\d+)\)", ps1)
        limit = int(m.group(2)) * (3600 if m.group(1) == "Hours" else 60)
        self.assertGreater(limit, sum(minutes for _, _, minutes in wb.MORNING) * 60)


BILLS_MD = """# Bills

## Cards

- Northwind Rewards: northwind-bank.example
- Harbour Card: harbourbank.example
- Quiet Card: quietbank.example
"""
THREADS = ({"id": "t-1", "messages": [STATEMENT_READY, BALANCE_1]},)


class MainTests(unittest.TestCase):
    def run_main(self, args, bills=BILLS_MD, open_md="# Open loops\n", today=dt.date(2026, 3, 10),
                 run=None, threads=THREADS):
        prompts, af_calls = [], []

        def fake_af(argv):
            af_calls.append(argv)
            return 0, ""

        out, err = io.StringIO(), io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            bills_path, open_path = Path(d) / "bills.md", Path(d) / "open.md"
            if bills is not None:
                bills_path.write_text(bills, encoding="utf-8")
            open_path.write_text(open_md, encoding="utf-8")
            log = Path(d) / "bills-watch.log"
            with mock.patch.object(bw, "LOG", log), contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = bw.main(args + ["--bills", str(bills_path), "--open", str(open_path)], today=today,
                               now=dt.datetime.combine(today, dt.time(7, 45)),
                               run_claude=run or fake_gmail(list(threads), log=prompts), run_af=fake_af)
            written = bills_path.read_text(encoding="utf-8") if bills_path.exists() else None
            logged = log.read_text(encoding="utf-8") if log.exists() else ""
        return code, out.getvalue(), err.getvalue(), written, af_calls, prompts, logged

    def test_dry_run_prints_the_table_and_the_loop_it_would_raise_and_writes_nothing(self):
        code, out, _, written, af_calls, prompts, logged = self.run_main(["--dry-run"])
        self.assertEqual(code, 0)
        self.assertIn("| Northwind Rewards | 2026-03-04 | $1,204.50 | $25.00 | 2026-03-25 | unpaid | m-01, m-02 |", out)
        self.assertIn("| Quiet Card | no statement email yet |", out)
        self.assertIn(f"would raise loop (nudge 2026-03-10): {PAY}", out)
        self.assertEqual((written, af_calls, logged), (BILLS_MD, [], ""))
        self.assertIn("after:2026/01/09", prompts[0])

    def test_a_real_run_raises_the_loop_records_it_and_rereads_from_the_last_read(self):
        code, _, _, written, af_calls, _, logged = self.run_main([])
        self.assertEqual(code, 0)
        self.assertEqual(af_calls, [["open", "add", PAY, "--nudge", "2026-03-10", "--from", "bills-watch",
                                     "--section", "Bills"]])
        led = bw.parse_bills(written)
        self.assertEqual(led.raised, [f"2026-03-10 · {PAY_KEY}"])
        self.assertEqual([r.id for r in led.records], ["m-01", "m-02"])
        self.assertEqual(led.read, dt.datetime(2026, 3, 10, 7, 45))
        self.assertIn("2 new email(s)", logged)
        code, _, _, _, af_calls, prompts, _ = self.run_main([], bills=written, today=dt.date(2026, 3, 11))
        self.assertIn("after:2026/03/07", prompts[0])
        self.assertEqual(len(prompts), 1, "nothing unseen: no get_thread run")
        self.assertEqual(af_calls, [], "the loop was raised once; he may have closed it")

    def test_a_gmail_failure_keeps_the_last_read_and_raises_the_failure_loop(self):
        def broken(prompt, max_turns, model):
            raise bw.GmailError("the Gmail connector is needs-auth")
        code, _, _, written, af_calls, _, _ = self.run_main([], bills=BILLS_MD + "\nGmail read 2026-03-01 07:45.\n",
                                                            run=broken)
        led = bw.parse_bills(written)
        self.assertEqual(code, 1)
        self.assertEqual(led.read, dt.datetime(2026, 3, 1, 7, 45))
        self.assertEqual(af_calls[0][:2], ["open", "add"])
        self.assertTrue(af_calls[0][2].startswith(bw.GMAIL_PREFIX))
        self.assertEqual(led.raised, [], "the failure loop is not a statement loop")

    def test_an_unreadable_thread_raises_the_failure_loop_and_keeps_the_cursor(self):
        # Review finding 3: an unreadable thread used to exit 0 with no loop.
        threads = [{"id": "t-1", "messages": [BALANCE_1]}, {"id": "t-2", "messages": [REMINDER]}]
        code, _, _, written, af_calls, _, _ = self.run_main(
            [], run=fake_gmail(threads, unreadable={"t-2"}), threads=threads)
        led = bw.parse_bills(written)
        self.assertEqual(code, 1)
        self.assertIsNone(led.read)
        self.assertEqual([r.id for r in led.records], ["m-02"], "the readable thread is kept")
        texts = [c[2] for c in af_calls if c[1] == "add"]
        self.assertTrue(any(t.startswith(f"{bw.FAIL_PREFIX}: 1 card email thread(s) came back unreadable (t-2)")
                            for t in texts), texts)

    def test_a_run_that_reads_no_figures_raises_the_failure_loop_and_stores_nothing(self):
        # Review finding 3: a format change gave 0 emails and still advanced the cursor.
        blank = msg("m-20", "2026-03-09", "alerts@notice.northwind-bank.example", "Your Balance Update",
                    "<p>A new layout with no labels this file knows</p>")
        code, _, _, written, af_calls, _, _ = self.run_main([], threads=[{"id": "t-1", "messages": [blank]}])
        led = bw.parse_bills(written)
        self.assertEqual(code, 1)
        self.assertEqual((led.read, led.records), (None, []))
        self.assertTrue(any(c[2].startswith(f"{bw.FAIL_PREFIX}: read 1 card email(s) but found no figures")
                            for c in af_calls if c[1] == "add"))

    def test_any_crash_is_logged_and_raises_the_failure_loop(self):
        # Review finding 3: main caught only GmailError.
        def crash(prompt, max_turns, model):
            raise KeyError("id")
        code, _, err, _, af_calls, _, logged = self.run_main([], run=crash)
        self.assertEqual(code, 1)
        self.assertIn("KeyError", err)
        self.assertIn("failed: it crashed (KeyError", logged)
        self.assertEqual(len(af_calls), 1)
        self.assertTrue(af_calls[0][2].startswith(f"{bw.FAIL_PREFIX}: it crashed (KeyError"))

    def test_a_refused_tool_skips_its_thread_for_good_and_raises_its_own_loop(self):
        # Review finding 4: an email that steers a tool call must not wedge the run.
        threads = [{"id": "t-1", "messages": [BALANCE_1]}, {"id": "t-2", "messages": [REMINDER]}]
        code, _, _, written, af_calls, _, _ = self.run_main([], run=fake_gmail(threads, refuse={"t-2"}), threads=threads)
        led = bw.parse_bills(written)
        self.assertEqual(code, 1)
        self.assertEqual(led.skipped, [f"2026-03-10 · thread t-2 · {ORDER}"])
        self.assertEqual(led.read, dt.datetime(2026, 3, 10, 7, 45), "the skip does not hold the cursor back")
        texts = [c[2] for c in af_calls if c[1] == "add"]
        self.assertTrue(any(t.startswith(f"{bw.REFUSED_PREFIX} and skipped card email thread t-2") for t in texts))
        self.assertFalse(any(t.startswith(bw.GMAIL_PREFIX) for t in texts))
        prompts = []
        self.run_main([], bills=written, today=dt.date(2026, 3, 11), run=fake_gmail(threads, log=prompts),
                      threads=threads)
        self.assertEqual(len(prompts), 1, "the skipped thread is not fetched again")

    def test_a_missing_or_cardless_bills_file_names_the_fix_and_raises_the_failure_loop(self):
        for bills in (None, "# Bills\n"):
            code, _, err, _, af_calls, prompts, _ = self.run_main([], bills=bills)
            self.assertEqual((code, prompts), (2, []))
            self.assertIn("## Cards", err)
            self.assertEqual(len(af_calls), 1)
            self.assertTrue(af_calls[0][2].startswith(bw.FAIL_PREFIX))
        code, _, _, _, af_calls, _, _ = self.run_main(["--dry-run"], bills=None)
        self.assertEqual((code, af_calls), (2, []))


if __name__ == "__main__":
    unittest.main()
