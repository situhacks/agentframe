#!/usr/bin/env python3
"""Detect paragraph-shaped stacks of sibling <text> elements in deck SVGs.

The ppt-master SVG->PPTX converter merges a multi-line paragraph into one
editable PowerPoint text frame only when it is authored as ONE <text> with
dy-stacked <tspan> line children. A paragraph authored as N sibling <text>
elements (one per visual line) exports as N separate text boxes, which makes
the deck miserable to edit. Upstream has no lint for this (checked v3.1.0),
so this sidecar owns it — it never patches vendored files.

Heuristic (conservative, deny-side cost is high):
  flag a run of consecutive sibling <text> elements when ALL hold —
    - each has x/y, and no descendant tspan carries x/y/dy (inline runs ok)
    - identical effective style: font-size, font-family, fill, font-weight,
      text-anchor (element attr, falling back to ancestors)
    - same x within X_TOL
    - ascending y with gaps in [MIN_GAP_RATIO, MAX_GAP_RATIO] x font-size,
      uniform within GAP_TOL
    - prose test: at least PROSE_MIN_LINES lines are >= PROSE_MIN_CHARS
      visible characters (filters KPI/label stacks, which are short)

Usage:
    python system/hooks/svg_paragraph_lint.py <svg-file-or-dir> [...]
Exits 1 when findings exist. Also imported by ppt_master_guard.py.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from xml.etree import ElementTree as ET

SVG_NS = "http://www.w3.org/2000/svg"

X_TOL = 0.75            # px: same left edge
GAP_TOL = 2.0           # px: gap uniformity within a run
MIN_GAP_RATIO = 0.9     # gap >= 0.9 x font-size (tighter = overlap, not lines)
MAX_GAP_RATIO = 2.0     # gap <= 2.0 x font-size (wider = separate blocks)
PROSE_MIN_CHARS = 30    # a "prose" line is at least this long
PROSE_MIN_LINES = 2     # flag when this many prose lines stack

STYLE_ATTRS = ("font-size", "font-family", "fill", "font-weight", "text-anchor")
DEFAULT_FONT_SIZE = 16.0

_NUM = re.compile(r"[+-]?(?:\d+\.?\d*|\d*\.\d+)")


def _tag(el: ET.Element, name: str) -> bool:
    return el.tag == f"{{{SVG_NS}}}{name}"


def _num(value: str | None) -> float | None:
    if value is None:
        return None
    m = _NUM.search(value)
    return float(m.group(0)) if m else None


def _effective(el: ET.Element, attr: str, parents: dict) -> str | None:
    cur = el
    while cur is not None:
        v = cur.get(attr)
        if v is not None:
            return v
        cur = parents.get(cur)
    return None


def _has_positional_tspan(el: ET.Element) -> bool:
    for t in el.iter(f"{{{SVG_NS}}}tspan"):
        if any(t.get(k) is not None for k in ("x", "y", "dy")):
            return True
    return False


def _visible_text(el: ET.Element) -> str:
    return re.sub(r"\s+", " ", "".join(el.itertext())).strip()


class _Line:
    def __init__(self, el: ET.Element, parents: dict):
        self.x = _num(el.get("x"))
        self.y = _num(el.get("y"))
        self.text = _visible_text(el)
        self.style = tuple(_effective(el, a, parents) for a in STYLE_ATTRS)
        self.font_size = _num(_effective(el, "font-size", parents)) or DEFAULT_FONT_SIZE

    def stackable_after(self, prev: "_Line") -> bool:
        if self.style != prev.style:
            return False
        if abs(self.x - prev.x) > X_TOL:
            return False
        gap = self.y - prev.y
        return prev.font_size * MIN_GAP_RATIO <= gap <= prev.font_size * MAX_GAP_RATIO


def _flush(run: list[_Line], findings: list[str]) -> None:
    if len(run) < 2:
        return
    gaps = [b.y - a.y for a, b in zip(run, run[1:])]
    if max(gaps) - min(gaps) > GAP_TOL:
        return
    prose = [ln for ln in run if len(ln.text) >= PROSE_MIN_CHARS]
    if len(prose) < PROSE_MIN_LINES:
        return
    head = run[0].text[:40]
    findings.append(
        f"{len(run)} sibling <text> lines at x={run[0].x:g}, "
        f"y={run[0].y:g}-{run[-1].y:g} (\"{head}...\") look like one paragraph "
        f"split per line - author as ONE <text> with dy-stacked <tspan> lines "
        f"so it exports as a single editable text box"
    )


def check_svg_text(content: str, name: str | None = None) -> list[str]:
    """Return findings for one SVG document (empty list = clean)."""
    try:
        root = ET.fromstring(content)
    except ET.ParseError:
        return []

    parents = {c: p for p in root.iter() for c in p}
    findings: list[str] = []

    for parent in root.iter():
        run: list[_Line] = []
        for child in list(parent):
            if not _tag(child, "text"):
                continue  # layered decoration between lines doesn't break a run
            if _has_positional_tspan(child):
                _flush(run, findings)
                run = []
                continue
            line = _Line(child, parents)
            if line.x is None or line.y is None or not line.text:
                _flush(run, findings)
                run = []
                continue
            if run and line.stackable_after(run[-1]):
                run.append(line)
            else:
                _flush(run, findings)
                run = [line]
        _flush(run, findings)

    if name:
        findings = [f"{name}: {f}" for f in findings]
    return findings


def check_paths(paths: list[Path]) -> list[str]:
    findings: list[str] = []
    for path in paths:
        files = sorted(path.glob("*.svg")) if path.is_dir() else [path]
        for f in files:
            try:
                content = f.read_text(encoding="utf-8")
            except OSError:
                continue
            findings.extend(check_svg_text(content, name=f.name))
    return findings


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: svg_paragraph_lint.py <svg-file-or-dir> [...]", file=sys.stderr)
        return 2
    findings = check_paths([Path(a) for a in argv])
    for f in findings:
        print(f)
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
