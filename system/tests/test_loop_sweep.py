"""Contract tests for `system/tools/loop_sweep.py`, the daily loop sweep.

A model proposes a JSON plan from the loops; Python checks every action against fixed rules and
applies the ones that pass through af. Every loop, sender and amount here is invented; no test
touches Claude, af, Discord or the network.
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
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import loop_sweep as ls  # noqa: E402

TODAY = dt.date(2026, 3, 10)
NOW = dt.datetime(2026, 3, 10, 8, 20)

OPEN = """# Open loops

<!-- grammar: - [ ] YYYY-MM-DD · text · nudge YYYY-MM-DD · from: <id>
     - [ ] this commented line is not a loop · from: main -->

## Waiting on the operator

### Errands
- [ ] 2026-03-01 · Ask the neighbour about the parcel on 03-08 · nudge 2026-03-08 · from: main
- [ ] 2026-03-01 · Pay the gym fee of $40 by 03-08 · nudge 2026-03-07 · from: src-2026-03-01-01
- [ ] 2026-03-02 · Claim the e-transfer from the club, $12.00 · nudge 2026-03-09 · from: src-2026-03-02-01
- [ ] 2026-03-01 · Buy a warm winter hat and gloves · nudge 2026-03-02 · from: main
- [ ] 2026-03-03 · verdict: Corner Cafe, brunch 03-01 · nudge 2026-03-12 · from: src-a
- [ ] 2026-03-04 · verdict: River Gallery, show 03-02 · nudge 2026-03-11 · from: src-b
- [ ] 2026-03-01 · Weekly weigh-in: weight and waist to the fitness log · nudge 2026-03-08 · every weekly · from: main
- [ ] 2026-03-05 · Write the monthly finances review into months/2026-03.md · nudge 2026-03-09 · from: main
- [ ] 2026-03-02 · Pick up the widget from Widget Shop · nudge 2026-03-05 · from: main
- [ ] 2026-03-03 · Book the dentist cleaning appointment · nudge 2026-03-05 · from: main
* not a loop line at all
- [ ] 2026-03-06 · Book the dentist cleaning appointment soon · nudge 2026-03-20 · from: main

## Waiting on others

- [ ] 2026-03-01 · Sam owes $15 for the tickets · nudge 2026-03-05 · from: main

## Decisions needed

- [ ] 2026-03-01 · Renew or move? · need now · detail needs/renew-or-move.md · from: main

## Bills

