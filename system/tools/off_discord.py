"""What stays off Discord: the one filter for every surface that posts his loops there.

Discord is a third-party service. A loop whose text, or the line written for it, matches the
built-in sensitive categories (health, medication, therapy, passwords, account numbers) or one of
his private terms is kept off: the board leaves it out, and the push counts it without its words.

Text is normalised before any match (normalize()): NFKC, so fullwidth letters and signs read as
plain ones; zero-width and soft-hyphen characters removed; and Cyrillic and Greek lookalike
letters read as the Latin letters they imitate. "ＭＥＤＳ", "den<zero-width>tist" and "mеds" with
a Cyrillic e all match.

His own terms (names, topics) live in the private, gitignored
workspace/life/systems/discord/boards/open-private.txt, one a line, `#` for a comment. Without
that file nothing that names a loop goes out: the board is not posted, and the push sends only a
count.

Numbers are masked, not hidden (mask()). Everywhere: any token holding four or more digits that
is not a date or a time, and any grouped digit run (123-456-789, 604-555-1234, 4520 1234 5678).
In the push only: an amount of money in any notation ($40, C$40, 40 CAD, USD 1,200, 40 EUR,
¥5000, 40 bucks). The push shows as a notification preview on his lock screen; the #open board is
a pinned message he opens, and what his bills come to is its point.
"""

from __future__ import annotations

import re
import unicodedata
from pathlib import Path

TERMS_FILE = Path("workspace/life/systems/discord/boards/open-private.txt")

_INVISIBLE = dict.fromkeys(map(ord, "­͏؜ᅟᅠ឴឵᠎​‌‍‎"
                                    "‏‪‫‬‭‮⁠⁡⁢⁣⁤⁦"
                                    "⁧⁨⁩ㅤ﻿ﾠ"))
# Cyrillic and Greek letters that look like Latin ones.
_LOOKALIKE = str.maketrans("аеорсухіјѕԁԛԝкмнтвгпАВЕКМНОРСТХІЈЅԜΑΒΕΖΗΙΚΜΝΟΡΤΥΧαεικνορτυχ",
                           "aeopcyxijsdqwkmhtbrnABEKMHOPCTXIJSWABEZHIKMNOPTYXaeikvoptux")


def normalize(text: str) -> str:
    """The text as the filters read it: NFKC, no invisible characters, lookalikes as Latin."""
    return unicodedata.normalize("NFKC", text).translate(_INVISIBLE).translate(_LOOKALIKE)


# Generic categories only; anything about him belongs in the private terms file.
SENSITIVE = re.compile(
    r"\b(?:health|medical|meds|medic(?:ation|ine)s?|prescri\w*|refills?|pharmac\w*|rx|doctors?|doc|dr\b\.?|gp|"
    r"physicians?|clinics?|hospitals?|urgent care|dental|dentists?|orthodont\w*|optometr\w*|opticians?|"
    r"dermatolog\w*|chiropract\w*|vaccin\w*|flu shots?|check-?ups?|physio\w*|therap\w*|counsel\w*|psych\w*|"
    r"mental|diagnos\w*|symptoms?|mri|x-?rays?|ct scans?|ultrasounds?|\w+oscopy|biops\w*|allerg\w*|ob/?gyn|"
    r"gyn\w*|urolog\w*|urin\w*|blood|lab tests?|std|sti|hiv|sexual\w*|passwords?|passcodes?|"
    r"account numbers?|card numbers?|appts?|"
    r"(?:eye|skin|ear|foot|hearing|vision|lab|derm)\s+(?:appointments?|exams?|tests?))(?!\w)",
    re.IGNORECASE,
)
ACRONYMS = re.compile(r"\b(?:PIN|SIN)s?\b")  # in capitals only: "pin the board" is fine

_SIGN = "$¢£¤¥֏؋৲৳฿៛₠-⃀﷼"  # every currency sign, after NFKC
_CODE = r"(?:CAD|USD|EUR|GBP|AUD|NZD|JPY|CHF|CNY|RMB|INR|MXN|HKD|SGD|KRW|SEK|NOK|DKK|BTC)"
_NUM = r"\d(?:[\d,.' ]*\d)?(?:\s?[kKmM]\b)?"
_MONEY = (rf"(?:\b[A-Za-z]{{1,3}})?[{_SIGN}]\s?{_NUM}|{_NUM}\s?[{_SIGN}]|\b{_CODE}\s?{_NUM}|"
          rf"{_NUM}\s?(?:{_CODE}|dollars?|bucks|euros?|pounds?|quid|cents?|loonies|toonies)\b")
_DATE = r"(?<!\d)(?:(?:19|20)\d\d-(?:0[1-9]|1[0-2])(?:-(?:0[1-9]|[12]\d|3[01]))?|\d{1,2}[-/]\d{1,2}|\d{1,2}:\d{2})(?![\d/-])"
_GROUPED = r"(?:\+?\d{1,3}[ .-]?)?(?:\(\d{2,4}\)\s?|(?<!\d)\d{2,4}[ .-])\d{2,4}(?:[ .-]\d{2,5})+(?!\d)"
_TOKEN = r"(?<!\S)\S*\d\S*"
_DATE_RE = re.compile(_DATE)
MONEY_RE = re.compile(_MONEY, re.IGNORECASE)  # an amount of money in any notation; the sweep reads it too
_MASK = re.compile(rf"(?P<date>{_DATE})|(?P<money>{_MONEY})|(?P<grouped>{_GROUPED})|(?P<token>{_TOKEN})",
                   re.IGNORECASE)


def private_terms(root: Path) -> list[str] | None:
    """His private terms, lower case; None when the file is missing."""
    try:
        lines = (root / TERMS_FILE).read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    return [normalize(x.strip()).casefold() for x in lines if x.strip() and not x.lstrip().startswith("#")]


def is_private(texts: tuple[str, ...] | list[str], terms: list[str] | None) -> bool:
    """Whether any of these texts must stay off Discord: a sensitive category, or one of his terms."""
    for text in texts:
        if not text:
            continue
        t = normalize(text)
        if SENSITIVE.search(t) or ACRONYMS.search(t):
            return True
        folded = t.casefold()
        if any(term in folded for term in terms or ()):
            return True
    return False


def mask(text: str, amounts: bool) -> str:
    """The normalised text with numbers that could identify him masked as "####", and with
    amounts=True each amount of money as "$…". Dates and times stay."""
    def sub(m: re.Match) -> str:
        if m.group("date"):
            return m.group(0)
        if m.group("money"):
            return "$…" if amounts else m.group(0)
        if m.group("grouped"):
            return "####"
        token = m.group(0)
        if sum(c.isdigit() for c in token) < 4 or _DATE_RE.fullmatch(token.strip("([{<\"'.,;:!?)]}>")):
            return token
        tail = re.search(r"[,.;:!?)\]]*$", token).group(0)
        return "####" + tail
    return _MASK.sub(sub, normalize(text))
