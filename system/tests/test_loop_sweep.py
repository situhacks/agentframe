"""Contract tests for `system/tools/loop_sweep.py`, the daily loop sweep.

The sweep closes a loop only on proof: a loop watch `done` from an allowlisted sender whose email
ties to that loop and no other. Everything else is a report line for main. Every loop, sender and
amount here is invented; no test touches Claude, af, Discord or the network.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
import re
import sys
import tempfile
import unittest
from decimal import Decimal
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import loop_sweep as ls  # noqa: E402
import off_discord  # noqa: E402

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
- [ ] 2026-03-05 · Write the monthly finances review into months/2026-03.md · nudge 2026-03-10 · from: main
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
ET_TEXT = "Claim the e-transfer from the club, $12.00"

WATCH = """# Loop watch

header

## Claim the e-transfer from the club, $12.00

- Verdict: done
- From: `notify@payments.interac.ca (Interac)`
- Date: 2026-03-09
- Subject: `Interac e-Transfer: QUILLON ROWING has sent you $12.00 (CAD)`
- Quote: `your money has been deposited to your account`

## Pick up the widget from Widget Shop

- Verdict: done
- From: `orders@widget.example (Widget Shop)`
- Date: 2026-03-09
- Subject: `Your widget is ready`
- Quote: `your widget is ready for pickup`

## Book the dentist cleaning appointment

- Verdict: update (the model said done; it is from his own sent mail)
- From: `no-reply@booking.example (Clinic)`
- Date: 2026-03-09
- Subject: `Re: cleaning`
- Quote: `see you then`

## Failures

- Something: malformed reply