- [ ] 2026-03-02 · Pay Test Card by 2026-03-20: statement of 2026-03-01, $100.00, minimum $10.00 · nudge 2026-03-15 · from: bills-watch
"""

N = {  # af's numbers for OPEN
    "neighbour": 1, "gym": 2, "etransfer": 3, "hat": 4, "cafe": 5, "gallery": 6, "weigh": 7, "finances": 8,
    "widget": 9, "dentist_old": 10, "dentist_new": 11, "others": 12, "needs": 13, "bills": 14,
}


def loops():
    return ls.parse_loops(OPEN)


def sweep():
    return ls.Sweep(loops(), TODAY)


def kinds(over=None):
    base = {N["neighbour"]: ls.Kind("personal", dt.date(2026, 3, 8), False)}
    base.update(over or {})
    return base


def fake_af_for(text_holder):
    """An af that numbers like af (by this file's parser, checked separately) and records writes."""
    calls = []

    def run(root, args):
        calls.append(args)
        if args[:3] == ["open", "due", "--json"]:
            text = (Path(root) / ls.OPEN).read_text(encoding="utf-8")
            due = [{"n": x.n, "text": x.text} for x in ls.parse_loops(text) if x.due_on() and x.due_on() <= TODAY]
            return 0, json.dumps(due)
        return 0, "ok"
    return run, calls


class ParseTests(unittest.TestCase):
    def test_loops_are_numbered_as_af_numbers_them(self):
        ls_ = loops()
        self.assertEqual(len(ls_), 14)
        self.assertEqual(ls_[0].text, "Ask the neighbour about the parcel on 03-08")
        self.assertEqual((ls_[0].section, ls_[0].top), ("Errands", "Waiting on the operator"))
        self.assertEqual(ls_[N["weigh"] - 1].every, "weekly")
        needs = ls_[N["needs"] - 1]
        self.assertEqual((needs.need, needs.detail, needs.nudge), ("now", "needs/renew-or-move.md", None))
        self.assertIn("needs list", needs.owned)
        self.assertIn("bills watch", ls_[N["bills"] - 1].owned)
        self.assertIn("someone else", ls_[N["others"] - 1].owned)
        self.assertIsNone(ls_[N["hat"] - 1].owned)

    def test_a_line_that_breaks_the_grammar_gets_no_number(self):
        text = OPEN.replace("- [ ] 2026-03-01 · Pay the gym fee", "- [ ] 2026-13-01 · Pay the gym fee")
        self.assertEqual(len(ls.parse_loops(text)), 13)
        self.assertEqual(ls.parse_loops(text)[1].text, "Claim the e-transfer from the club, $12.00")

    def test_the_numbering_check_passes_on_agreement_and_stops_on_drift(self):
        ls_ = loops()
        made_due = ls.parse_loops(ls.everything_due(OPEN, ls_))
        self.assertTrue(all(x.due_on() == dt.date(2000, 1, 1) for x in made_due))
        agree = json.dumps([{"n": x.n, "text": x.text} for x in made_due])
        self.assertIsNone(ls.numbering_agrees(ls_, agree))
        drift = json.dumps([{"n": x.n + (1 if x.n > 3 else 0), "text": x.text} for x in made_due])
        self.assertIn("differently", ls.numbering_agrees(ls_, drift))
        self.assertIn("differently", ls.numbering_agrees(ls_, json.dumps([])))

    def test_the_numbering_check_runs_on_a_copy(self):
        seen = {}

        def run(root, args):
            seen["root"] = root
            return 0, json.dumps([{"n": x.n, "text": x.text} for x in loops()])
        with tempfile.TemporaryDirectory() as d:
            self.assertIsNone(ls.check_numbering(Path(d), OPEN, loops(), run))
        self.assertNotEqual(seen["root"], Path(d))
        self.assertIn("agentframe-sweep-", str(seen["root"]))


class KindTests(unittest.TestCase):
    def test_the_loops_own_words_name_its_stakes(self):
        self.assertIn("money", ls.heuristic_stakes("Pay the gym fee of $40"))
        self.assertEqual(ls.heuristic_stakes("Buy a winter hat, up to $50"), ["purchase"])
        self.assertIn("deadline", ls.heuristic_stakes("Buy the ticket before it fills"))
        self.assertIn("deadline", ls.heuristic_stakes("Tickets sell out fast"))
        self.assertIn("money", ls.heuristic_stakes("Claim the e-transfer from the club"))
        self.assertIn("deadline", ls.heuristic_stakes("Sign-up closes, say yes by Wed"))
        self.assertIn("deadline", ls.heuristic_stakes("Written notice by 03-19 stops the charge"))
        self.assertIn("booking", ls.heuristic_stakes("Book the dentist cleaning"))
        self.assertEqual(ls.heuristic_stakes("Ask the neighbour about the parcel on 03-08"), [])
        self.assertEqual(ls.heuristic_stakes("Talk to the manager Monday morning, before you leave"), [])

    def test_the_model_cannot_make_a_money_loop_personal(self):
        gym = loops()[N["gym"] - 1]
        self.assertEqual(ls.kind_of(gym, ls.Kind("personal")), "money")
        neighbour = loops()[N["neighbour"] - 1]
        self.assertEqual(ls.stakes_of(neighbour, ls.Kind("money")), ["money"])
        self.assertEqual(ls.stakes_of(neighbour, ls.Kind("personal", stakes=True)), ["stakes"])
        self.assertEqual(ls.stakes_of(neighbour, ls.Kind("personal")), [])

    def test_a_plain_purchase_or_booking_is_guarded_but_not_urgent(self):
        lp = {x.n: x for x in loops()}
        hat, dentist, gym, et = lp[N["hat"]], lp[N["dentist_old"]], lp[N["gym"]], lp[N["etransfer"]]
        self.assertEqual(ls.stakes_of(hat, ls.Kind("money")), ["purchase", "money"], "never closed on its date")
        self.assertEqual(ls.urgency_of(hat, ls.Kind("money")), [], "but re-dated like any to-do")
        self.assertEqual(ls.urgency_of(dentist, ls.Kind("booking")), [])
        self.assertEqual(ls.urgency_of(gym, None), ["money", "deadline"])
        self.assertEqual(ls.urgency_of(et, None), ["money"])
        self.assertEqual(ls.urgency_of(hat, ls.Kind("money", stakes=True)), ["stakes"], "the model may say a deadline waits")

    def test_a_date_is_named_in_any_of_his_usual_forms(self):
        d = dt.date(2026, 3, 8)
        for text in ("on 2026-03-08", "Sun 03-08", "on 3/8 at noon", "Mar 8", "March 8th", "8 Mar"):
            self.assertTrue(ls.mentions_date(text, d), text)
        for text in ("on 03-18", "2026-03-09", "13/8", "Mar 18"):
            self.assertFalse(ls.mentions_date(text, d), text)


WATCH = """# Loop watch

header

## Claim the e-transfer from the club, $12.00

- Verdict: done
- From: `notify@payments.interac.ca (Interac)`
- Date: 2026-03-09
- Subject: `Deposit complete`
- Quote: `your money has been deposited to your account`

## Pick up the widget from Widget Shop

- Verdict: update (the model said done; it is from his own sent mail)
- From: `orders@widgets.example (Widget Shop)`
- Date: 2026-03-09
- Subject: `Re: pickup`
- Quote: `your widget will be ready next week`

## Failures

- Something: malformed reply

Run 2026-03-10 07:50 · loops checked 3 · with evidence 2 · failures 1 · skipped over the cap 0 · cost $0.05 · undeleted 0
"""


class WatchTests(unittest.TestCase):
    def test_only_todays_report_counts(self):
        hits = ls.read_watch(WATCH, TODAY)
        self.assertEqual(set(hits), {"Claim the e-transfer from the club, $12.00", "Pick up the widget from Widget Shop"})
        self.assertEqual(hits["Claim the e-transfer from the club, $12.00"].address, "notify@payments.interac.ca")
        self.assertEqual(ls.read_watch(WATCH, TODAY + dt.timedelta(days=1)), {})
        self.assertEqual(ls.read_watch("# Loop watch\n\nNo footer.\n", TODAY), {})

    def test_senders_must_fit_the_loop(self):
        lp = {x.n: x for x in loops()}
        et, gym, widget, dentist = lp[N["etransfer"]], lp[N["gym"]], lp[N["widget"]], lp[N["dentist_old"]]
        self.assertEqual(ls.sender_fits(et, "money", "notify@payments.interac.ca", []), "Interac")
        self.assertIsNone(ls.sender_fits(et, "money", "club@clubmail.example", []), "an e-transfer settles only from Interac")
        self.assertIsNone(ls.sender_fits(et, "money", "someone@interac.ca.evil.example", []))
        self.assertIsNone(ls.sender_fits(gym, "money", "statements@card.example", ["card.example"]),
                          "a card issuer settles only a loop that names the card")
        named = one("Pay the Fernhollow card statement").loops[0]
        self.assertEqual(ls.sender_fits(named, "money", "statements@fernhollow.example", ["fernhollow.example"]),
                         "his card issuer")
        self.assertIsNone(ls.sender_fits(gym, "money", "statements@card.example", []))
        self.assertEqual(ls.sender_fits(widget, "other", "orders@widget.example", []), "the sender the loop names")
        self.assertEqual(ls.sender_fits(dentist, "booking", "no-reply@janeapp.com", []), "a ticketing platform")
        self.assertIsNone(ls.sender_fits(widget, "other", "widget@gmail.com", []), "a free mail address settles nothing")
        self.assertIsNone(ls.sender_fits(gym, "money", "ads@mail.example", []))

    def test_a_done_from_a_fitting_sender_closes_and_a_capped_verdict_does_not(self):
        sw = sweep()
        ls.watch_closes(sw, ls.read_watch(WATCH, TODAY), {}, [])
        self.assertEqual(len(sw.changes), 1)
        c = sw.changes[0]
        self.assertEqual((c.verb, c.loop.n), ("close", N["etransfer"]))
        self.assertTrue(c.outcome.startswith("Sweep 2026-03-10: "))
        self.assertIn("payments.interac.ca", c.outcome)
        self.assertNotIn("deposited", c.outcome, "no email text in an outcome")

    def test_a_done_from_the_wrong_sender_is_refused(self):
        sw = sweep()
        report = WATCH.replace("notify@payments.interac.ca (Interac)", "club@clubmail.example (Club)")
        ls.watch_closes(sw, ls.read_watch(report, TODAY), {}, [])
        self.assertEqual(sw.changes, [])
        self.assertIn("does not fit", sw.refused[0][1])


class ActionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "workspace" / "life" / "systems").mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, actions, k=None, files=None, report="", sw=None):
        sw = sw or sweep()
        ls.check_plan(sw, ls.Plan(kinds=k if k is not None else kinds(), actions=actions), self.root, files or {}, report)
        return sw

    def test_a_personal_day_that_passed_closes_unconfirmed(self):
        sw = self.check([{"do": "close_day_passed", "id": N["neighbour"], "reason": "a one-day chat"}])
        self.assertEqual(sw.refused, [])
        self.assertEqual(sw.changes[0].outcome, "Sweep 2026-03-10: day passed (03-08), unconfirmed")

    def test_a_passed_date_never_closes_a_money_deadline_or_booking_loop(self):
        k = kinds({N["gym"]: ls.Kind("personal", dt.date(2026, 3, 8), False),
                   N["dentist_old"]: ls.Kind("personal", dt.date(2026, 3, 5), False)})
        sw = self.check([{"do": "close_day_passed", "id": N["gym"]},
                         {"do": "close_day_passed", "id": N["dentist_old"]}], k)
        self.assertEqual(sw.changes, [])
        self.assertEqual(len(sw.refused), 2)

    def test_a_day_passed_close_needs_a_personal_kind_a_past_day_and_that_day_in_the_text(self):
        for kind in (ls.Kind("other", dt.date(2026, 3, 8)), ls.Kind("personal", TODAY),
                     ls.Kind("personal", dt.date(2026, 3, 7)), ls.Kind("personal", None),
                     ls.Kind("personal", dt.date(2026, 3, 8), stakes=True)):
            sw = self.check([{"do": "close_day_passed", "id": N["neighbour"]}], {N["neighbour"]: kind})
            self.assertEqual(sw.changes, [], kind)

    def test_a_vault_line_with_a_date_in_the_window_closes(self):
        log = self.root / "workspace" / "life" / "systems" / "fitness-log.md"
        log.write_text("- 2026-03-01 · weight 80.0, waist 90\n- 2026-03-08 · weight 79.5 kg, waist 89 cm\n", encoding="utf-8")
        rel = "workspace/life/systems/fitness-log.md"
        files = {rel: log.read_text(encoding="utf-8")}
        sw = self.check([{"do": "close_vault", "id": N["weigh"], "file": rel, "quote": "weight 79.5 kg, waist 89 cm"}],
                        files=files)
        self.assertEqual(sw.refused, [])
        self.assertIn("fitness-log.md has an entry of 2026-03-08", sw.changes[0].outcome)
        self.assertFalse(sw.changes[0].final, "a recurring loop closed on evidence re-dates one period on")
        old = self.check([{"do": "close_vault", "id": N["weigh"], "file": rel, "quote": "weight 80.0, waist 90"}],
                         files=files)
        self.assertEqual(old.changes, [], "last period's entry cannot settle this one")
        other = self.check([{"do": "close_vault", "id": N["weigh"], "file": "workspace/elsewhere.md",
                             "quote": "weight 79.5 kg, waist 89 cm"}], files=files)
        self.assertIn("not one of his evidence files", other.refused[0][1])

    def test_a_file_the_loop_asked_for_closes_only_inside_workspace(self):
        target = self.root / "workspace" / "life" / "systems" / "finances" / "months" / "2026-03.md"
        target.parent.mkdir(parents=True)
        target.write_text("# March\n", encoding="utf-8")
        sw = self.check([{"do": "close_file", "id": N["finances"], "file": "workspace/life/systems/finances/months/2026-03.md"}])
        self.assertEqual(sw.refused, [])
        self.assertIn("exists, written", sw.changes[0].outcome)
        for bad in ("../outside.md", "C:/Windows/win.ini", "/etc/passwd", "workspace/../system/x.md",
                    "workspace/life/systems/finances/months/2026-04.md"):
            sw = self.check([{"do": "close_file", "id": N["finances"], "file": bad}])
            self.assertEqual(sw.changes, [], bad)

    def test_duplicates_keep_the_newer_loop_and_share_words(self):
        sw = self.check([{"do": "end_duplicate", "id": N["dentist_old"], "keep": N["dentist_new"]}])
        self.assertEqual(sw.refused, [])
        self.assertIn("duplicate of a loop kept open", sw.changes[0].outcome)
        older = self.check([{"do": "end_duplicate", "id": N["dentist_new"], "keep": N["dentist_old"]}])
        self.assertIn("older", older.refused[0][1])
        unlike = self.check([{"do": "end_duplicate", "id": N["hat"], "keep": N["dentist_new"]}])
        self.assertIn("too few words", unlike.refused[0][1])
        bills = self.check([{"do": "end_duplicate", "id": N["bills"], "keep": N["gym"]}])
        self.assertIn("owned", bills.refused[0][1])

    def test_a_hand_raised_bill_ends_as_a_duplicate_of_the_bills_watch_loop(self):
        text = OPEN.replace("Pay the gym fee of $40 by 03-08", "Pay the Test Card statement by 2026-03-20: 2026-03-01, $100.00, minimum $10.00")
        sw = ls.Sweep(ls.parse_loops(text), TODAY)
        self.check([{"do": "end_duplicate", "id": N["gym"], "keep": N["bills"]}], sw=sw)
        self.assertEqual(sw.refused, [])

    def test_a_recurring_duplicate_ends_for_good(self):
        text = OPEN.replace("- [ ] 2026-03-05 · Write the monthly", "- [ ] 2026-03-09 · Weekly weigh-in: weight and waist "
                            "to the fitness log · nudge 2026-03-15 · every weekly · from: main\n- [ ] 2026-03-05 · Write the monthly")
        sw = ls.Sweep(ls.parse_loops(text), TODAY)
        self.check([{"do": "end_duplicate", "id": N["weigh"], "keep": N["weigh"] + 1}], sw=sw)
        self.assertEqual(sw.refused, [])
        self.assertTrue(sw.changes[0].final)

    def test_a_merge_keeps_every_item_and_the_earliest_nudge(self):
        sw = self.check([{"do": "merge", "ids": [N["cafe"], N["gallery"]],
                          "text": "Place verdicts: Corner Cafe brunch, River Gallery show", "reason": "two verdicts"}])
        self.assertEqual(sw.refused, [])
        *closes, add = sw.changes
        self.assertEqual((add.verb, add.nudge, add.src, add.section), ("add", dt.date(2026, 3, 11), "src-a src-b", "Errands"))
        self.assertEqual([c.loop.n for c in closes], [N["cafe"], N["gallery"]])
        self.assertIn("M1", sw.merged)
        dropped = self.check([{"do": "merge", "ids": [N["cafe"], N["gallery"]], "text": "Place verdicts: Corner Cafe"}])
        self.assertIn("drops loop", dropped.refused[0][1])
        dotted = self.check([{"do": "merge", "ids": [N["cafe"], N["gallery"]], "text": "Cafe · Gallery"}])
        self.assertIn("grammar", dotted.refused[0][1])
        recurring = self.check([{"do": "merge", "ids": [N["weigh"], N["hat"]], "text": "weigh-in and winter hat"}])
        self.assertIn("recurs", recurring.refused[0][1])

    def test_an_overdue_merge_is_due_today_not_later(self):
        sw = self.check([{"do": "merge", "ids": [N["widget"], N["cafe"]], "text": "Widget pickup, Corner Cafe verdict"}])
        self.assertEqual(sw.changes[-1].nudge, TODAY)

    def test_a_redate_moves_only_an_idle_to_do_once(self):
        ok = self.check([{"do": "redate", "id": N["widget"], "nudge": "2026-03-14", "reason": "still wanted"}])
        self.assertEqual(ok.refused, [])
        self.assertEqual(ok.changes[0].nudge, dt.date(2026, 3, 14))
        money = self.check([{"do": "redate", "id": N["gym"], "nudge": "2026-03-14"}])
        self.assertIn("money", money.refused[0][1])
        hat = self.check([{"do": "redate", "id": N["hat"], "nudge": "2026-03-14"}])
        self.assertIn("purchase", hat.refused[0][1], "a purchase is guarded, so it stays in Do")
        fresh = self.check([{"do": "redate", "id": N["finances"], "nudge": "2026-03-14"}])
        self.assertIn("overdue", fresh.refused[0][1])
        far = self.check([{"do": "redate", "id": N["widget"], "nudge": "2026-04-30"}])
        self.assertIn("within", far.refused[0][1])
        report = "## Run 2026-03-05 08:20\n\n- Re-dated 2026-02-28 → 2026-03-02: Pick up the widget from Widget Shop\n"
        again = self.check([{"do": "redate", "id": N["widget"], "nudge": "2026-03-14"}], report=report)
        self.assertIn("once", again.refused[0][1])

    def test_owned_loops_take_only_a_flag(self):
        for name in ("needs", "bills", "others"):
            sw = self.check([{"do": "redate", "id": N[name], "nudge": "2026-03-14"},
                             {"do": "flag", "id": N[name], "note": "look at this"}])
            self.assertEqual(sw.changes, [], name)
            self.assertEqual(len(sw.flags), 1, name)

    def test_a_loop_takes_one_action_and_unknown_shapes_are_refused(self):
        sw = self.check([{"do": "redate", "id": N["widget"], "nudge": "2026-03-14"},
                         {"do": "merge", "ids": [N["widget"], N["cafe"]], "text": "Widget pickup, Corner Cafe"},
                         {"do": "delete", "id": N["hat"]},
                         {"do": "flag", "id": N["hat"], "note": "x", "extra": 1},
                         {"do": "redate", "id": 99, "nudge": "2026-03-14"},
                         {"do": "redate", "id": True, "nudge": "2026-03-14"}])
        self.assertEqual(len(sw.changes), 1)
        self.assertEqual(len(sw.refused), 5)
        self.assertIn("another action", sw.refused[0][1])

    def test_a_kept_loop_cannot_also_be_ended(self):
        sw = self.check([{"do": "end_duplicate", "id": N["dentist_old"], "keep": N["dentist_new"]},
                         {"do": "end_duplicate", "id": N["dentist_new"], "keep": N["dentist_old"]}])
        self.assertEqual(len(sw.changes), 1)

    def test_a_plan_too_large_is_refused_whole(self):
        many = "".join(f"- [ ] 2026-03-01 · Errand {chr(97 + i % 26)}{chr(97 + i // 26)} · nudge 2026-03-01 · from: main\n"
                       for i in range(30))
        sw = ls.Sweep(ls.parse_loops("## Waiting on the operator\n" + many), TODAY)
        acts = [{"do": "redate", "id": i, "nudge": "2026-03-14"} for i in range(1, 31)]
        ls.check_plan(sw, ls.Plan(actions=acts), self.root, {}, "")
        self.assertEqual(sw.changes, [])
        self.assertIn("whole plan", sw.refused[-1][0])


