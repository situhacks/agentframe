#!/usr/bin/env python3
"""Write the voice system to one file so a drafting turn loads it in one read.

`library/context/operator/voice/README.md` names a dozen files to load before
any user-voiced prose: identity, profile, anti-patterns, the register, the
matching pairs and template, and every corpus piece whole. Twelve reads get
skipped, especially after a compaction; one does not. This module concatenates
them in the README's order into `system/index/voice/` (a derived cache, never
truth) and says where it put them and how long they are.

    python system/af.py voice bundle <project> <deliverable>
    python system/af.py voice bundle --register informal --context long-form
    python system/af.py voice bundle --register informal --context email      # core by default
    python system/voice_bundle.py --register informal --context long-form --tier core

The head's `voice:` block supplies the base register and the borrow; the
deliverable `type:` picks the template and the default task context. The
corpus goes last, oldest to newest, so the newest exemplar sits closest to
the drafting.

Two tiers. `full` is the whole list and serves a first draft or anything
over a page. `core` drops the pairs and keeps one exemplar (the newest
corpus piece under CORE_EXEMPLAR_CAP bytes, else the smallest) and serves
a short write or a copyedit pass over operator-edited text, where the style
pass never runs. The tier defaults from the task context (TIER_BY_CONTEXT)
and the report prints the other tier's size so the agent can switch.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VOICE_DIR = os.path.join(ROOT, "library", "context", "operator", "voice")
OUT_DIR = os.path.join(ROOT, "system", "index", "voice")
READ_LIMIT = 2000  # the Read tool's default line window

REGISTERS = ("formal", "informal")
TIERS = ("core", "full")
CORE_EXEMPLAR_CAP = 20_000  # bytes; the one corpus piece a core bundle carries
TIER_BY_CONTEXT = {"email": "core", "short-form": "core"}  # every other context defaults to full
CONTEXT_PAIRS = {
    "long-form": ["plain-not-clever.md"],
    "short-form": ["plain-not-clever.md"],
    "email": ["plain-not-clever.md"],
    "builder-pov": ["builder-pov.md", "plain-not-clever.md"],
    "market-signal": ["market-signal.md"],
    "slide": ["slide-and-cover.md"],
    "cover": ["slide-and-cover.md"],
}
TYPE_TEMPLATE = {
    "substack-essay": "substack-essay.md",
    "linkedin-copy": "linkedin-body.md",
    "body-copy": "linkedin-body.md",
    "linkedin-post": "linkedin-body.md",
    "carousel": "carousel-arc.md",
    "carousel-copy": "carousel-arc.md",
}
TYPE_CONTEXT = {
    "substack-essay": "long-form",
    "linkedin-copy": "long-form",
    "body-copy": "long-form",
    "linkedin-post": "long-form",
    "carousel": "slide",
    "carousel-copy": "slide",
}
DENSITY_LINE = (
    "Please remove all mannered prose: when a literal phrase is available, use it. "
    "Delete the clause that props up a claim already made. One idea per paragraph."
)

FRONTMATTER = re.compile(r"\A---\r?\n(.*?)\r?\n---", re.S)


# ---------------------------------------------------------------- recipe

def _scalar(fm: str, key: str) -> str | None:
    m = re.search(rf"^\s*{re.escape(key)}:\s*(.+?)\s*$", fm, re.M)
    if not m:
        return None
    value = m.group(1).strip().strip("'\"")
    return value or None


def recipe_from_head(path: str) -> tuple[str | None, list[str], str | None]:
    """(base register, borrow registers, deliverable type) from a head's frontmatter."""
    text = open(path, encoding="utf-8-sig").read()
    m = FRONTMATTER.match(text.lstrip("﻿"))
    fm = m.group(1) if m else ""
    base = _scalar(fm, "base_register") or _scalar(fm, "register")
    if base and base not in REGISTERS:
        base = next((r for r in REGISTERS if r in base), None)  # "substack-informal" -> informal
    borrow: list[str] = []
    bm = re.search(r"^\s*borrow_from:\s*\[([^\]]*)\]", fm, re.M)
    if bm:
        borrow = [b.strip().strip("'\"") for b in bm.group(1).split(",") if b.strip()]
    return base, [b for b in borrow if b in REGISTERS], _scalar(fm, "type")


# ---------------------------------------------------------------- the bundle