Run 2026-03-10 07:50 · loops checked 3 · with evidence 3 · failures 1 · skipped over the cap 0 · cost $0.05 · undeleted 0
"""


def loops():
    return ls.parse_loops(OPEN)


def sweep(mode="report-only"):
    return ls.Sweep(loops(), TODAY, mode)


def loop_of(text, section="Errands", n=1):
    open_text = f"## Waiting on the operator\n\n### {section}\n- [ ] 2026-02-20 · {text} · nudge 2026-03-01 · from: main\n"
    loop = ls.parse_loops(open_text)[0]
    loop.n = n
    return loop


def hit(address, subject="", quote="", verdict="done"):
    return ls.WatchHit(verdict, address, "2026-03-09", subject, quote)


def interac(subject, quote=""):
    return hit("notify@payments.interac.ca", subject, quote)


def fake_af():
    calls = []

    def run(root, args):
        calls.append(args)
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

    def test_a_needs_item_with_its_fields_on_the_line_is_a_needs_item(self):
        t = "2026-03-01 · {} · need {}"
        full = ("- [ ] " + t.format("Which plan?", "on 2026-03-09") + " · rec: the cheaper plan · options: a) cheap; b) big"
                " · why: low use · more: knowledge/phone.md#plans · from: src-1")
        bare = "- [ ] " + t.format("Accept?", "when the quote arrives") + " · rec: wait · from: main"
        wrong = "- [ ] " + t.format("Bad order?", "now") + " · why: y · rec: r · from: main"
        norec = "- [ ] " + t.format("No rec?", "now") + " · why: y · from: main"
        got = ls.parse_loops(chr(10).join(["## Decisions needed", "", full, bare, wrong, norec, ""]))
        self.assertEqual([(x.text, x.need, x.detail, x.src) for x in got],
                         [("Which plan?", "on 2026-03-09", None, "src-1"),
                          ("Accept?", "when the quote arrives", None, "main")])
        self.assertEqual(got[0].due_on(), dt.date(2026, 3, 9))
        self.assertIn("needs list", got[1].owned)

    def test_a_one_character_value_is_not_swallowed_by_the_fields_after_it(self):
        # Rust and Python read these two lines the same way; the lazy `??` forms keep each value to itself.
        one = "- [ ] 2026-03-01 · Q? · need now · rec: b · options: a) x; b) y · why: z · from: main"
        plain, need, detail = ls._split_need(one)
        self.assertEqual((plain, need, detail), ("- [ ] 2026-03-01 · Q? · from: main", "now", None))
        trigger = "- [ ] 2026-03-01 · Q? · need when x · rec: ab · from: main"
        plain, need, detail = ls._split_need(trigger)
        self.assertEqual((plain, need, detail), ("- [ ] 2026-03-01 · Q? · from: main", "when x", None))
        self.assertEqual([x.need for x in ls.parse_loops(chr(10).join([one, trigger]))], ["now", "when x"])
        old = "- [ ] 2026-03-01 · Q? · need when x · detail needs/q.md · from: main"
        self.assertEqual(ls._split_need(old)[1:], ("when x", "needs/q.md"))

    def test_a_line_that_breaks_the_grammar_gets_no_number(self):
        text = OPEN.replace("- [ ] 2026-03-01 · Pay the gym fee", "- [ ] 2026-13-01 · Pay the gym fee")
        self.assertEqual(len(ls.parse_loops(text)), 13)
        self.assertEqual(ls.parse_loops(text)[1].text, ET_TEXT)


class WatchTests(unittest.TestCase):
    def test_only_todays_report_counts_and_its_fields_are_read(self):
        hits = ls.read_watch(WATCH, TODAY)
        self.assertEqual(set(hits), {ET_TEXT, "Pick up the widget from Widget Shop", "Book the dentist cleaning appointment"})
        h = hits[ET_TEXT]
        self.assertEqual((h.verdict, h.address, h.date), ("done", "notify@payments.interac.ca", "2026-03-09"))
        self.assertEqual(h.subject, "Interac e-Transfer: QUILLON ROWING has sent you $12.00 (CAD)")
        self.assertEqual(h.quote, "your money has been deposited to your account")
        self.assertEqual(ls.read_watch(WATCH, TODAY + dt.timedelta(days=1)), {})
        self.assertEqual(ls.read_watch("# Loop watch\n\nNo footer.\n", TODAY), {})


class AmountTests(unittest.TestCase):
    def test_an_amount_needs_a_currency_or_a_decimal_part(self):
        self.assertEqual(ls.amounts("Claim $40 from Robin"), {Decimal("40")})
        self.assertEqual(ls.amounts("Claim 40.00 from Robin"), {Decimal("40")}, "40.00 is 40")
        self.assertEqual(ls.amounts("Claim 40.5 from Robin"), {Decimal("40.50")})
        self.assertNotIn(Decimal("40"), ls.amounts("Claim $400 from Robin"), "40 does not match 400")
        self.assertEqual(ls.amounts("Send 40$ and C$15, then 1,200.50, then CAD 7 and 9 CAD"),
                         {Decimal("40"), Decimal("15"), Decimal("1200.50"), Decimal("7"), Decimal("9")})
        for none in ("Return 2 books to the library", "Collect 40 for the gym", "Return item #40", "Budget $40k",
                     "Call at 10:30 on 3/8", "Meet Mar 8 at 8 am", "Lunch the 12th", "Due 2026-03-08",
                     "Cite src-2026-03-08-40", "Sam 40/month", "Windows 10"):
            self.assertEqual(ls.amounts(none), set(), none)

    def test_an_email_states_amounts_with_a_currency(self):
        self.assertEqual(ls.amounts("has sent you $40.00 (CAD)"), {Decimal("40")})
        self.assertEqual(ls.amounts("vous a envoyé 40,00 $"), {Decimal("40")})
        self.assertEqual(ls.amounts("CAD 1,200.00 and 7 CAD"), {Decimal("1200"), Decimal("7")})
        self.assertEqual(ls.amounts("order 40 of 400 items"), set(), "a bare number is not an amount")
        self.assertEqual(ls.amounts("40.001 $"), set(), "40.001 is not 40.00, and 001 is not 1")


MOVED = "Interac e-Transfer: ROBIN MARSH sent you money."


class InteracTests(unittest.TestCase):
    """Rule 1 for Interac: the money moved, and the email names the loop's person or amount."""

    def test_interac_ties_by_the_counterpartys_name(self):
        email = interac(MOVED)
        self.assertIn("person", ls.ties(loop_of("Claim the e-transfer from Robin"), email, {}))
        self.assertIsNone(ls.ties(loop_of("Claim the e-transfer from the club"), email, {}))
        self.assertIsNone(ls.ties(loop_of("claim the e-transfer from robin"), email, {}),
                          "a name must be written as a name in the loop")
        sent = interac("INTERAC e-Transfer: Your money transfer to ROBIN MARSH was deposited.")
        self.assertIsNotNone(ls.ties(loop_of("Send Marsh the deposit"), sent, {}))
        got = interac("INTERAC e-Transfer: You've received $5.00 from ROBIN MARSH and it has been automatically deposited.")
        self.assertIsNotNone(ls.ties(loop_of("It's time to collect from Robin"), got, {}))
        self.assertEqual(ls.interac_names("INTERAC e-Transfer: Rénée Ostrander sent you money."), {"renee", "ostrander"})
        self.assertEqual(ls.interac_names("Deposit complete"), set())

    def test_a_word_that_starts_a_sentence_or_is_a_possessive_is_never_a_name(self):
        for loop, name in (("Bill the client for October", "BILL NG"), ("Chase Dana for the cabin money", "CHASE QUAYLE"),
                           ("Mark the deposit as paid for Dana", "MARK WU"), ("Will check the cabin booking", "WILL SMITH"),
                           ("Robin owes me for the cabin", "ROBIN MARSH"), ("TODO: Bill the client", "BILL NG"),
                           ("Ask Dana. Chase the refund", "CHASE QUAYLE"), ("Send April's rent to the landlord", "APRIL CHEN"),
                           ("Pay rent for April to the landlord", "APRIL CHEN"), ("Send Dana's Chase card back", "CHASE QUAYLE"),
                           ("Ask Dana; Mark has the keys", "MARK WU")):
            email = interac(f"INTERAC e-Transfer: {name} sent you money.")
            self.assertIsNone(ls.ties(loop_of(loop), email, {}), loop)
        self.assertEqual(ls.name_capitals("TODO: Bill the client. Ask Dana about Chase's card, Robin's Mark and Sam in April"),
                         {"dana", "sam"})

    def test_only_a_completed_wording_with_no_refusal_word_anywhere_is_proof(self):
        loop = loop_of("Pay Robin back $40 for dinner")
        for subject, quote in (("INTERAC e-Transfer: ROBIN MARSH sent you money.", ""),
                               ("INTERAC e-Transfer: Your money transfer to ROBIN MARSH was deposited.", ""),
                               ("INTERAC e-Transfer: Your money transfer to ROBIN MARSH has been deposited.", ""),
                               ("INTERAC e-Transfer: ROBIN MARSH accepted your money transfer.", ""),
                               ("Interac e-Transfer", "your $40.00 has been automatically deposited")):
            self.assertIsNotNone(ls.ties(loop, interac(subject, quote), {}), subject)
        for subject, quote in (("INTERAC e-Transfer: Request from ROBIN MARSH", ""),
                               ("INTERAC e-Transfer: ROBIN MARSH has requested $40.00 from you", ""),
                               ("INTERAC e-Transfer: Your money transfer to ROBIN MARSH was cancelled.", ""),
                               ("INTERAC e-Transfer: Your request to ROBIN MARSH for $40.00 was declined", "deposited"),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH has expired", ""),
                               ("Reminder: ROBIN MARSH sent you money.", ""),
                               ("INTERAC e-Transfer: ROBIN MARSH sent you money.", "it has not been deposited yet"),
                               ("INTERAC e-Transfer: ROBIN MARSH sent you money.", "it hasn't been deposited"),
                               ("INTERAC e-Transfer: a transfer to ROBIN MARSH", "$40.00 is on its way"),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH was returned and deposited back to your account.", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH was reversed; funds deposited back.", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH was refunded and deposited.", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH failed. Funds deposited back.", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH was stopped. Amount deposited back.", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH has been reclaimed and deposited to you", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH was never deposited", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH is pending; it will be deposited when accepted", ""),
                               ("INTERAC e-Transfer: ROBIN MARSH was unable to be deposited", ""),
                               ("INTERAC e-Transfer: Your transfer to ROBIN MARSH", "$40.00 deposited"),
                               ("INTERAC e-Transfer: Autodeposit to ROBIN MARSH", "funds were deposited earlier")):
            self.assertIsNone(ls.ties(loop, interac(subject, quote), {}), (subject, quote))

    def test_a_refusal_word_in_any_line_of_the_watch_report_stops_the_proof(self):
        watch = WATCH.replace("- Date: 2026-03-09\n", "- Date: 2026-03-09\n- Note: `the transfer was reversed`\n", 1)
        sw = sweep()
        ls.find_proofs(sw, ls.read_watch(watch, TODAY), {})
        self.assertEqual(sw.closes, [])
        self.assertIn("reversed", ls.read_watch(watch, TODAY)[ET_TEXT].everything())
        self.assertNotIn("failures", ls.read_watch(WATCH, TODAY)[ET_TEXT].everything(), "the footer is not the email")

    def test_interac_ties_by_the_exact_amount(self):
        email = interac("Interac e-Transfer: $40.00 (CAD) has been deposited")
        self.assertIn("amount", ls.ties(loop_of("Collect $40 for the gym"), email, {}))
        self.assertIn("amount", ls.ties(loop_of("Collect 40.00 for the gym"), email, {}))
        self.assertIsNone(ls.ties(loop_of("Collect 40 for the gym"), email, {}), "a bare number is no amount")
        self.assertIsNone(ls.ties(loop_of("Collect $400 for the gym"), email, {}), "40 does not match 400")
        self.assertIsNone(ls.ties(loop_of("Collect $4 for the gym"), email, {}))
        self.assertIsNone(ls.ties(loop_of("Return 2 books to the library"), interac("$2.00 has been deposited"), {}))
        quoted = interac("Deposit complete", "the $40.00 has been deposited")
        self.assertIsNotNone(ls.ties(loop_of("Collect 40.00 for the gym"), quoted, {}))

    def test_only_interac_itself_is_interac(self):
        loop = loop_of("Claim the e-transfer from Robin")
        self.assertIsNotNone(ls.ties(loop, hit("x@INTERAC.CA", MOVED), {}))
        for address in ("x@interac.ca.evil.example", "x@interac-ca.example", "x@evilinterac.ca", "x@interac.ca.",
                        "x@ınterac.ca", "x@gmail.com", "no address"):
            self.assertIsNone(ls.ties(loop, hit(address, MOVED), {}), address)