class OverdueTests(unittest.TestCase):
    def test_overdue_money_and_deadline_loops_are_reminded_and_kept_open(self):
        sw = sweep()
        ls.find_overdue(sw, kinds())
        names = {x.n for x, _ in sw.overdue}
        self.assertIn(N["gym"], names)
        self.assertIn(N["etransfer"], names)
        for quiet in ("hat", "dentist_old", "neighbour", "others", "needs", "bills", "widget"):
            self.assertNotIn(N[quiet], names, quiet)

    def test_the_handoff_carries_board_words_and_no_words_for_a_loop_kept_off(self):
        sw = sweep()
        ls.find_overdue(sw, kinds())
        items = ls.handoff_items(sw, {N["gym"]: "Pay the gym fee"})
        by_text = {x["text"]: x for x in items}
        gym = by_text["Pay the gym fee of $40 by 03-08"]
        self.assertEqual(gym, {"text": "Pay the gym fee of $40 by 03-08", "show": "Pay the gym fee", "due": "2026-03-07",
                               "days": 3, "why": ["money", "deadline"]})
        self.assertIsNone(by_text["Claim the e-transfer from the club, $12.00"]["show"])
        no_plan = {x["text"]: x for x in ls.handoff_items(sw, None)}
        self.assertEqual(no_plan["Pay the gym fee of $40 by 03-08"]["show"], "Pay the gym fee of $40 by 03-08")

    def test_a_loop_closing_today_is_not_reminded(self):
        sw = sweep()
        ls.watch_closes(sw, ls.read_watch(WATCH, TODAY), {}, [])
        ls.find_overdue(sw, {})
        self.assertNotIn(N["etransfer"], {x.n for x, _ in sw.overdue})

    def test_the_sweep_sends_nothing_itself_and_writes_todays_handoff(self):
        sw = sweep()
        ls.find_overdue(sw, {})
        with tempfile.TemporaryDirectory() as d, mock.patch("notify.main") as notify_main:
            ls.handoff(sw, Path(d), dry_run=False, lines=None)
            data = json.loads((Path(d) / ls.HANDOFF).read_text(encoding="utf-8"))
        notify_main.assert_not_called()
        self.assertEqual(data["date"], "2026-03-10")
        self.assertEqual(len(data["items"]), len(sw.overdue))
        self.assertIn("handed", sw.pushed)