def default_tier(contexts: list[str] | None) -> str:
    """core only when every task context asks for it; a long-form context anywhere means full."""
    tiers = {TIER_BY_CONTEXT.get(c, "full") for c in (contexts or ["long-form"])}
    return "core" if tiers == {"core"} else "full"


def _corpus_pieces(corpus_dir: str) -> list[str]:
    if not os.path.isdir(corpus_dir):
        return []
    return [os.path.join(corpus_dir, n) for n in sorted(os.listdir(corpus_dir))
            if n.endswith(".md") and n.lower() != "readme.md"]


def exemplar(corpus_dir: str, cap: int = CORE_EXEMPLAR_CAP) -> str | None:
    """The newest corpus piece at or under the cap, else the smallest piece."""
    pieces = _corpus_pieces(corpus_dir)
    if not pieces:
        return None
    small = [p for p in pieces if os.path.getsize(p) <= cap]
    return small[-1] if small else min(pieces, key=os.path.getsize)


def plan(register: str, contexts: list[str] | None = None, borrow: list[str] | None = None,
         template: str | None = None, voice_dir: str = VOICE_DIR, tier: str = "full") -> list[tuple[str, str]]:
    """The ordered (label, path) list the README prescribes. Existence is checked in build()."""
    contexts = contexts or ["long-form"]
    borrow = borrow or []
    if tier not in TIERS:
        raise ValueError(f"tier must be one of {TIERS}, not {tier!r}")
    parts = [
        ("procedure", os.path.join(voice_dir, "README.md")),
        ("identity", os.path.join(voice_dir, "identity.md")),
        ("profile", os.path.join(voice_dir, "voice-profile.md")),
        ("anti-patterns", os.path.join(voice_dir, "anti-patterns.md")),
        (f"register {register}", os.path.join(voice_dir, "registers", f"{register}.md")),
    ]
    for b in borrow:
        parts.append((f"borrow register {b}", os.path.join(voice_dir, "registers", f"{b}.md")))
    if template:
        parts.append((f"template {template}", os.path.join(voice_dir, "templates", template)))
    corpus_dir = os.path.join(voice_dir, "corpus", register)
    if tier == "core":
        piece = exemplar(corpus_dir)
        if piece:
            parts.append((f"exemplar {os.path.basename(piece)}", piece))
        return parts
    pair_files = [f"{register}.md"] + [f"{b}.md" for b in borrow]
    for c in contexts:
        pair_files += CONTEXT_PAIRS.get(c, [])
    seen: set[str] = set()
    for name in pair_files:
        if name in seen:
            continue
        seen.add(name)
        parts.append((f"pairs {name}", os.path.join(voice_dir, "pairs", name)))
    for piece in _corpus_pieces(corpus_dir):
        parts.append((f"corpus {os.path.basename(piece)}", piece))
    return parts


def _rel(path: str) -> str:
    return os.path.relpath(path, ROOT).replace("\\", "/")


def _read(path: str) -> str:
    return open(path, encoding="utf-8-sig").read().rstrip("\n")


def estimate(register: str, contexts: list[str] | None = None, borrow: list[str] | None = None,
             template: str | None = None, voice_dir: str = VOICE_DIR, tier: str = "full") -> tuple[int, int, int]:
    """(files, bytes, lines) a tier would bundle, without writing it."""
    present = [p for _, p in plan(register, contexts, borrow, template, voice_dir, tier) if os.path.isfile(p)]
    texts = [_read(p) for p in present]
    return len(present), sum(len(t.encode("utf-8")) for t in texts), sum(t.count("\n") + 3 for t in texts) + 4