class ProofTests(unittest.TestCase):
    """Rule 1: a close needs an allowlisted sender and a tie to this loop."""
    CARDS = {"Fernhollow Visa": ["fernhollow.example"], "Clementine Mastercard": ["clementine.example"],
             "Visa": ["bank.example"], "Freedom": ["quince.example"], "Larch": ["larchtrust.example"]}

    def test_a_card_issuer_ties_only_with_a_card_word_beside_it_or_the_cards_full_name(self):
        email = hit("alerts@fernhollow.example", "Payment received")
        for loop in ("Pay the Fernhollow statement", "Pay the fernhollow visa", "Fernhollow card: pay the balance",
                     "Pay the bill for Fernhollow"):
            self.assertIn("card", ls.ties(loop_of(loop), email, self.CARDS), loop)
        for none in ("Pay Robin back $40", "Pay the visa application fee", "Buy fernhollows for the garden",
                     "Call Fernhollow about the fee", "Pay the Fernhollow lodge for the weekend, then the card"):
            self.assertIsNone(ls.ties(loop_of(none), email, self.CARDS), none)
        self.assertIsNone(ls.ties(loop_of("Pay the Fernhollow statement"), email, {}), "an issuer not in bills.md")
        clementine = hit("statements@clementine.example", "Your statement")
        self.assertIsNotNone(ls.ties(loop_of("Pay the Clementine card"), clementine, self.CARDS))
        self.assertIsNotNone(ls.ties(loop_of("Ask about the Clementine Mastercard fee"), clementine, self.CARDS),
                             "the card's full name from bills.md")
        for none in ("Buy clementines for the party", "Buy a clementine for the party"):
            self.assertIsNone(ls.ties(loop_of(none), clementine, self.CARDS), none)
        quince = hit("a@quince.example", "Statement ready")
        self.assertIsNone(ls.ties(loop_of("Quince Sam for the cabin money"), quince, self.CARDS),
                          "an everyday-word issuer with no card word beside it")
        self.assertIsNotNone(ls.ties(loop_of("Pay the Quince credit card"), quince, self.CARDS))
        self.assertIsNone(ls.ties(loop_of("Plant the larch by the fence"), hit("a@larchtrust.example"), self.CARDS),
                          "a one-word card name needs a card word too")
        bank = hit("alerts@bank.example", "Statement ready")
        self.assertIsNone(ls.ties(loop_of("Renew the travel visa for the trip"), bank, self.CARDS),
                          "a card named only for its network names nothing")

    def test_a_ticketing_receipt_must_name_the_loops_event_and_nothing_cancelled(self):
        receipt = hit("orders@eventbrite.com", "Your tickets for Harbour Lantern Night")
        self.assertIn("event", ls.ties(loop_of("Get the Eventbrite tickets for lantern night"), receipt, {}))
        self.assertIsNone(ls.ties(loop_of("Get tickets for the lantern night by the harbour"), receipt, {}),
                          "the loop must name the platform")
        self.assertIsNone(ls.ties(loop_of("Get the Eventbrite tickets for jazz night"), receipt, {}),
                          "a receipt for another event")
        self.assertIsNone(ls.ties(loop_of("Get the Eventbrite tickets for Friday night"),
                                  hit("orders@eventbrite.com", "Your order for Friday night"), {}),
                          "a weekday and ticket words are not an event")
        self.assertIsNone(ls.ties(loop_of("Get the Eventbrite tickets for lantern night"),
                                  hit("orders@eventbrite.com", "New events near you: lantern night"), {}), "not a receipt")
        for subject, quote in (("Order cancelled: Harbour Lantern Night", ""), ("Your order for Lantern Night", "refund issued"),
                               ("Order confirmed: Lantern Night", "this order was cancelled")):
            self.assertIsNone(ls.ties(loop_of("Get the Eventbrite tickets for lantern night"),
                                      hit("orders@eventbrite.com", subject, quote), {}), subject)
        self.assertIsNone(ls.ties(loop_of("Book the Marigold table via OpenTable"),
                                  hit("x@opentable.com", "Reservation cancelled; confirmation 123 at Marigold"), {}))
        self.assertIsNotNone(ls.ties(loop_of("Book the Marigold table via OpenTable"),
                                     hit("x@opentable.com", "Your reservation at Marigold is confirmed"), {}))
        for subject in ("Order #4412 receipt: Lakeside Strings", "Booking confirmed, Lakeside Strings",
                        "Your confirmation for Lakeside Strings"):
            self.assertIsNotNone(ls.ties(loop_of("Sort out the Ticketmaster seats for Lakeside Strings"),
                                         hit("x@ticketmaster.ca", subject), {}), subject)
        self.assertIsNotNone(ls.ties(loop_of("Book the Tock dinner at Marigold"),
                                     hit("x@exploretock.com", "Your reservation at Marigold is confirmed"), {}),
                             "the loop uses the platform's own name, not its domain")
        for address, loop in (("info@meetup.com", "Meetup with Sam about lantern night"),
                              ("x@dice.fm", "Buy dice for lantern night"),
                              ("x@universe.com", "Watch the Universe lantern documentary"),
                              ("x@partiful.com", "Partiful invite for the lantern potluck")):
            self.assertIsNone(ls.ties(loop_of(loop), hit(address, "Order confirmed: lantern"), {}), address)
        self.assertIsNone(ls.ties(loop_of("Get the Eventbrite tickets for lantern night"),
                                  hit("orders@harbourlantern.example", "Your tickets for lantern night"), {}))

    def test_a_sender_the_loop_names_is_not_proof(self):
        """The named-sender close is gone: anyone can register a domain."""
        self.assertIsNone(ls.ties(loop_of("Pick up the widget from Widget Shop"), hit("orders@widget.example"), {}))

    def test_a_proof_closes_its_loop_and_says_how(self):
        sw = sweep()
        ls.find_proofs(sw, ls.read_watch(WATCH, TODAY), {})
        self.assertEqual([c.loop.n for c in sw.closes], [N["etransfer"]])
        outcome = sw.closes[0].outcome
        self.assertTrue(outcome.startswith("Sweep 2026-03-10: an email of 2026-03-09 from payments.interac.ca"))
        self.assertIn("amount", outcome)
        self.assertNotIn("deposited", outcome, "no email text in an outcome")
        self.assertEqual(len(sw.unproved), 1)
        self.assertIn(f"#{N['widget']} Pick up the widget", sw.unproved[0])
        self.assertIn("not proof", sw.unproved[0])

    def test_a_proof_that_fits_two_loops_closes_neither(self):
        text = OPEN.replace("Buy a warm winter hat and gloves", "Pay back the $12.00 for the hat")
        sw = ls.Sweep(ls.parse_loops(text), TODAY)
        ls.find_proofs(sw, ls.read_watch(WATCH, TODAY), {})
        self.assertEqual(sw.closes, [])
        self.assertEqual(len(sw.ambiguous), 1)
        self.assertIn(f"#{N['etransfer']} Claim", sw.ambiguous[0])
        self.assertIn(f"#{N['hat']} Pay back", sw.ambiguous[0])
        self.assertIn("none closed", sw.ambiguous[0])

    def test_a_name_that_is_also_a_word_of_another_loop_is_ambiguous(self):
        watch = WATCH.replace("QUILLON ROWING", "CAFE ROWING")  # a loop reads "verdict: Corner Cafe"
        sw = sweep()
        ls.find_proofs(sw, ls.read_watch(watch, TODAY), {})
        self.assertEqual(sw.closes, [])
        self.assertIn(f"#{N['cafe']} verdict: Corner Cafe", sw.ambiguous[0])
        river = WATCH.replace("QUILLON ROWING", "RIVER ROWING")  # "verdict: River Gallery": after a colon, no name
        sw = sweep()
        ls.find_proofs(sw, ls.read_watch(river, TODAY), {})
        self.assertEqual([c.loop.n for c in sw.closes], [N["etransfer"]])

    def test_an_owned_loop_counts_toward_ambiguity(self):
        text = OPEN.replace("Sam owes $15 for the tickets", "The club owes $12.00 for the boat")
        sw = ls.Sweep(ls.parse_loops(text), TODAY)
        ls.find_proofs(sw, ls.read_watch(WATCH, TODAY), {})
        self.assertEqual(sw.closes, [])
        self.assertIn(f"#{N['others']} The club owes", sw.ambiguous[0])

    def test_two_loops_with_the_same_text_are_ambiguous(self):
        text = OPEN.replace("- [ ] 2026-03-01 · Buy a warm winter hat and gloves",
                            f"- [ ] 2026-03-01 · {ET_TEXT}")
        sw = ls.Sweep(ls.parse_loops(text), TODAY)
        ls.find_proofs(sw, ls.read_watch(WATCH, TODAY), {})
        self.assertEqual(sw.closes, [])
        self.assertEqual(len(sw.ambiguous), 1, "the one email is reported once")

    def test_a_capped_verdict_or_an_owned_loop_proves_nothing(self):
        sw = sweep()
        watch = WATCH.replace("Claim the e-transfer from the club, $12.00\n\n- Verdict: done",
                              "Claim the e-transfer from the club, $12.00\n\n- Verdict: update (capped)")
        ls.find_proofs(sw, ls.read_watch(watch, TODAY), {})
        self.assertEqual(sw.closes, [])
        owned = WATCH.replace(f"## {ET_TEXT}", "## Sam owes $15 for the tickets")
        sw = sweep()
        ls.find_proofs(sw, ls.read_watch(owned, TODAY), {})
        self.assertEqual(sw.closes, [])