class ModelTests(unittest.TestCase):
    def stream(self, *events):
        return [json.dumps(e) for e in events]

    def test_a_clean_run_gives_its_reply_and_cost(self):
        res = ls.parse_run(self.stream({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []},
                                       {"type": "result", "subtype": "success", "result": "{}", "total_cost_usd": 0.07}))
        self.assertEqual((res.text, res.cost), ("{}", 0.07))

    def test_a_run_that_saw_or_reached_for_a_tool_is_discarded(self):
        for events in (
            [{"type": "system", "subtype": "init", "tools": ["Bash"], "mcp_servers": []}],
            [{"type": "system", "subtype": "init", "tools": [], "mcp_servers": [{"name": "claude.ai Gmail"}]}],
            [{"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Bash"}]}}],
            [{"type": "result", "subtype": "success", "result": "{}", "permission_denials": [{"tool_name": "Bash"}]}],
        ):
            with self.assertRaises(ls.PlanError):
                ls.parse_run(self.stream(*events, {"type": "result", "subtype": "success", "result": "{}"}))

    def test_a_budget_stop_or_no_result_is_no_plan(self):
        with self.assertRaises(ls.PlanError):
            ls.parse_run(self.stream({"type": "result", "subtype": "error_max_budget_usd", "is_error": True}))
        with self.assertRaises(ls.PlanError):
            ls.parse_run([])

    def test_the_plan_parses_strictly(self):
        plan = ls.parse_plan('```json\n{"loops": [{"id": 1, "kind": "personal", "event_date": "2026-03-08", '
                             '"stakes": false}, {"id": 2, "kind": "weird"}], "actions": [], "board": []}\n```')
        self.assertEqual(plan.kinds[1], ls.Kind("personal", dt.date(2026, 3, 8), False))
        self.assertEqual(plan.kinds[2].kind, "other")
        for bad in ("not json", "[]", '{"loops": []}'):
            with self.assertRaises(ls.PlanError):
                ls.parse_plan(bad)

    def test_the_call_is_locked_down(self):
        seen = {}

        def fake_run(args, **kw):
            seen.update(args=args, kw=kw)
            return mock.Mock(returncode=0, stdout="", stderr="")
        with tempfile.TemporaryDirectory() as d, \
                mock.patch.object(ls, "RUN_DIR", Path(d) / "run"), \
                mock.patch.object(ls.shutil, "which", return_value="claude"), \
                mock.patch.object(ls.subprocess, "run", side_effect=fake_run):
            ls.claude("the prompt", ls.MODEL)
        args, kw = seen["args"], seen["kw"]
        self.assertEqual(kw["input"], "the prompt", "the prompt goes on stdin, not the command line")
        self.assertNotIn("the prompt", args)
        self.assertEqual(args[args.index("--tools") + 1], "")
        self.assertIn("--strict-mcp-config", args)
        self.assertNotIn("--allowedTools", args)
        self.assertNotIn("--mcp-config", args)
        self.assertEqual(args[args.index("--permission-mode") + 1], "dontAsk")
        self.assertEqual(args[args.index("--setting-sources") + 1], "local")
        self.assertEqual(args[args.index("--max-turns") + 1], "1")
        self.assertEqual(args[args.index("--max-budget-usd") + 1], f"{ls.BUDGET_USD:.2f}")
        self.assertEqual(args[args.index("--model") + 1], "claude-sonnet-5-5")
        self.assertIn("--system-prompt", args)
        self.assertNotEqual(Path(kw["cwd"]), ROOT)
        denied = args[args.index("--disallowedTools") + 1].split(",")
        self.assertIn("mcp__claude_ai_Gmail__search_threads", denied)
        self.assertIn("mcp__claude_ai_Gmail__send_message", denied)

    def test_the_prompt_holds_the_loops_and_no_email(self):
        p = ls.prompt_for(loops(), {"workspace/life/x.md": "- 2026-03-08 · a line"}, TODAY)
        self.assertIn("Today is 2026-03-10 (Tuesday)", p)
        self.assertIn('"text": "Pay the gym fee of $40 by 03-08"', p)
        self.assertIn('"overdue_days": 3', p)
        self.assertIn('<file path="workspace/life/x.md">', p)
        self.assertIn("A passed date is never evidence", p)

    def test_evidence_files_must_be_inside_workspace(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "workspace" / "life" / "systems").mkdir(parents=True)
            (root / "workspace" / "life" / "log.md").write_text("- 2026-03-08 · entry\n", encoding="utf-8")
            (root / "secret.txt").write_text("outside\n", encoding="utf-8")
            (root / ls.EVIDENCE_LIST).write_text("# mine\nworkspace/life/log.md\n../secret.txt\nsecret.txt\n"
                                                 "workspace/missing.md\n", encoding="utf-8")
            self.assertEqual(list(ls.evidence_files(root)), ["workspace/life/log.md"])


class BoardTests(unittest.TestCase):
    def test_the_board_shows_open_loops_with_overdue_first_and_keeps_private_terms_off(self):
        sw = sweep()
        ls.find_overdue(sw, kinds())
        plan = ls.Plan(board=[
            {"id": N["gym"], "section": "Coming up", "line": "Pay the gym fee"},
            {"id": N["hat"], "section": "When you have time", "line": "Winter hat and gloves"},
            {"id": N["others"], "section": "Owed to you", "line": "Sam owes $15"},
            {"id": N["needs"], "section": "Coming up", "line": "Renew or move"},
            {"id": N["dentist_new"], "section": "Coming up", "line": "Dentist cleaning"},
            {"id": N["widget"], "section": "Nowhere", "line": "bad section"},
            {"id": 99, "section": "Coming up", "line": "no such loop"},
        ])
        board = ls.render_board(sw, plan, loops(), ["dentist"])
        self.assertTrue(board.startswith("**Open: what's waiting**"))
        self.assertLess(board.index("**Overdue, not confirmed**"), board.index("**Owed to you**"))
        self.assertIn("- Pay the gym fee (due 03-07, 3d over)", board)
        self.assertIn("- Winter hat and gloves", board)
        self.assertIn("- Sam owes $15", board)
        for gone in ("Renew or move", "Dentist", "bad section", "no such loop"):
            self.assertNotIn(gone, board)
        self.assertIn("- 1 need you now in AgentFrame.", board)

    def test_a_closed_loop_leaves_the_board(self):
        sw = sweep()
        plan = ls.Plan(board=[{"id": N["hat"], "section": "When you have time", "line": "Winter hat"}])
        after = [x for x in loops() if x.n != N["hat"]]
        self.assertNotIn("Winter hat", ls.render_board(sw, plan, after, []))


class RunTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        life = self.root / "workspace" / "life"
        (life / "systems").mkdir(parents=True)
        (life / "open.md").write_text(OPEN, encoding="utf-8")
        (self.root / "workspace" / "life" / "systems" / "loop-watch-report.md").write_text(WATCH, encoding="utf-8")
        self.prompts = []
        self.plan = {"loops": [{"id": N["neighbour"], "kind": "personal", "event_date": "2026-03-08", "stakes": False}],
                     "actions": [{"do": "close_day_passed", "id": N["neighbour"], "reason": "a chat"},
                                 {"do": "redate", "id": N["widget"], "nudge": "2026-03-14", "reason": "still wanted"},
                                 {"do": "close_day_passed", "id": N["gym"]}],
                     "board": [{"id": N["hat"], "section": "When you have time", "line": "Winter hat"}]}

    def tearDown(self):
        self.tmp.cleanup()

    def claude(self, prompt, model):
        self.prompts.append(prompt)
        return [json.dumps({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []}),
                json.dumps({"type": "result", "subtype": "success", "result": json.dumps(self.plan), "total_cost_usd": 0.11})]

    def files(self):
        return sorted(str(p.relative_to(self.root)) for p in self.root.rglob("*") if p.is_file())

    def run_main(self, *argv, run_af=None):
        run_af = run_af or fake_af_for(None)[0]
        out = io.StringIO()
        with contextlib.redirect_stdout(out), mock.patch("notify.main", return_value=0) as push, \
                mock.patch("discord_board.main", return_value=0) as board:
            code = ls.main(["--vault", str(self.root), *argv], now=NOW, run_claude=self.claude, run_af=run_af)
        return code, out.getvalue(), push, board

    def test_a_run_applies_through_af_and_appends_the_report(self):
        run_af, calls = fake_af_for(None)
        code, out, push, board = self.run_main(run_af=run_af)
        self.assertEqual(code, 0, out)
        writes = [c for c in calls if c[:3] != ["open", "due", "--json"]]
        self.assertIn(["open", "close", "Claim the e-transfer from the club, $12.00", "--outcome",
                       "Sweep 2026-03-10: the loop watch found an email of 2026-03-09 from payments.interac.ca "
                       "(Interac) showing it done"], writes)
        self.assertIn(["open", "close", "Ask the neighbour about the parcel on 03-08", "--outcome",
                       "Sweep 2026-03-10: day passed (03-08), unconfirmed"], writes)
        self.assertIn(["open", "redate", "Pick up the widget from Widget Shop", "--nudge", "2026-03-14"], writes)
        self.assertEqual(len(writes), 3)
        for c in writes:
            if c[1] == "close":
                self.assertTrue(c[c.index("--outcome") + 1].startswith("Sweep 2026-03-10: "))
        report = (self.root / ls.REPORT).read_text(encoding="utf-8")
        self.assertIn("## Run 2026-03-10 08:20", report)
        self.assertIn("- Re-dated 2026-03-05 → 2026-03-14: Pick up the widget from Widget Shop", report)
        self.assertIn("- Refused close_day_passed #2: #2 does not start with", report)
        self.assertIn("- Overdue, reminded: Pay the gym fee", report)
        self.assertRegex(report, r"(?m)^Loops 14 · closed 2 · new merged loops 0 · re-dated 1 · overdue \d+ · flags 0 · "
                                 r"refused 1 · failures 0 · cost \$0\.11$")
        push.assert_not_called()
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertEqual(handed["date"], "2026-03-10")
        self.assertIn("Pay the gym fee of $40 by 03-08", [x["text"] for x in handed["items"]])
        self.assertIn("- Phone: handed", report)
        board.assert_not_called()
        self.assertIn("not posted: write", report, "no private terms file, so the board waits")

    def test_the_model_never_sees_email(self):
        self.run_main("--dry-run")
        self.assertEqual(len(self.prompts), 1)
        for email_text in ("payments.interac.ca", "deposited", "Deposit complete", "Loop watch"):
            self.assertNotIn(email_text, self.prompts[0])

    def test_a_dry_run_changes_sends_and_writes_nothing(self):
        (self.root / ls.BOARD_PRIVATE).parent.mkdir(parents=True)
        (self.root / ls.BOARD_PRIVATE).write_text("# terms kept off Discord\n", encoding="utf-8")
        before = {p: (self.root / p).read_bytes() for p in self.files()}
        run_af, calls = fake_af_for(None)
        code, out, push, board = self.run_main("--dry-run", run_af=run_af)
        self.assertEqual(code, 0)
        self.assertEqual({p: (self.root / p).read_bytes() for p in self.files()}, before)
        self.assertEqual([c for c in calls if c[:3] != ["open", "due", "--json"]], [])
        push.assert_not_called()
        self.assertIn("Closed: Ask the neighbour", out)
        self.assertIn("Dry run.", out)
        self.assertIn(f"#{N['gym']} money urgent (money, purchase, deadline)", out)
        self.assertIn(f"#{N['hat']} money (purchase)", out)
        self.assertIn("What the 08:30 push would lead with:", out)
        self.assertIn("- 1 more kept off Discord", out, "the model wrote no board line for it")

    def test_a_later_morning_can_be_previewed_only_as_a_dry_run(self):
        code, out, _, _ = self.run_main("--dry-run", "--today", "2026-03-12")
        self.assertIn("Loop sweep plan for 2026-03-12", out)
        self.assertIn("Today is 2026-03-12", self.prompts[-1])
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            self.run_main("--today", "2026-03-12")
        self.assertFalse((self.root / ls.REPORT).exists())

    def test_the_board_is_written_and_posted_once_the_private_terms_file_exists(self):
        (self.root / ls.BOARD_PRIVATE).parent.mkdir(parents=True)
        (self.root / ls.BOARD_PRIVATE).write_text("# terms kept off Discord\n", encoding="utf-8")
        code, out, push, board = self.run_main()
        self.assertEqual(code, 0, out)
        board.assert_called_once()
        self.assertEqual(board.call_args.args[0][:2], ["board", "open"])
        self.assertIn("Winter hat", (self.root / ls.BOARD).read_text(encoding="utf-8"))

    def test_numbering_drift_applies_nothing(self):
        calls = []

        def run_af(root, args):
            calls.append(args)
            return 0, json.dumps([{"n": 1, "text": "something else"}])
        code, out, _, _ = self.run_main(run_af=run_af)
        self.assertEqual(code, 1)
        self.assertEqual([c for c in calls if c[:3] != ["open", "due", "--json"]], [])
        self.assertIn("nothing applied", (self.root / ls.REPORT).read_text(encoding="utf-8"))

    def test_af_failing_partway_still_writes_the_report_and_log_and_exits_1(self):
        fake, _ = fake_af_for(None)

        def run_af(root, args):
            if args[:2] == ["open", "redate"]:
                raise FileNotFoundError("af is not on PATH")
            return fake(root, args)
        code, out, _, _ = self.run_main(run_af=run_af)
        self.assertEqual(code, 1)
        report = (self.root / ls.REPORT).read_text(encoding="utf-8")
        self.assertIn("- Closed: Ask the neighbour", report)
        self.assertNotIn("- Re-dated", report)
        self.assertIn("- Failed: stopped after 2 of 3 changes: FileNotFoundError: af is not on PATH", report)
        self.assertIn("closed 2 · new merged loops 0 · re-dated 0", (self.root / ls.LOG).read_text(encoding="utf-8"))

    def test_a_crash_after_applying_still_writes_the_report(self):
        with mock.patch.object(ls, "handoff", side_effect=PermissionError("locked")), \
                contextlib.redirect_stderr(io.StringIO()):
            code, out, _, _ = self.run_main()
        self.assertEqual(code, 1)
        report = (self.root / ls.REPORT).read_text(encoding="utf-8")
        self.assertIn("- Closed: Ask the neighbour", report)
        self.assertIn("- Failed: the run stopped: PermissionError: locked", report)
        self.assertFalse((self.root / ls.RUNNING).exists())

    def test_an_unwritable_report_still_logs_and_prints_the_run(self):
        (self.root / ls.REPORT).mkdir(parents=True)  # a directory where the report goes
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code, out, _, _ = self.run_main()
        self.assertEqual(code, 1)
        self.assertIn("could not write the report", err.getvalue())
        self.assertIn("- Closed: Ask the neighbour", out)
        self.assertIn("closed 2", (self.root / ls.LOG).read_text(encoding="utf-8"))

    def test_a_live_run_marks_itself_running_for_the_push_and_a_dry_run_does_not(self):
        seen = []
        real = self.claude

        def claude(prompt, model):
            seen.append((self.root / ls.RUNNING).exists())
            return real(prompt, model)
        with contextlib.redirect_stdout(io.StringIO()), mock.patch("discord_board.main", return_value=0):
            ls.main(["--vault", str(self.root)], now=NOW, run_claude=claude, run_af=fake_af_for(None)[0])
            ls.main(["--vault", str(self.root), "--dry-run"], now=NOW, run_claude=claude, run_af=fake_af_for(None)[0])
        self.assertEqual(seen, [True, False])
        self.assertFalse((self.root / ls.RUNNING).exists())

    def test_no_plan_still_closes_on_watch_evidence_and_reminds(self):
        self.plan = "not a plan"
        run_af, calls = fake_af_for(None)
        with mock.patch.object(self, "claude", side_effect=lambda p, m: [json.dumps(
                {"type": "result", "subtype": "success", "result": "not json"})]):
            code, out, push, board = self.run_main(run_af=run_af)
        self.assertEqual(code, 1)
        writes = [c for c in calls if c[:3] != ["open", "due", "--json"]]
        self.assertEqual(len(writes), 1)
        self.assertEqual(writes[0][1], "close")
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertIn("Pay the gym fee of $40 by 03-08", [x["show"] for x in handed["items"]],
                      "with no plan the loop's own text goes, for the push to filter")
        report = (self.root / ls.REPORT).read_text(encoding="utf-8")
        self.assertIn("- Failed: no plan: the model's reply is not JSON", report)
        self.assertIn("- Board: not changed: no plan", report)

    def test_the_report_keeps_the_last_runs(self):
        old = ls.HEADER + "\n" + "".join(f"## Run 2026-01-{i:02d} 08:20\n\n- Nothing to change.\n\nLoops 1\n\n"
                                         for i in range(1, 31)) * 3
        new = ls.append_report(old, "## Run 2026-03-10 08:20\n\n- Nothing to change.\n")
        self.assertEqual(new.count("## Run "), ls.KEEP_RUNS)
        self.assertTrue(new.rstrip().endswith("- Nothing to change."))
        self.assertTrue(new.startswith("# Loop sweep"))


