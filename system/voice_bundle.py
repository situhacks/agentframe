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
    python system/voice_bundle.py --register informal --context long-form

The head's `voice:` block supplies the base register and the borrow; the
deliverable `type:` picks the template and the default task context. The
corpus goes last, oldest to newest, so the newest exemplar sits closest to
the drafting.
"""

from __future__ import annotations

import argparse
import datetime
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VOICE_DIR = os.path.join(ROOT, "library", "context", "operator", "voice")
OUT_DIR = os.path.join(ROOT, "system", "index", "voice")
READ_LIMIT = 2000  # the Read tool's default line window

REGISTERS = ("formal", "informal")
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

def plan(register: str, contexts: list[str] | None = None, borrow: list[str] | None = None,
         template: str | None = None, voice_dir: str = VOICE_DIR) -> list[tuple[str, str]]:
    """The ordered (label, path) list the README prescribes. Existence is checked in build()."""
    contexts = contexts or ["long-form"]
    borrow = borrow or []
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
    pair_files = [f"{register}.md"] + [f"{b}.md" for b in borrow]
    for c in contexts:
        pair_files += CONTEXT_PAIRS.get(c, [])
    seen: set[str] = set()
    for name in pair_files:
        if name in seen:
            continue
        seen.add(name)
        parts.append((f"pairs {name}", os.path.join(voice_dir, "pairs", name)))
    corpus_dir = os.path.join(voice_dir, "corpus", register)
    if os.path.isdir(corpus_dir):
        for name in sorted(os.listdir(corpus_dir)):
            if name.endswith(".md") and name.lower() != "readme.md":
                parts.append((f"corpus {name}", os.path.join(corpus_dir, name)))
    return parts


def _rel(path: str) -> str:
    return os.path.relpath(path, ROOT).replace("\\", "/")


def build(register: str, contexts: list[str] | None = None, borrow: list[str] | None = None,
          template: str | None = None, head: str | None = None, voice_dir: str = VOICE_DIR,
          out_dir: str = OUT_DIR, now: datetime.datetime | None = None) -> dict:
    contexts = contexts or ["long-form"]
    borrow = borrow or []
    parts = plan(register, contexts, borrow, template, voice_dir)
    present = [(label, p) for label, p in parts if os.path.isfile(p)]
    missing = [(label, p) for label, p in parts if not os.path.isfile(p)]
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, f"{register}-{'+'.join(contexts)}.md")

    chunks = []
    for i, (label, path) in enumerate(present, 1):
        body = open(path, encoding="utf-8-sig").read().rstrip("\n")
        chunks.append(f"<!-- ===== voice bundle part {i}/{len(present)}: {label} ({_rel(path)}) ===== -->\n\n{body}\n")
    body_text = "\n\n".join(chunks)
    approx_lines = body_text.count("\n") + 4
    stamp = (now or datetime.datetime.now()).strftime("%Y-%m-%d %H:%M")
    header = (
        f"<!-- VOICE BUNDLE generated {stamp}. register {register}; context {'+'.join(contexts)}; "
        f"borrow {', '.join(borrow) if borrow else 'none'}; head {_rel(head) if head else 'none'}. "
        f"{len(present)} files, about {approx_lines} lines. Read this file whole"
        + (f"; it is longer than the Read tool's {READ_LIMIT}-line window, so read it in two calls (offset {READ_LIMIT + 1})"
           if approx_lines > READ_LIMIT else "")
        + ". Re-run the bundle after any compaction. Derived cache under system/index/: never edit it; edit the files it names. -->\n"
        f"<!-- {DENSITY_LINE} -->\n\n"
    )
    text = header + body_text
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    return {
        "path": out,
        "register": register,
        "contexts": contexts,
        "borrow": borrow,
        "parts": [(label, _rel(p)) for label, p in present],
        "missing": [(label, _rel(p)) for label, p in missing],
        "bytes": len(text.encode("utf-8")),
        "lines": text.count("\n") + 1,
    }


def format_report(result: dict) -> str:
    lines = [
        f"af voice bundle: {_rel(result['path'])} — {len(result['parts'])} files, "
        f"{result['bytes']:,} bytes, {result['lines']} lines (register {result['register']}, "
        f"context {'+'.join(result['contexts'])}"
        + (f", borrow {', '.join(result['borrow'])}" if result["borrow"] else "") + ")"
    ]
    for label, p in result["parts"]:
        lines.append(f"  {label:<36} {p}")
    for label, p in result["missing"]:
        lines.append(f"  MISSING {label:<28} {p}")
    lines.append("")
    if result["lines"] > READ_LIMIT:
        lines.append(f"Read it whole in two calls: the Read tool shows {READ_LIMIT} lines by default, so read again with offset {READ_LIMIT + 1}.")
    else:
        lines.append("Read it whole, in one call, before writing a line; read it again after any compaction.")
    lines.append(DENSITY_LINE)
    return "\n".join(lines)


# ---------------------------------------------------------------- CLI

def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="write the voice system to one file for a single read")
    ap.add_argument("--register", choices=REGISTERS, required=True)
    ap.add_argument("--context", action="append", choices=sorted(CONTEXT_PAIRS), help="task context (repeatable); default long-form")
    ap.add_argument("--borrow", action="append", choices=REGISTERS)
    ap.add_argument("--template", help="a file in templates/, e.g. substack-essay.md")
    args = ap.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except (AttributeError, ValueError):
        pass
    print(format_report(build(args.register, args.context, args.borrow, args.template)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
