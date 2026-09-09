#!/usr/bin/env python3
"""Deterministic backstop for the zero-budget voice rules.

`library/context/operator/voice/anti-patterns.md` carries the judgment. This lint
carries only what a regex can settle and what a diff can prove, so a late agent
pass cannot ship them unnoticed:

  hard (blocks `af ready` through system/hooks/voice_guard.py):
    banned-tic          "my read" / "my honest read", "quietly"
    double-dash         two em dashes in one sentence
    hyphen-normalised   a sentence the previous version wrote with the operator's
                        spaced hyphen ( - ) now carries an em dash instead

  soft (reported, never blocking):
    dash-consecutive    an em dash in each of two consecutive sentences
    dash-rate           em dashes per 1,000 words above the drift alarm
    hyphen-shift        spaced hyphens fell and em dashes rose since the previous version
    thread-as-topic     "thread" for a subject rather than a literal thread
    litotes             the negated shapes anti-patterns.md names
    contrastive         more than one "isn't X, it's Y" pivot in the piece

Cadence numbers are deliberately absent: a threshold on sentence length only
teaches the agent to aim at the number (AGENTS.builder.md, design principle 7).

A deliverable is user-voiced when its frontmatter declares `voice:` or
`register:`; anything else is reported as skipped and exits 0.

    python system/voice_lint.py <head.md> [--prev auto|none|<file>] [--json]
    python system/voice_lint.py --project <slug> --deliverable <row-or-path>
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

DASH_RATE_ALARM = 3.0  # per 1,000 words; anti-patterns.md § Punctuation
SPACED_HYPHEN = " - "

FRONTMATTER = re.compile(r"\A---\r?\n(.*?)\r?\n---\r?\n?", re.S)
USER_VOICED = re.compile(r"^(?:voice|register):", re.M)
DASH = re.compile(r"—|(?<=\s)–(?=\s)")  # an em dash, or a spaced en dash doing its job
QUOTED = re.compile(r'"[^"\n]{1,400}"')
SENTENCE_SPLIT = re.compile(
    r"(?<=[.!?])\s+(?=[\"'(\[A-Z0-9*])|(?<=[.!?][\"')\]])\s+(?=[\"'(\[A-Z0-9*])"
)
BANNED_TICS = (
    ("my read", re.compile(r"\bmy (?:honest )?read\b", re.I)),
    ("quietly", re.compile(r"\bquietly\b", re.I)),
)
LITOTES = re.compile(
    r"\b(?:not un\w+|not im(?:possible|probable)\w*|not without\b|no small\b|"
    r"no stranger to\b|no accident\b|hardly (?:surprising|a surprise|new)\b|"
    r"scarcely\b|not (?:bad|terrible|the worst)\b)",
    re.I,
)
CONTRASTIVE = re.compile(
    r"\b(?:isn't|is not|wasn't|was not|aren't|are not)\b[^.!?;]{1,60}?(?:, it's|; it's|, it is)\b",
    re.I,
)
THREAD = re.compile(
    r"(?<!\bemail )(?<!\bcomment )(?<!\bX )(?<!\bTwitter )(?<!\bSlack )(?<!\bReddit )"
    r"(?<!\bforum )(?<!\bDiscord )(?<!\bmessage )\bthreads?\b(?! the needle)",
    re.I,
)


# ---------------------------------------------------------------- text shaping

def split_frontmatter(text: str) -> tuple[str, str]:
    """(frontmatter, body); frontmatter is '' when the file has none."""
    text = text.lstrip("﻿")
    m = FRONTMATTER.match(text)
    if not m:
        return "", text
    return m.group(1), text[m.end():]


def is_user_voiced(frontmatter: str) -> bool:
    return bool(USER_VOICED.search(frontmatter))


def _blank_keeping_lines(match: re.Match) -> str:
    return "\n" * match.group(0).count("\n")


def prose(body: str) -> str:
    """Body with comments, code, images, tables and rules blanked; line count preserved."""
    text = body.replace("\r\n", "\n")
    text = re.sub(r"<!--.*?-->", _blank_keeping_lines, text, flags=re.S)
    text = re.sub(r"```.*?```", _blank_keeping_lines, text, flags=re.S)
    out = []
    for line in text.split("\n"):
        s = line.strip()
        if s.startswith(("![", "|")) or re.fullmatch(r"-{3,}|\*{3,}|_{3,}", s):
            out.append("")
        else:
            out.append(line)
    return "\n".join(out)


def sentences(text: str) -> list[tuple[str, int]]:
    """(sentence, 1-based line within `text`), paragraph by paragraph."""
    out = []
    for para in re.finditer(r"(?:[^\n]+\n?)+", text):
        block = para.group(0)
        if not block.strip():
            continue
        block_line = text[: para.start()].count("\n") + 1
        flat = block.replace("\n", " ").strip()
        cursor = 0
        for part in SENTENCE_SPLIT.split(flat):
            part = part.strip()
            if not part:
                continue
            pos = flat.find(part, cursor)
            cursor = pos + len(part) if pos >= 0 else cursor
            line = block_line + block[:pos].count("\n") if pos >= 0 else block_line
            out.append((part, line))
    return out


def _norm(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


def _snippet(text: str, start: int, end: int, width: int = 60) -> str:
    lo, hi = max(0, start - width), min(len(text), end + width)
    return ("…" if lo > 0 else "") + text[lo:hi].replace("\n", " ").strip() + ("…" if hi < len(text) else "")


def _inside(spans: list[tuple[int, int]], pos: int) -> bool:
    return any(a <= pos < b for a, b in spans)


# ---------------------------------------------------------------- the lint

def lint(text: str, prev_text: str | None = None) -> dict:
    fm, body = split_frontmatter(text)
    result: dict = {"user_voiced": is_user_voiced(fm), "hard": [], "soft": [], "stats": {}}
    if not result["user_voiced"]:
        return result

    offset = text.lstrip("﻿")[: len(text.lstrip("﻿")) - len(body)].count("\n")
    p = prose(body)
    quoted = [m.span() for m in QUOTED.finditer(p)]

    def line_of(pos: int) -> int:
        return offset + p[:pos].count("\n") + 1

    def add(level: str, code: str, line: int, message: str, snippet: str = "") -> None:
        result[level].append({"code": code, "line": line, "message": message, "snippet": snippet})

    # banned tics (outside quotations, which may legitimately carry the word)
    for name, rx in BANNED_TICS:
        for m in rx.finditer(p):
            if _inside(quoted, m.start()):
                continue
            add("hard", "banned-tic", line_of(m.start()), f'"{name}" is a banned tic (anti-patterns.md § Banned tics)', _snippet(p, m.start(), m.end()))

    # dashes, sentence by sentence
    sents = sentences(p)
    prev_had_dash = False
    for sent, line in sents:
        n = len(DASH.findall(sent))
        if n >= 2:
            add("hard", "double-dash", offset + line, f"{n} em dashes in one sentence (always wrong: commas or parentheses own the aside)", _snippet(sent, 0, len(sent), 90))
        if n and prev_had_dash:
            add("soft", "dash-consecutive", offset + line, "em dash in each of two consecutive sentences", _snippet(sent, 0, len(sent), 90))
        prev_had_dash = bool(n)

    # soft word-level shapes
    for m in THREAD.finditer(p):
        if not _inside(quoted, m.start()):
            add("soft", "thread-as-topic", line_of(m.start()), '"thread" for a subject; name the subject instead (literal threads are fine)', _snippet(p, m.start(), m.end()))
    for m in LITOTES.finditer(p):
        if not _inside(quoted, m.start()):
            add("soft", "litotes", line_of(m.start()), "litotes shape; write the affirmative or hedge openly", _snippet(p, m.start(), m.end()))
    pivots = [m for m in CONTRASTIVE.finditer(p) if not _inside(quoted, m.start())]
    if len(pivots) > 1:
        for m in pivots[1:]:
            add("soft", "contrastive", line_of(m.start()), f"contrastive negation spent {len(pivots)}x; budget is one per piece", _snippet(p, m.start(), m.end()))

    # rates
    words = len(p.split())
    dashes = len(DASH.findall(p))
    hyphens = p.count(SPACED_HYPHEN)
    rate = (dashes * 1000.0 / words) if words else 0.0
    result["stats"] = {"words": words, "em_dashes": dashes, "dash_rate_per_1k": round(rate, 2), "spaced_hyphens": hyphens}
    if rate > DASH_RATE_ALARM:
        add("soft", "dash-rate", 0, f"{rate:.1f} em dashes per 1,000 words (drift alarm {DASH_RATE_ALARM:.0f}); fuse or repunctuate, never split into periods")

    # against the previous version: the operator's hyphen must survive a cleanup
    if prev_text is not None:
        _, prev_body = split_frontmatter(prev_text)
        pp = prose(prev_body)
        head_norm = {_norm(s) for s, _ in sents}
        for sent, _line in sentences(pp):
            if SPACED_HYPHEN not in sent or _norm(sent) in head_norm:
                continue
            swapped = (
                sent.replace(SPACED_HYPHEN, " — "),
                sent.replace(SPACED_HYPHEN, "—"),
                sent.replace(SPACED_HYPHEN, " – "),
            )
            hit = next((_norm(v) for v in swapped if _norm(v) in head_norm), None)
            if hit:
                line = next((l for s, l in sents if _norm(s) == hit), 0)
                add("hard", "hyphen-normalised", offset + line, "the previous version's spaced hyphen became an em dash; the operator's punctuation is not a typo (anti-patterns.md § Punctuation)", _snippet(hit, 0, len(hit), 90))
        prev_hyphens = pp.count(SPACED_HYPHEN)
        prev_dashes = len(DASH.findall(pp))
        if hyphens < prev_hyphens and dashes > prev_dashes:
            add("soft", "hyphen-shift", 0, f"spaced hyphens {prev_hyphens} -> {hyphens} while em dashes {prev_dashes} -> {dashes} since the previous version")
        result["stats"].update({"prev_em_dashes": prev_dashes, "prev_spaced_hyphens": prev_hyphens})

    return result


# ---------------------------------------------------------------- files

def previous_version(path: str) -> str | None:
    """`{name}-v{N-1}.md` beside `{name}-v{N}.md`, when N > 1 and it exists."""
    folder, base = os.path.split(path)
    m = re.fullmatch(r"(.+)-v(\d+)\.md", base)
    if not m or int(m.group(2)) <= 1:
        return None
    candidate = os.path.join(folder, f"{m.group(1)}-v{int(m.group(2)) - 1}.md")
    return candidate if os.path.isfile(candidate) else None


def _read(path: str) -> str:
    with open(path, "r", encoding="utf-8-sig") as fh:
        return fh.read()


def lint_file(path: str, prev: str | None = "auto") -> dict:
    """Lint one head. `prev` is 'auto' (the sibling version), None, or a path."""
    prev_path = previous_version(path) if prev == "auto" else (prev or None)
    prev_text = _read(prev_path) if prev_path and os.path.isfile(prev_path) else None
    result = lint(_read(path), prev_text)
    result["path"] = path
    result["prev"] = prev_path if prev_text is not None else None
    return result


def resolve_head(project: str, deliverable: str) -> str | None:
    """The tracked head for a project row or project-relative path, via af's resolver."""
    import contextlib
    import io

    sys.path.insert(0, ROOT)
    from system import af  # noqa: WPS433

    try:
        with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
            cdir = af.project_dir(project)
            sdoc = af.state_doc(cdir)
            fm, _ = af.split_fm(af.read(os.path.join(cdir, sdoc)), sdoc)
            _, rel = af.resolve_deliverable_target(fm, deliverable)
    except SystemExit:
        return None
    head = os.path.join(cdir, rel)
    return head if os.path.isfile(head) else None