class OverdueTests(unittest.TestCase):
    def test_every_overdue_loop_not_owned_is_handed_on(self):
        sw = sweep()
        ls.find_overdue(sw)
        names = {x.n for x in sw.overdue}
        self.assertEqual(names, {N["neighbour"], N["gym"], N["etransfer"], N["hat"], N["weigh"], N["widget"],
                                 N["dentist_old"]})
        for not_overdue in ("finances", "cafe", "dentist_new", "others", "needs", "bills"):
            self.assertNotIn(N[not_overdue], names, not_overdue)

    def test_only_an_applied_close_leaves_the_overdue_list(self):
        """A failed close stays reminded; a close not tried (report-only) is listed once, as would-close."""
        for applied, handed in ((None, False), (False, True), (True, False)):
            sw = sweep()
            sw.closes = [ls.Close(loops()[N["etransfer"] - 1], "x", applied)]
            ls.find_overdue(sw)
            self.assertEqual(N["etransfer"] in {x.n for x in sw.overdue}, handed, applied)

    def test_the_handoff_carries_his_own_loop_text_and_nothing_written_for_it(self):
        sw = sweep()
        ls.find_overdue(sw)
        items = {x["text"]: x for x in ls.handoff_items(sw)}
        self.assertEqual(items["Pay the gym fee of $40 by 03-08"],
                         {"text": "Pay the gym fee of $40 by 03-08", "due": "2026-03-07", "days": 3})
        self.assertTrue(all(set(x) == {"text", "due", "days"} for x in items.values()))

    def test_the_sweep_sends_nothing_itself_and_writes_todays_handoff(self):
        sw = sweep()
        ls.find_overdue(sw)
        with tempfile.TemporaryDirectory() as d, mock.patch("notify.main") as notify_main:
            ls.handoff(sw, Path(d))
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
        plan = ls.parse_plan('```json\n{"notice": [{"group": "duplicate", "ids": [10, 11, 10, "x", true]}, '
                             '{"group": "redate", "ids": [4]}, {"group": "stale"}], "board": [{"id": 4}, "x"]}\n```')
        self.assertEqual(plan.notice, [("duplicate", [10, 11])])
        self.assertFalse(hasattr(plan, "board"), "a board in the reply is ignored")
        self.assertEqual(ls.parse_plan('{"notice": []}').notice, [])
        for bad in ("not json", "[]", '{"notice": {}}', '{"loops": [], "actions": [], "board": []}'):
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
        self.assertEqual(args[args.index("--model") + 1], json.loads((ROOT / "system" / "models.json").read_text(encoding="utf-8"))["sonnet"])
        self.assertNotEqual(Path(kw["cwd"]), ROOT)
        denied = args[args.index("--disallowedTools") + 1].split(",")
        self.assertIn("mcp__claude_ai_Gmail__search_threads", denied)

    def test_the_prompt_holds_the_loops_only(self):
        p = ls.prompt_for(loops(), TODAY)
        self.assertIn("Today is 2026-03-10 (Tuesday)", p)
        self.assertIn('"text": "Pay the gym fee of $40 by 03-08"', p)
        self.assertIn('"overdue_days": 3', p)
        self.assertIn("changes no loop", p)
        for gone in ("close_", "redate", "<file", "Interac", "board", "Discord"):
            self.assertNotIn(gone, p)