def build(register: str, contexts: list[str] | None = None, borrow: list[str] | None = None,
          template: str | None = None, head: str | None = None, voice_dir: str = VOICE_DIR,
          out_dir: str = OUT_DIR, now: datetime.datetime | None = None, tier: str | None = None) -> dict:
    contexts = contexts or ["long-form"]
    borrow = borrow or []
    tier = tier or default_tier(contexts)
    parts = plan(register, contexts, borrow, template, voice_dir, tier)
    present = [(label, p) for label, p in parts if os.path.isfile(p)]
    missing = [(label, p) for label, p in parts if not os.path.isfile(p)]
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, f"{register}-{'+'.join(contexts)}{'-core' if tier == 'core' else ''}.md")

    chunks = []
    digest = hashlib.sha1(f"{tier}|{register}|{'+'.join(contexts)}|{','.join(borrow)}".encode("utf-8"))
    for i, (label, path) in enumerate(present, 1):
        body = _read(path)
        digest.update(f"\n{_rel(path)}\n{body}".encode("utf-8"))
        chunks.append(f"<!-- ===== voice bundle part {i}/{len(present)}: {label} ({_rel(path)}) ===== -->\n\n{body}\n")
    body_text = "\n\n".join(chunks)
    bundle_id = digest.hexdigest()[:8]
    approx_lines = body_text.count("\n") + 4
    stamp = (now or datetime.datetime.now()).strftime("%Y-%m-%d %H:%M")
    header = (
        f"<!-- VOICE BUNDLE id {bundle_id}, generated {stamp}. tier {tier}; register {register}; context {'+'.join(contexts)}; "
        f"borrow {', '.join(borrow) if borrow else 'none'}; head {_rel(head) if head else 'none'}. "
        f"{len(present)} files, about {approx_lines} lines. Read this file whole"
        + (f"; it is longer than the Read tool's {READ_LIMIT}-line window, so read it in two calls (offset {READ_LIMIT + 1})"
           if approx_lines > READ_LIMIT else "")
        + ". It stays loaded for the session: read it again only after a compaction, when the register changes, "
        "or before the back half of a draft over about 2,500 words. "
        "Derived cache under system/index/: never edit it; edit the files it names. -->\n"
        f"<!-- {DENSITY_LINE} -->\n\n"
    )
    text = header + body_text
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    other = "full" if tier == "core" else "core"
    return {
        "path": out,
        "id": bundle_id,
        "tier": tier,
        "register": register,
        "contexts": contexts,
        "borrow": borrow,
        "parts": [(label, _rel(p)) for label, p in present],
        "missing": [(label, _rel(p)) for label, p in missing],
        "bytes": len(text.encode("utf-8")),
        "lines": text.count("\n") + 1,
        "other_tier": (other,) + estimate(register, contexts, borrow, template, voice_dir, other),
    }


def format_report(result: dict) -> str:
    lines = [
        f"af voice bundle: {_rel(result['path'])} — {len(result['parts'])} files, "
        f"{result['bytes']:,} bytes, {result['lines']} lines (tier {result['tier']}, register {result['register']}, "
        f"context {'+'.join(result['contexts'])}"
        + (f", borrow {', '.join(result['borrow'])}" if result["borrow"] else "") + f"; id {result['id']})"
    ]
    for label, p in result["parts"]:
        lines.append(f"  {label:<36} {p}")
    for label, p in result["missing"]:
        lines.append(f"  MISSING {label:<28} {p}")
    lines.append("")
    if result["lines"] > READ_LIMIT:
        lines.append(f"Read it whole in two calls: the Read tool shows {READ_LIMIT} lines by default, so read again with offset {READ_LIMIT + 1}.")
    else:
        lines.append("Read it whole, in one call, before writing a line.")
    lines.append(f"Already holding bundle {result['id']} in context with no compaction since? It is current; skip the read. "
                 "Read again after a compaction, when the register changes, or before the back half of a draft over about 2,500 words.")
    other, files, nbytes, nlines = result["other_tier"]
    if other == "core":
        lines.append(f"For a short write (an email, a message, a paragraph, a note under a page) or a copyedit pass over his text, "
                     f"--tier core is {files} files, about {nlines} lines: the rules and one exemplar, no pairs.")
    else:
        lines.append(f"For a first draft or anything over a page, --tier full adds the pairs and the whole corpus: "
                     f"{files} files, about {nlines} lines.")
    lines.append(DENSITY_LINE)
    return "\n".join(lines)


# ---------------------------------------------------------------- CLI

def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="write the voice system to one file for a single read")
    ap.add_argument("--register", choices=REGISTERS, required=True)
    ap.add_argument("--context", action="append", choices=sorted(CONTEXT_PAIRS), help="task context (repeatable); default long-form")
    ap.add_argument("--borrow", action="append", choices=REGISTERS)
    ap.add_argument("--template", help="a file in templates/, e.g. substack-essay.md")
    ap.add_argument("--tier", choices=TIERS, help="core: rules + one exemplar for a short write or copyedit; full: everything. Default from the context")
    args = ap.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass
    print(format_report(build(args.register, args.context, args.borrow, args.template, tier=args.tier)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