def format_report(result: dict, label: str | None = None) -> str:
    path = label or result.get("path", "<text>")
    rel = os.path.relpath(path, ROOT) if os.path.isabs(path) and path.startswith(ROOT) else path
    if not result["user_voiced"]:
        return f"voice lint: {rel} is not user-voiced (no voice:/register: in frontmatter); skipped"
    prev = result.get("prev")
    head = f"voice lint: {rel}" + (f" vs {os.path.basename(prev)}" if prev else " (no previous version to diff)")
    lines = [head]
    if result["hard"]:
        lines.append(f"  HARD ({len(result['hard'])}, blocks af ready):")
        for f in result["hard"]:
            lines.append(f"    {f['code']:<18} L{f['line']}: {f['message']}")
            if f["snippet"]:
                lines.append(f"      > {f['snippet']}")
    if result["soft"]:
        lines.append(f"  soft ({len(result['soft'])}, judge them):")
        for f in result["soft"]:
            where = f"L{f['line']}: " if f["line"] else ""
            lines.append(f"    {f['code']:<18} {where}{f['message']}")
            if f["snippet"]:
                lines.append(f"      > {f['snippet']}")
    s = result["stats"]
    lines.append(f"  stats: {s.get('words', 0)} words, {s.get('em_dashes', 0)} em dashes ({s.get('dash_rate_per_1k', 0)}/1k), {s.get('spaced_hyphens', 0)} spaced hyphens")
    if not result["hard"]:
        lines.append("  ok: no hard findings")
    return "\n".join(lines)


# ---------------------------------------------------------------- CLI

def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="AgentFrame voice lint (zero-budget rules and the hyphen alarm)")
    ap.add_argument("file", nargs="?", help="the head to lint")
    ap.add_argument("--project", help="resolve the head through the project tracker instead of a path")
    ap.add_argument("--deliverable", help="tracker row slug or project-relative path (with --project)")
    ap.add_argument("--prev", default="auto", help="'auto' (sibling v{N-1}), 'none', or a file")
    ap.add_argument("--json", action="store_true", help="machine-readable result")
    args = ap.parse_args(argv)

    if args.project:
        if not args.deliverable:
            ap.error("--deliverable is required with --project")
        path = resolve_head(args.project, args.deliverable)
        if not path:
            print(f"voice lint: could not resolve {args.project} / {args.deliverable}", file=sys.stderr)
            return 2
    elif args.file:
        path = args.file
    else:
        ap.error("give a file, or --project with --deliverable")

    prev = "auto" if args.prev == "auto" else (None if args.prev == "none" else args.prev)
    result = lint_file(path, prev)
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=1))
    else:
        print(format_report(result))
    return 1 if result["hard"] else 0


if __name__ == "__main__":
    sys.exit(main())