class ScheduleTests(unittest.TestCase):
    def test_the_task_runs_after_the_loop_watch_and_before_the_push(self):
        ps1 = (ROOT / "system" / "tools" / "register-loop-sweep.ps1").read_text(encoding="utf-8")
        at = re.search(r"-Daily -At '(\d\d:\d\d)'", ps1).group(1)
        watch = re.search(r"-Daily -At '(\d\d:\d\d)'",
                          (ROOT / "system" / "tools" / "register-loop-watch.ps1").read_text(encoding="utf-8")).group(1)
        push = re.search(r"-Daily -At '(\d\d:\d\d)'",
                         (ROOT / "system" / "tools" / "register-daily-reminders.ps1").read_text(encoding="utf-8")).group(1)
        self.assertLess(watch, at)
        self.assertLess(at, push)
        self.assertIn("$taskName = 'AgentFrame loop sweep'", ps1)
        self.assertIn("loop-sweep.ps1", ps1)
        timeout = int(re.search(r"-ExecutionTimeLimit \(New-TimeSpan -Minutes (\d+)\)", ps1).group(1))
        self.assertGreater(timeout * 60, ls.TIMEOUT_S + 120)

    def test_the_laptop_timers_carry_it(self):
        timers = (ROOT / "system" / "tools" / "laptop" / "timers.ps1").read_text(encoding="utf-8")
        self.assertIn("@{ Script = 'register-loop-sweep.ps1'; Task = 'AgentFrame loop sweep' }", timers)

    def test_the_tracked_files_hold_no_address_beyond_generic_services(self):
        for name in ("loop_sweep.py", "loop-sweep.ps1", "register-loop-sweep.ps1"):
            text = (ROOT / "system" / "tools" / name).read_text(encoding="utf-8")
            self.assertIsNone(re.search(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+\.[A-Za-z]{2,}", text), name)

    def test_the_push_watches_the_sweeps_marker(self):
        import push_due_loops
        self.assertEqual(push_due_loops.SWEEP_RUNNING, ls.RUNNING)

    def test_the_entry_script_says_the_sweep_hands_off_and_sends_nothing(self):
        head = (ROOT / "system" / "tools" / "loop-sweep.ps1").read_text(encoding="utf-8").split("param(")[0]
        self.assertIn("hands overdue money and deadline loops", head)
        self.assertNotIn("to his phone", head)


def one(text, section="Errands"):
    """A sweep over a single loop under the operator's errands, due 2026-03-01."""
    open_text = f"## Waiting on the operator\n\n### {section}\n- [ ] 2026-02-20 · {text} · nudge 2026-03-01 · from: main\n"
    return ls.Sweep(ls.parse_loops(open_text), TODAY)


# Money, deadline and appointment loops the old word lists missed (the reviewer's cases).
MISSED = ["Send rent to landlord Mar 1", "Transfer 500 to Sam on Mar 3", "Submit tax return Mar 3",
          "Hand in lease renewal form by Mar 3", "Dentist appointment Mar 3"]


class SenderTests(unittest.TestCase):
    """Finding 1: a sender fits by its registered domain only, and a loop with money or a deadline
    settles only from a fixed sender."""

    def test_the_registered_domain_is_the_last_two_labels_or_three_under_a_two_part_suffix(self):
        self.assertEqual(ls.registrable("mail.widget.example"), "widget.example")
        self.assertEqual(ls.registrable("sam.evil.xyz"), "evil.xyz")
        self.assertEqual(ls.registrable("orders.shop.co.uk"), "shop.co.uk")
        self.assertEqual(ls.registrable("widget.example"), "widget.example")

    def test_a_word_of_the_loop_in_any_label_does_not_fit(self):
        loop = one("Pay Sam back for the concert tickets").loops[0]
        for address in ("noreply@sam.evil.xyz", "a@concert.ru", "x@back.io", "y@pay.attacker.net",
                        "z@tickets.concert.example"):
            for kind in ("money", "booking", "other"):
                self.assertIsNone(ls.sender_fits(loop, kind, address, []), (address, kind))

    def test_a_money_or_deadline_loop_never_settles_from_the_sender_it_names(self):
        for text in ("Pay the Widget invoice", "Renew the widget licence", "Return the widget by Mar 12",
                     "Settle up with Widget, 40 owed"):
            loop = one(text).loops[0]
            self.assertIsNone(ls.sender_fits(loop, ls.kind_of(loop, None), "billing@widget.example", []), text)
        hat = one("Pick up the widget from Widget Shop").loops[0]
        self.assertIsNone(ls.sender_fits(hat, "other", "orders@widget.example", [], ls.Kind("other", stakes=True)),
                          "the model's say that someone waits makes it urgent too")

    def test_fixed_senders_still_settle_urgent_loops(self):
        loop = one("Pay the Fernhollow card, $40 by Mar 8").loops[0]
        self.assertEqual(ls.sender_fits(loop, "money", "statements@alerts.fernhollow.example", ["fernhollow.example"]),
                         "his card issuer")
        tickets = one("Book the tickets before it fills").loops[0]
        self.assertEqual(ls.sender_fits(tickets, "booking", "noreply@eventbrite.com", []), "a ticketing platform")

    def test_an_ordinary_loop_settles_from_its_named_senders_registered_name_only(self):
        loop = one("Pick up the widget from Widget Shop").loops[0]
        self.assertEqual(ls.sender_fits(loop, "other", "orders@mail.widget.example", []), "the sender the loop names")
        self.assertIsNone(ls.sender_fits(loop, "other", "orders@widget.evil.example", []))
        self.assertIsNone(ls.sender_fits(loop, "other", "orders@shop.example", []), "too short or generic a name")


class GuardTests(unittest.TestCase):
    """Finding 2: a passed date closes only a plainly social loop, and the stakes words cover rent,
    transfers, tax, forms, renewals, appointments, bare amounts and "by <month> <day>"."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def act(self, text, action, kind=None):
        sw = one(text)
        ls.check_plan(sw, ls.Plan(kinds={1: kind or ls.Kind("personal", dt.date(2026, 3, 3), False)},
                                  actions=[dict(action, id=1)]), self.root, {}, "")
        return sw

    def test_the_new_stakes_words(self):
        self.assertIn("money", ls.heuristic_stakes("Send rent to landlord Mar 1"))
        self.assertIn("money", ls.heuristic_stakes("Transfer 500 to Sam on Mar 3"))
        self.assertEqual(ls.heuristic_stakes("Sam 500"), ["amount"])
        self.assertIn("money", ls.heuristic_stakes("Send $40 to Alex"))
        self.assertIn("money", ls.heuristic_stakes("Give Jo 40 dollars"))
        self.assertIn("money", ls.heuristic_stakes("Submit tax return Mar 3"))
        self.assertIn("deadline", ls.heuristic_stakes("Submit tax return Mar 3"))
        self.assertIn("deadline", ls.heuristic_stakes("Hand in lease renewal form by Mar 3"))
        self.assertIn("deadline", ls.heuristic_stakes("Reply to the landlord by March 3rd"))
        self.assertIn("deadline", ls.heuristic_stakes("Reply to the landlord by the 15th"))
        self.assertIn("booking", ls.heuristic_stakes("Dentist appointment Mar 3"))
        self.assertEqual(ls.heuristic_stakes("Buy a winter hat, up to $50"), ["purchase"], "a price is a purchase")
        for plain in ("Coffee with Alex on Mar 3", "Call Mom on 3/8 at 7:30", "Text Jo about the 8th", "Taxi home",
                      "Decide 1 thing", "Plan the 2027 trip", "Write months/2026-03.md"):
            self.assertEqual(ls.heuristic_stakes(plain), [], plain)

    def test_a_bare_amount_guards_but_does_not_remind_daily(self):
        for text in ("Sam 500", "Set up the laptop on Windows 10", "Account cleanup: 30 emails sent"):
            loop = one(text).loops[0]
            self.assertIn("amount", ls.stakes_of(loop, None), text)
            self.assertEqual(ls.urgency_of(loop, None), [], text)
        sw = self.act("Sam 500", {"do": "redate", "nudge": "2026-03-14"}, ls.Kind("other"))
        self.assertIn("amount", sw.refused[0][1])
        self.assertIsNone(ls.sender_fits(one("Widget 500").loops[0], "other", "a@widget.example", []))
        for text in ("Follow up with Jo [src-2026-11-01-02]", "Collect item #275219 from the desk", "Read pages 3/12"):
            self.assertEqual(ls.heuristic_stakes(text), [], text)

    def test_a_passed_date_closes_none_of_the_missed_money_loops(self):
        for text in MISSED:
            sw = self.act(text, {"do": "close_day_passed"})
            self.assertEqual(sw.changes, [], text)
            self.assertTrue(sw.refused, text)

    def test_a_passed_date_closes_only_a_plainly_social_loop(self):
        sw = self.act("Clean out the garage on Mar 3", {"do": "close_day_passed"})
        self.assertEqual(sw.changes, [])
        self.assertIn("does not start with", sw.refused[0][1])
        for text in ("Visit Alex on Mar 3", "Call Jo on 03-03", "Meet the new neighbour Mar 3"):
            sw = self.act(text, {"do": "close_day_passed"})
            self.assertEqual(len(sw.changes), 1, text)

    def test_none_of_the_missed_loops_is_re_dated(self):
        for text in MISSED:
            sw = self.act(text, {"do": "redate", "nudge": "2026-03-14"}, ls.Kind("other"))
            self.assertEqual(sw.changes, [], text)
            self.assertTrue(sw.refused, text)
        self.assertIn("names its own day", self.act("Visit the gallery on Mar 3", {"do": "redate", "nudge": "2026-03-14"},
                                                    ls.Kind("other")).refused[0][1])
        booking = self.act("Book the hair appointment", {"do": "redate", "nudge": "2026-03-14"}, ls.Kind("booking"))
        self.assertEqual(booking.changes, [], "a booking is guarded, so it stays in Do")
        ok = self.act("Tidy the garage shelves", {"do": "redate", "nudge": "2026-03-14"}, ls.Kind("other"))
        self.assertEqual(len(ok.changes), 1, "a plain to-do with no stake moves")

    def test_a_day_is_named_in_his_usual_forms(self):
        for text in ("on 2026-03-08", "Sun 03-08", "on 3/8", "Mar 8", "March 8th", "8 Mar", "by the 8th"):
            self.assertTrue(ls.names_a_day(text), text)
        for text in ("months/2026-03.md", "Decide 10 things", "a market 5 minutes away", "Plan the 2027 trip",
                     "Follow up with Jo [src-2026-11-01-02]", "an itch for 2-3 years"):
            self.assertFalse(ls.names_a_day(text), text)


class EvidenceTests(unittest.TestCase):
    """Finding 3: vault and file evidence share two words with the loop, and never settle an urgent loop."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.dir = self.root / "workspace" / "life" / "systems"
        self.dir.mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def act(self, text, action, files=None):
        sw = one(text)
        ls.check_plan(sw, ls.Plan(actions=[dict(action, id=1)]), self.root, files or {}, "")
        return sw

    def test_an_unrelated_dated_line_settles_nothing(self):
        log = self.dir / "journal.md"
        log.write_text("- 2026-03-05 · walked the dog around the park twice today\n", encoding="utf-8")
        rel = "workspace/life/systems/journal.md"
        sw = self.act("Return the library novels", {"do": "close_vault", "file": rel,
                                                   "quote": "walked the dog around the park"}, {rel: log.read_text(encoding="utf-8")})
        self.assertEqual(sw.changes, [])
        self.assertIn("shares two words", sw.refused[0][1])
        sw = self.act("Walk the dog around the park", {"do": "close_vault", "file": rel,
                                                       "quote": "walked the dog around the park"}, {rel: log.read_text(encoding="utf-8")})
        self.assertEqual(len(sw.changes), 1)

    def test_an_urgent_loop_is_never_closed_by_a_file(self):
        log = self.dir / "money-log.md"
        log.write_text("- 2026-03-05 · rent paid to the landlord for March\n", encoding="utf-8")
        rel = "workspace/life/systems/money-log.md"
        sw = self.act("Send rent to the landlord", {"do": "close_vault", "file": rel,
                                                    "quote": "rent paid to the landlord"}, {rel: log.read_text(encoding="utf-8")})
        self.assertEqual(sw.changes, [])
        self.assertIn("reminded instead", sw.refused[0][1])
        form = self.dir / "lease-renewal-form.md"
        form.write_text("# Lease renewal form\n", encoding="utf-8")
        sw = self.act("Write up the lease renewal form by Mar 12",
                      {"do": "close_file", "file": "workspace/life/systems/lease-renewal-form.md"})
        self.assertEqual(sw.changes, [])
        self.assertIn("reminded instead", sw.refused[0][1])

    def test_a_file_must_share_two_words_not_one_date(self):
        (self.dir / "notes").mkdir()
        (self.dir / "notes" / "2026-03.md").write_text("# notes\n", encoding="utf-8")
        (self.dir / "packing-list.md").write_text("# list\n", encoding="utf-8")
        sw = self.act("Write the 2026-03 trip review", {"do": "close_file", "file": "workspace/life/systems/notes/2026-03.md"})
        self.assertIn("fewer than two words", sw.refused[0][1])
        sw = self.act("Write the packing list for the trip", {"do": "close_file",
                                                             "file": "workspace/life/systems/packing-list.md"})
        self.assertEqual(len(sw.changes), 1)


class BoardMaskTests(unittest.TestCase):
    """Finding 5: the board keeps bill amounts and masks account-like numbers."""

    def test_amounts_stay_and_long_numbers_are_masked(self):
        sw = sweep()
        plan = ls.Plan(board=[{"id": N["hat"], "section": "When you have time",
                               "line": "Winter hat, $45.00, order 99887766"}])
        board = ls.render_board(sw, plan, loops(), [])
        self.assertIn("- Winter hat, $45.00, order ####", board)


class ApplyTests(unittest.TestCase):
    """Finding 6: a merge closes its originals first and adds last; failures leave nothing lost."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / ls.OPEN).parent.mkdir(parents=True)
        (self.root / ls.OPEN).write_text(OPEN, encoding="utf-8")

    def tearDown(self):
        self.tmp.cleanup()

    def merged(self):
        sw = sweep()
        ls.check_plan(sw, ls.Plan(actions=[{"do": "merge", "ids": [N["cafe"], N["gallery"]],
                                            "text": "Place verdicts: Corner Cafe brunch, River Gallery show"}]),
                      self.root, {}, "")
        return sw

    def run_af(self, fail=lambda args: False):
        calls = []

        def run(root, args):
            calls.append(args)
            return (1, "refused") if fail(args) else (0, "ok")
        return run, calls

    def test_the_originals_close_before_the_merged_loop_is_added(self):
        sw = self.merged()
        self.assertEqual([c.verb for c in sw.changes], ["close", "close", "add"])
        run, calls = self.run_af()
        ls.apply(sw, self.root, run)
        self.assertEqual([c[1] for c in calls], ["close", "close", "add"])
        self.assertTrue(all(c.applied for c in sw.changes))
        self.assertEqual(sw.failures, [])

    def test_no_merged_loop_when_no_original_closed(self):
        sw = self.merged()
        run, calls = self.run_af(lambda a: a[1] == "close")
        ls.apply(sw, self.root, run)
        self.assertEqual([c[1] for c in calls], ["close", "close"])
        self.assertIn("none of its originals closed", sw.failures[-1])

    def test_a_failed_add_puts_the_closed_originals_back(self):
        sw = self.merged()
        run, calls = self.run_af(lambda a: a[1] == "add" and a[2].startswith("Place verdicts"))
        ls.apply(sw, self.root, run)
        adds = [c for c in calls if c[1] == "add"]
        self.assertEqual([c[2] for c in adds], ["Place verdicts: Corner Cafe brunch, River Gallery show",
                                                "verdict: Corner Cafe, brunch 03-01", "verdict: River Gallery, show 03-02"])
        self.assertEqual(adds[1][adds[1].index("--nudge") + 1], "2026-03-12", "its own nudge, when still ahead")
        self.assertFalse(any(c.applied for c in sw.changes))
        self.assertEqual(len(sw.notes), 2)
        self.assertNotIn("Closed:", ls.render_run(sw, NOW), "the report names only what happened")

    def test_an_error_stops_the_run_and_keeps_what_was_done(self):
        sw = sweep()
        ls.check_plan(sw, ls.Plan(kinds=kinds(), actions=[
            {"do": "close_day_passed", "id": N["neighbour"]},
            {"do": "redate", "id": N["widget"], "nudge": "2026-03-14"}]), self.root, {}, "")

        def run(root, args):
            if args[1] == "redate":
                raise FileNotFoundError("af is not on PATH")
            return 0, "ok"
        ls.apply(sw, self.root, run)
        self.assertEqual([c.applied for c in sw.changes], [True, False])
        self.assertIn("stopped after 1 of 2 changes: FileNotFoundError", sw.failures[0])
        run_text = ls.render_run(sw, NOW)
        self.assertIn("- Closed: Ask the neighbour", run_text)
        self.assertNotIn("Re-dated", run_text)
        self.assertIn("closed 1 · new merged loops 0 · re-dated 0", ls.summary(sw))


def probe_loop(text, n=1, opened="2026-11-01", nudge="2026-11-03"):
    """A loop as the second review's probes build them."""
    o, nd = dt.date.fromisoformat(opened), dt.date.fromisoformat(nudge)
    return ls.Loop(n, "Errands", "Waiting on the operator", o, text, nd, None, "main", None, None,
                   f"- [ ] {opened} · {text} · nudge {nudge} · from: main", n)


PROBE_DAY = dt.date(2026, 11, 9)


class SecondReviewTests(unittest.TestCase):
    """Each case the second review's probes found unsafe (probe.py, probe2.py)."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def plan(self, loops, actions, kinds=None, files=None):
        sw = ls.Sweep(loops, PROBE_DAY)
        ls.check_plan(sw, ls.Plan(kinds=kinds or {}, actions=actions), self.root, files or {}, "")
        return sw

    def test_a_passed_date_closes_only_a_social_start_with_zero_stakes(self):
        for text in ("Repay Sam for dinner on Nov 3", "PayPal Jo for coffee on Nov 3", "Text Sam forty bucks on Nov 3",
                     "Text Sam 5 for coffee Nov 3", "Settle up with Jo after dinner Nov 3", "Venmo Jo 25 for lunch Nov 3",
                     "Call Jo about the backpack on Nov 3", "See Jo at the dentist appointment Nov 3"):
            sw = self.plan([probe_loop(text)], [{"do": "close_day_passed", "id": 1}],
                           {1: ls.Kind("personal", dt.date(2026, 11, 3), False)})
            self.assertEqual(sw.changes, [], text)
        sw = self.plan([probe_loop("Catch up with Jo on Nov 3")], [{"do": "close_day_passed", "id": 1}],
                       {1: ls.Kind("personal", dt.date(2026, 11, 3), False)})
        self.assertEqual(len(sw.changes), 1, "a social start with zero stakes still closes")

    def test_zero_stakes_is_an_allowlist(self):
        for text in ("Repay Sam", "Prepay the hydro account", "Mail the cheque", "Zelle Mia for the cabin",
                     "Return the book", "Order the part", "Fill in the form", "Sam 5", "Lunch, ＄12", "Ticketing",
                     "Re​pay Sam", "Rеpay Sam"):
            self.assertFalse(ls.zero_stakes(text), text)
        for text in ("Ask the neighbour about the parcel on 03-08", "Visit Alex on Nov 3",
                     "Follow up with Jo [src-2026-11-01-02]", "Write the review into months/2026-03.md"):
            self.assertTrue(ls.zero_stakes(text), text)

    def test_money_without_the_old_words_is_urgent(self):
        for text in ("Repay Sam for dinner on Nov 3", "PayPal Jo for coffee on Nov 3", "Text Sam forty bucks for pizza",
                     "Venmo Jo 25 for lunch", "Mail the cheque to the landlord", "Settle up with Jo after dinner",
                     "Zelle Mia for the cabin", "Send Alex 40 CAD", "Prepay the hydro account"):
            self.assertIn("money", ls.urgency_of(probe_loop(text), None), text)
        for text in ("Talk to my parent", "Buy flowers", "Book a taxi"):
            self.assertNotIn("money", ls.urgency_of(probe_loop(text), None), text)

    def test_vault_and_file_evidence_never_close_a_guarded_loop(self):
        f = self.root / "workspace/life/notes/standing-desk.md"
        f.parent.mkdir(parents=True)
        f.write_text("- 2026-11-05 researched standing desk options online\n", encoding="utf-8")
        rel = "workspace/life/notes/standing-desk.md"
        files = {rel: f.read_text(encoding="utf-8")}
        for text in ("Buy a standing desk", "Book the standing desk delivery", "Return Sam 40 for the standing desk"):
            vault = self.plan([probe_loop(text)], [{"do": "close_vault", "id": 1, "file": rel,
                                                    "quote": "researched standing desk options online"}], files=files)
            self.assertEqual(vault.changes, [], text)
            file = self.plan([probe_loop(text)], [{"do": "close_file", "id": 1, "file": rel}])
            self.assertEqual(file.changes, [], text)
        ok = self.plan([probe_loop("Research standing desk options")],
                       [{"do": "close_vault", "id": 1, "file": rel, "quote": "researched standing desk options online"}],
                       files=files)
        self.assertEqual(len(ok.changes), 1, "a zero-stakes loop still closes on its evidence")

    def test_an_urgent_loop_never_ends_as_the_duplicate_of_a_plain_one(self):
        a = probe_loop("Pay Fernhollow Bank credit card balance", 1)
        b = probe_loop("Research Fernhollow Bank credit card rewards", 2, opened="2026-11-02")
        sw = self.plan([a, b], [{"do": "end_duplicate", "id": 1, "keep": 2}])
        self.assertEqual(sw.changes, [])
        self.assertIn("money or a deadline", sw.refused[0][1])

    def test_a_guarded_duplicate_must_be_near_identical_to_a_guarded_loop(self):
        a = probe_loop("Book the dentist cleaning", 1)
        b = probe_loop("Book the dentist cleaning and the eye exam and the haircut", 2, opened="2026-11-02")
        sw = self.plan([a, b], [{"do": "end_duplicate", "id": 1, "keep": 2}])
        self.assertIn("near-identical", sw.refused[0][1])
        c = probe_loop("Book the dentist cleaning soon", 3, opened="2026-11-02")
        sw = self.plan([a, c], [{"do": "end_duplicate", "id": 1, "keep": 3}])
        self.assertEqual(len(sw.changes), 1)
        d = probe_loop("Tidy the dentist cleaning notes", 4, opened="2026-11-02")
        sw = self.plan([a, d], [{"do": "end_duplicate", "id": 1, "keep": 4}])
        self.assertIn("zero stakes", sw.refused[0][1])

    def test_a_merge_cannot_quiet_money_or_drop_an_amount(self):
        a, b = probe_loop("Pay Sam 500 rent", 1), probe_loop("Ask Sam about the parking spot", 2)
        sw = self.plan([a, b], [{"do": "merge", "ids": [1, 2], "text": "Ask Sam about the parking spot and the 500"}])
        self.assertEqual(sw.changes, [])
        self.assertIn("money or a deadline", sw.refused[0][1])
        c, d = probe_loop("Claim the $12 from Jo", 1), probe_loop("Claim the $15 from Mia", 2)
        sw = self.plan([c, d], [{"do": "merge", "ids": [1, 2], "text": "Claim the money from Jo and Mia, $15"}])
        self.assertIn("drops 12", sw.refused[0][1])

    def test_a_card_issuer_settles_only_a_loop_that_names_its_card(self):
        cards = ["fernhollowbank.com"]
        jo = probe_loop("Pay Jo back for the Fernhollow show")
        self.assertIsNone(ls.sender_fits(jo, "money", "x@fernhollowbank.com", cards))
        card = probe_loop("Pay the Fernhollow Bank card")
        self.assertEqual(ls.sender_fits(card, "money", "alerts@FERNHOLLOWBANK.COM", cards), "his card issuer")
        by_name = probe_loop("Pay the Test Card statement")
        self.assertEqual(ls.sender_fits(by_name, "money", "a@fernhollowbank.com", {"Test Card": cards}), "his card issuer")
        for address in ("alerts@fernhollowbank.com.evil.io", "alerts@fernhollowbank-alerts.com", "alerts@xfernhollowbank.com",
                        "alerts@fernhollow.bank", "a@fernhollowbank.com>@evil.io"):
            self.assertIsNone(ls.sender_fits(card, "money", address, cards), address)

    def test_a_named_sender_never_settles_a_guarded_loop(self):
        show = probe_loop("Book tickets for the Fernhollow show")
        self.assertIsNone(ls.sender_fits(show, "booking", "x@fernhollow.com", []))
        lamp = probe_loop("Pick up the lamp from Fernhollow")
        self.assertEqual(ls.sender_fits(lamp, "other", "x@fernhollow.co.uk", []), "the sender the loop names")
        self.assertIsNone(ls.sender_fits(lamp, "other", "x@fernhollow.evil.io", []))

    def test_an_exception_in_a_merges_add_restores_the_originals(self):
        text = ("# Open loops\n\n## Errands\n- [ ] 2026-11-01 · verdict: Corner Cafe · nudge 2026-11-12 · from: src-a\n"
                "- [ ] 2026-11-02 · verdict: River Gallery · nudge 2026-11-11 · from: src-b\n")
        (self.root / ls.OPEN).parent.mkdir(parents=True)
        (self.root / ls.OPEN).write_text(text, encoding="utf-8")
        sw = ls.Sweep(ls.parse_loops(text), PROBE_DAY)
        ls.check_plan(sw, ls.Plan(actions=[{"do": "merge", "ids": [1, 2], "text": "Verdicts: Corner Cafe, River Gallery"}]),
                      self.root, {}, "")
        calls = []

        def run_af(root, args):
            calls.append(args)
            if args[1] == "add" and args[2].startswith("Verdicts"):
                raise TimeoutError("af hung")
            return 0, "ok"
        ls.apply(sw, self.root, run_af)
        self.assertEqual([c[2] for c in calls if c[1] == "add"],
                         ["Verdicts: Corner Cafe, River Gallery", "verdict: Corner Cafe", "verdict: River Gallery"])
        self.assertFalse(any(c.applied for c in sw.changes))
        self.assertIn("TimeoutError: af hung", sw.failures[0])
        self.assertEqual(len(sw.notes), 2)

    def test_a_failed_restore_still_reports_the_close(self):
        sw = SecondReviewTests.merged_sweep(self)

        def run_af(root, args):
            if args[1] == "add":
                raise TimeoutError("af hung")
            return 0, "ok"
        ls.apply(sw, self.root, run_af)
        run_text = ls.render_run(sw, NOW)
        self.assertIn("- Closed: verdict: Corner Cafe", run_text, "a close that stands is in the report")
        self.assertIn("could not restore", run_text)

    def merged_sweep(self):
        text = ("# Open loops\n\n## Errands\n- [ ] 2026-11-01 · verdict: Corner Cafe · nudge 2026-11-12 · from: src-a\n"
                "- [ ] 2026-11-02 · verdict: River Gallery · nudge 2026-11-11 · from: src-b\n")
        (self.root / ls.OPEN).parent.mkdir(parents=True, exist_ok=True)
        (self.root / ls.OPEN).write_text(text, encoding="utf-8")
        sw = ls.Sweep(ls.parse_loops(text), PROBE_DAY)
        ls.check_plan(sw, ls.Plan(actions=[{"do": "merge", "ids": [1, 2], "text": "Verdicts: Corner Cafe, River Gallery"}]),
                      self.root, {}, "")
        return sw

    def test_af_calls_time_out(self):
        with mock.patch.object(ls.subprocess, "run", side_effect=ls.subprocess.TimeoutExpired("af", 1)) as run:
            code, msg = ls.af(self.root, ["open", "due", "--json"])
        self.assertEqual(code, 124)
        self.assertIn("timed out", msg)
        self.assertEqual(run.call_args.kwargs["timeout"], ls.AF_TIMEOUT_S)

    def test_the_marker_is_written_before_anything_is_read(self):
        seen = []
        real_clear = ls.clear_running

        def clear(root):
            seen.append((root / ls.RUNNING).exists())
            real_clear(root)
        with mock.patch.object(ls, "clear_running", side_effect=clear), contextlib.redirect_stderr(io.StringIO()):
            code = ls.main(["--vault", str(self.root)], now=NOW, run_claude=lambda p, m: [], run_af=lambda r, a: (0, ""))
        self.assertEqual(code, 1, "no open.md to read")
        self.assertEqual(seen, [True], "the marker stood even though the run stopped at once")
        self.assertFalse((self.root / ls.RUNNING).exists())


if __name__ == "__main__":
    unittest.main()