class NoticeTests(unittest.TestCase):
    def test_entries_become_numbers_and_the_loops_own_text(self):
        sw = sweep()
        plan = ls.Plan(notice=[("duplicate", [N["dentist_old"], N["dentist_new"]]), ("merge", [N["cafe"], N["gallery"]]),
                               ("stale", [N["finances"], 99]), ("duplicate", [N["hat"]]), ("merge", [98, 99])])
        ls.take_notice(sw, plan)
        self.assertEqual(sw.noticed["duplicate"], [f"#{N['dentist_old']} Book the dentist cleaning appointment; "
                                                   f"#{N['dentist_new']} Book the dentist cleaning appointment soon"])
        self.assertEqual(sw.noticed["merge"], [f"#{N['cafe']} verdict: Corner Cafe, brunch 03-01; "
                                               f"#{N['gallery']} verdict: River Gallery, show 03-02"])
        self.assertEqual(sw.noticed["stale"], [f"#{N['finances']} Write the monthly finances review into months/2026-03.md"])

    def test_each_group_is_capped(self):
        sw = sweep()
        ls.take_notice(sw, ls.Plan(notice=[("stale", [x.n for x in loops()])]))
        self.assertEqual(len(sw.noticed["stale"]), min(ls.MAX_NOTICED, 14))


