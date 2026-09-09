"""Read-only Board tab model: board.md joined to the live session roster.

The board file is owned by ``system/board.py`` (``af board``). This module only
reads: it parses the file leniently (grammar errors are reported, never
repaired), joins each card to its live session from ``claude agents --json``
(cached for a few seconds so the dashboard poll never spawns a CLI process per
request), attaches project names for the keycaps, and lists the recent archive.
"""

from __future__ import annotations

import datetime as dt
import sys
import time
from pathlib import Path

from . import state

try:
    from system import board as workboard
except ModuleNotFoundError:  # server started outside the repo root
    sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
    from system import board as workboard


ROSTER_TTL_SECONDS = 20.0  # above app.js POLL_MS (12s): the poll should never miss the cache
_roster_cache: dict[str, tuple[float, list[dict] | None]] = {}


def cached_roster(root: Path, *, ttl: float = ROSTER_TTL_SECONDS) -> list[dict] | None:
    key = str(Path(root).resolve())
    now = time.monotonic()
    hit = _roster_cache.get(key)
    if hit and now - hit[0] < ttl:
        return hit[1]
    rows = workboard.roster(root, timeout=6.0)
    _roster_cache[key] = (now, rows)
    return rows


def project_names(root: Path) -> dict[str, str]:
    names = {}
    for project in state.scan_projects(root, include_completed=True):
        slug = project.get("slug")
        if slug:
            names[slug] = project.get("name") or slug
    return names


def empty_model(now: dt.datetime) -> dict:
    return {
        "exists": False,
        "generated_at": now.isoformat(timespec="seconds"),
        "meta": dict(workboard.DEFAULT_META),
        "lanes": [{"name": lane, "cards": []} for lane in workboard.LANES],
        "open": 0,
        "waiting_on_you": 0,
        "roster_available": False,
        "errors": [],
        "issues": [],
        "projects": {},
        "archive": [],
        "init_hint": "python system/af.py board init",
    }


def build_model(root: Path, *, now: dt.datetime | None = None, rows: list[dict] | None = None,
                use_roster: bool = True) -> dict:
    root = Path(root)
    now = now or dt.datetime.now().astimezone()
    if not workboard.exists(root):
        return empty_model(now)
    text = workboard.paths(root)["board"].read_text(encoding="utf-8-sig")
    board, errors = workboard.parse(text)
    if rows is None and use_roster:
        rows = cached_roster(root)
    snap = workboard.snapshot(root, board, rows=rows, now=now)
    snap["exists"] = True
    snap["errors"] = errors
    snap["issues"] = workboard.issues(root, board, now=now, rows=rows)
    snap["projects"] = project_names(root)
    snap["archive"] = workboard.archive_cards(root, months=2, now=now)
    return snap