# The reviews' cases: money, deadline, registration and appointment loops whose date passed.
PASSED = ["Send rent to landlord Mar 1", "Transfer 500 to Sam on Mar 3", "Submit tax return Mar 3",
          "Hand in lease renewal form by Mar 3", "Register for the half marathon", "Return the shoes before the window closes",
          "Cover Jo's share of the cabin", "Contribute to the savings plan before March", "Ask Bill about the hike",
          "Call Robin on Mar 3", "Repay Sam for the desk on Mar 3", "Talk to the landlord about the deposit"]


class RunTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        life = self.root / "workspace" / "life"
        (life / "systems").mkdir(parents=True)
        (life / "open.md").write_text(OPEN, encoding="utf-8")
        (life / "systems" / "loop-watch-report.md").write_text(WATCH, encoding="utf-8")
        self.prompts = []
        self.plan = {"notice": [{"group": "duplicate", "ids": [N["dentist_old"], N["dentist_new"]]},
                                {"group": "merge", "ids": [N["cafe"], N["gallery"]]},
                                {"group": "stale", "ids": [N["finances"]]}]}

    def tearDown(self):
        self.tmp.cleanup()

    def claude(self, prompt, model):
        self.prompts.append(prompt)
        reply = self.plan if isinstance(self.plan, str) else json.dumps(self.plan)
        return [json.dumps({"type": "system", "subtype": "init", "tools": [], "mcp_servers": []}),
                json.dumps({"type": "result", "subtype": "success", "result": reply, "total_cost_usd": 0.11})]

    def files(self):
        return sorted(str(p.relative_to(self.root)) for p in self.root.rglob("*") if p.is_file())

    def terms_file(self):
        (self.root / off_discord.TERMS_FILE).parent.mkdir(parents=True, exist_ok=True)
        (self.root / off_discord.TERMS_FILE).write_text("# terms kept off Discord\n", encoding="utf-8")

    def run_main(self, *argv, run_af=None):
        run_af = run_af or fake_af()[0]
        out = io.StringIO()
        with contextlib.redirect_stdout(out), mock.patch("notify.main", return_value=0) as push, \
                mock.patch("discord_board.main", return_value=0) as board:
            code = ls.main(["--vault", str(self.root), *argv], now=NOW, run_claude=self.claude, run_af=run_af)
        board.assert_not_called()
        return code, out.getvalue(), push

    def report(self):
        return (self.root / ls.REPORT).read_text(encoding="utf-8")

    def test_report_only_is_the_default_writes_the_report_and_handoff_and_applies_nothing(self):
        self.terms_file()
        before = (self.root / ls.OPEN).read_bytes()
        run_af, calls = fake_af()
        code, out, push = self.run_main(run_af=run_af)
        self.assertEqual(code, 0, out)
        self.assertEqual(calls, [], "no af call at all")
        self.assertEqual((self.root / ls.OPEN).read_bytes(), before)
        report = self.report()
        self.assertIn(f"- Would close (report-only, not applied): #{N['etransfer']} {ET_TEXT} → Sweep 2026-03-10: "
                      "an email of 2026-03-09 from payments.interac.ca", report)
        self.assertIn("### Passed with no proof", report)
        self.assertNotIn(f"- #{N['etransfer']} {ET_TEXT} (due", report, "listed once, under would-close")
        self.assertEqual(report.count(ET_TEXT), 1)
        self.assertIn("### Looks like a duplicate", report)
        self.assertIn("### Could be merged", report)
        self.assertIn("### Looks stale", report)
        self.assertNotIn("Board", report)
        self.assertRegex(report, r"(?m)^Loops 14 · report-only · closed 0 · would close 1 · ambiguous 0 · "
                                 r"passed with no proof 6 · duplicates 1 · merges 1 · stale 1 · failures 0 · cost \$0\.11$")
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertNotIn(ET_TEXT, [x["text"] for x in handed["items"]])
        self.assertIn("Pay the gym fee of $40 by 03-08", [x["text"] for x in handed["items"]])
        push.assert_not_called()
        self.assertFalse((self.root / "workspace/life/systems/discord/boards/open.md").exists())

    def test_live_closes_on_proof_through_af_and_posts_nothing(self):
        self.terms_file()
        run_af, calls = fake_af()
        code, out, push = self.run_main("--live", run_af=run_af)
        self.assertEqual(code, 0, out)
        self.assertEqual(calls, [["open", "close", ET_TEXT, "--outcome",
                                  "Sweep 2026-03-10: an email of 2026-03-09 from payments.interac.ca (Interac, money "
                                  "moved, for the amount the loop names) proves it done"]])
        report = self.report()
        self.assertIn(f"- Closed: #{N['etransfer']} {ET_TEXT}", report)
        self.assertNotIn(f"- #{N['etransfer']} {ET_TEXT} (due", report, "closed, so not reminded")
        self.assertIn("· live · closed 1 · would close 0", report)
        self.assertNotIn("Board", report)
        push.assert_not_called()
        self.assertFalse((self.root / "workspace/life/systems/discord/boards/open.md").exists())

    def test_the_plan_asks_for_changes_and_nothing_but_the_proof_close_is_applied(self):
        """Old-shape actions in the reply are ignored; what they asked for is only reported."""
        self.plan["actions"] = [{"do": "close_day_passed", "id": N["neighbour"]},
                                {"do": "redate", "id": N["widget"], "nudge": "2026-03-14"},
                                {"do": "merge", "ids": [N["cafe"], N["gallery"]], "text": "Verdicts"},
                                {"do": "end_duplicate", "id": N["dentist_old"], "keep": N["dentist_new"]},
                                {"do": "close_vault", "id": N["weigh"], "file": "workspace/life/log.md", "quote": "x"}]
        run_af, calls = fake_af()
        code, out, _ = self.run_main("--live", run_af=run_af)
        self.assertEqual(code, 0, out)
        self.assertEqual([c[:3] for c in calls], [["open", "close", ET_TEXT]])
        report = self.report()
        for asked in (f"- #{N['neighbour']} Ask the neighbour", f"- #{N['widget']} Pick up the widget",
                      f"- #{N['weigh']} Weekly weigh-in"):
            self.assertIn(asked, report.split("### Passed with no proof")[1].split("###")[0])
        self.assertIn(f"#{N['dentist_old']} Book the dentist cleaning appointment; #{N['dentist_new']}", report)
        self.assertIn(f"#{N['cafe']} verdict: Corner Cafe, brunch 03-01; #{N['gallery']}", report)

    def test_a_passed_date_closes_nothing_and_each_loop_is_reminded(self):
        body = "".join(f"- [ ] 2026-02-20 · {t} · nudge 2026-03-03 · from: main\n" for t in PASSED)
        (self.root / ls.OPEN).write_text(f"## Waiting on the operator\n\n### Errands\n{body}", encoding="utf-8")
        self.plan = {"notice": []}
        run_af, calls = fake_af()
        code, out, _ = self.run_main("--live", run_af=run_af)
        self.assertEqual(calls, [])
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertEqual([x["text"] for x in handed["items"]], PASSED)
        for i, t in enumerate(PASSED, 1):
            self.assertIn(f"- #{i} {t} (due 2026-03-03)", self.report())

    def test_a_failed_close_is_reported_and_the_loop_stays_open_and_reminded(self):
        def run_af(root, args):
            return 1, "no open loop starts with that text"
        code, out, _ = self.run_main("--live", run_af=run_af)
        self.assertEqual(code, 1)
        report = self.report()
        self.assertIn(f"- Failed: close #{N['etransfer']} {ET_TEXT}: no open loop starts with that text; it stays open",
                      report)
        self.assertNotIn("- Closed:", report)
        self.assertIn(f"- #{N['etransfer']} {ET_TEXT} (due 2026-03-09)", report)
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertIn(ET_TEXT, [x["text"] for x in handed["items"]])

    def test_a_close_that_raises_fails_alone(self):
        sw = sweep("live")
        sw.closes = [ls.Close(loops()[N["etransfer"] - 1], "a"), ls.Close(loops()[N["hat"] - 1], "b")]
        seen = []

        def run_af(root, args):
            seen.append(args[2])
            if len(seen) == 1:
                raise FileNotFoundError("af is not on PATH")
            return 0, "ok"
        ls.apply(sw, self.root, run_af)
        self.assertEqual([c.applied for c in sw.closes], [False, True])
        self.assertIn("FileNotFoundError: af is not on PATH; it stays open", sw.failures[0])

    def test_a_loop_whose_text_is_not_unique_is_never_closed_by_number(self):
        text = OPEN.replace("Buy a warm winter hat and gloves", "Claim the e-transfer from the club, $12.00")
        (self.root / ls.OPEN).write_text(text, encoding="utf-8")
        sw = ls.Sweep(ls.parse_loops(text), TODAY, "live")
        sw.closes = [ls.Close(sw.loops[N["etransfer"] - 1], "x")]
        run_af, calls = fake_af()
        ls.apply(sw, self.root, run_af)
        self.assertEqual(calls, [])
        self.assertIn("same text", sw.failures[0])
        self.assertIsNone(ls.target(loops(), loop_of("-starts with a dash")))
        self.assertIsNone(ls.target(loops(), loop_of("2026")))

    def test_a_loop_whose_line_changed_since_the_read_is_left_alone(self):
        sw = sweep("live")
        sw.closes = [ls.Close(sw.loops[N["etransfer"] - 1], "x")]
        (self.root / ls.OPEN).write_text(OPEN.replace(ET_TEXT, ET_TEXT + " for the regatta"), encoding="utf-8")
        run_af, calls = fake_af()
        ls.apply(sw, self.root, run_af)
        self.assertEqual(calls, [], "af would match the longer text by prefix")
        self.assertIn("changed since the sweep read it", sw.failures[0])
        self.assertFalse(sw.closes[0].applied)

    def test_a_dry_run_changes_sends_and_writes_nothing(self):
        self.terms_file()
        before = {p: (self.root / p).read_bytes() for p in self.files()}
        run_af, calls = fake_af()
        code, out, push = self.run_main("--dry-run", run_af=run_af)
        self.assertEqual(code, 0, out)
        self.assertEqual({p: (self.root / p).read_bytes() for p in self.files()}, before)
        self.assertEqual(calls, [])
        push.assert_not_called()
        self.assertIn("Would close (dry-run, not applied)", out)
        self.assertIn("What the morning push would lead with:", out)
        self.assertIn("Dry run.", out)

    def test_a_later_morning_can_be_previewed_only_as_a_dry_run(self):
        code, out, _ = self.run_main("--dry-run", "--today", "2026-03-12")
        self.assertIn("Loop sweep plan for 2026-03-12", out)
        self.assertIn("Today is 2026-03-12", self.prompts[-1])
        for argv in (["--today", "2026-03-12"], ["--dry-run", "--live"]):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                self.run_main(*argv)
        self.assertFalse((self.root / ls.REPORT).exists())

    def test_the_model_never_sees_email(self):
        self.run_main("--dry-run")
        self.assertEqual(len(self.prompts), 1)
        for email_text in ("payments.interac.ca", "deposited", "QUILLON ROWING", "Loop watch"):
            self.assertNotIn(email_text, self.prompts[0])

    def test_a_crash_after_closing_still_writes_the_report(self):
        with mock.patch.object(ls, "handoff", side_effect=PermissionError("locked")), \
                contextlib.redirect_stderr(io.StringIO()):
            code, out, _ = self.run_main("--live")
        self.assertEqual(code, 1)
        report = self.report()
        self.assertIn("- Closed: #3", report)
        self.assertIn("- Failed: the run stopped: PermissionError: locked", report)
        self.assertFalse((self.root / ls.RUNNING).exists())

    def test_an_unwritable_report_still_logs_and_prints_the_run(self):
        (self.root / ls.REPORT).mkdir(parents=True)  # a directory where the report goes
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code, out, _ = self.run_main("--live")
        self.assertEqual(code, 1)
        self.assertIn("could not write the report", err.getvalue())
        self.assertIn("- Closed: #3", out)
        self.assertIn("closed 1", (self.root / ls.LOG).read_text(encoding="utf-8"))

    def test_a_run_that_writes_marks_itself_running_for_the_push_and_a_dry_run_does_not(self):
        seen = []
        real = self.claude

        def claude(prompt, model):
            seen.append((self.root / ls.RUNNING).exists())
            return real(prompt, model)
        with contextlib.redirect_stdout(io.StringIO()):
            for argv in ([], ["--live"], ["--dry-run"]):
                ls.main(["--vault", str(self.root), *argv], now=NOW, run_claude=claude, run_af=fake_af()[0])
        self.assertEqual(seen, [True, True, False])
        self.assertFalse((self.root / ls.RUNNING).exists())

    def test_no_plan_still_closes_on_proof_and_reminds(self):
        self.plan = "not json"
        run_af, calls = fake_af()
        code, out, _ = self.run_main("--live", run_af=run_af)
        self.assertEqual(code, 1)
        self.assertEqual([c[:3] for c in calls], [["open", "close", ET_TEXT]])
        handed = json.loads((self.root / ls.HANDOFF).read_text(encoding="utf-8"))
        self.assertIn("Pay the gym fee of $40 by 03-08", [x["text"] for x in handed["items"]],
                      "with no plan the loop's own text goes, for the push to filter")
        report = self.report()
        self.assertIn("- Failed: no plan: the model's reply is not JSON", report)

    def test_the_report_keeps_the_last_runs(self):
        old = ls.HEADER + "\n" + "".join(f"## Run 2026-01-{i:02d} 08:20\n\nLoops 1\n\n" for i in range(1, 31)) * 3
        new = ls.append_report(old, "## Run 2026-03-10 08:20\n\nLoops 2\n")
        self.assertEqual(new.count("## Run "), ls.KEEP_RUNS)
        self.assertTrue(new.rstrip().endswith("Loops 2"))
        self.assertTrue(new.startswith("# Loop sweep"))


class ScheduleTests(unittest.TestCase):
    TOOLS = ROOT / "system" / "tools"

    def text(self, name):
        return (self.TOOLS / name).read_text(encoding="utf-8")

    def test_the_batch_run_starts_the_sweep_after_the_loop_watch_and_before_the_push(self):
        import watch_batch as wb
        names = [name for name, _, _ in wb.MORNING]
        self.assertEqual(names, ["bills", "loop-watch", "loop-sweep", "harness-review"])
        self.assertLess(names.index("loop-watch"), names.index("loop-sweep"), "the sweep reads the watch's report")
        self.assertNotIn(wb.PUSH[0], names, "the push runs after the queue, so it carries what the queue raised")
        self.assertEqual(wb.PUSH[1], "push_due_loops.py")
        minutes = next(m for n, s, m in wb.MORNING if s == "loop_sweep.py")
        self.assertGreater(minutes * 60, ls.TIMEOUT_S + 120)

    def test_the_batch_task_registers_a_report_only_sweep_unless_a_live_one_is_asked_for(self):
        ps1 = self.text("register-watch-batch.ps1")
        self.assertIn("param([switch]$SweepLive)", ps1)
        base = next(line for line in ps1.splitlines() if line.startswith("$argument = "))
        self.assertNotIn("Live", base, "the default registration is report-only")
        self.assertIn("if ($SweepLive) { $argument += ' -SweepLive' }", ps1)
        self.assertIn("-Argument $argument", ps1)
        self.assertIn("Registered '$taskName' ($mode)", ps1)

    def test_the_entry_script_passes_live_only_when_asked(self):
        ps1 = self.text("loop-sweep.ps1")
        self.assertIn("if ($Live) { $pyArgs += '--live' }", ps1)
        self.assertEqual(ps1.count("$pyArgs += '--live'"), 1)
        head = ps1.split("param(")[0]
        self.assertIn("Report-only by default", head)
        self.assertNotIn("to his phone", head)

    def test_the_timers_script_shows_the_sweep_mode_of_the_batch_task(self):
        timers = self.text("timers.ps1")
        self.assertIn("@{ Script = 'register-watch-batch.ps1'; Task = 'AgentFrame batch' }", timers)
        self.assertIn("-SweepLive", timers)
        self.assertIn("report-only sweep", timers)

    def test_the_tracked_files_hold_no_address_beyond_generic_services(self):
        for name in ("loop_sweep.py", "loop-sweep.ps1"):
            self.assertIsNone(re.search(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+\.[A-Za-z]{2,}", self.text(name)), name)

    def test_the_push_watches_the_sweeps_marker(self):
        import push_due_loops
        self.assertEqual(push_due_loops.SWEEP_RUNNING, ls.RUNNING)
        self.assertEqual(push_due_loops.HANDOFF, ls.HANDOFF)

    def test_the_judgement_machinery_is_gone(self):
        for name in ("zero_stakes", "guarded", "urgency_of", "plain_social", "SOCIAL_RE", "STAKE_PARTS", "check_action",
                     "_apply_add", "redated_before", "evidence_files", "check_numbering", "similarity",
                     "render_board", "post_board", "publish_board", "board_lines", "BOARD", "BOARD_PRIVATE",
                     "loop_amounts", "email_amounts"):
            self.assertFalse(hasattr(ls, name), name)


if __name__ == "__main__":
    unittest.main()
