"""The scheduled backlog pass: list the rows that can be done, for main to dispatch.

Asks `af backlog ready` for the oldest open rows no `Waits:` line holds back, and raises one loop
for the main chat naming them. It starts no builder and runs no git: permission rules cannot
contain an unattended builder, because the tests it must run execute code it can edit. Each row
goes to a builder main dispatches and a reviewer checks. An open loop from an earlier pass stands
until it is closed, so a second pass raises nothing.

    python system/tools/backlog_pass.py [--max-rows 3] [--dry-run]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
LOOP_PREFIX = "Dispatch backlog rows to a reviewed builder:"


def af(args: list[str], vault: Path, env) -> subprocess.CompletedProcess:
    return subprocess.run(["af", *args], cwd=vault, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")


def ready_rows(vault: Path, limit: int, env) -> list[dict]:
    r = af(["backlog", "ready", "--json", "--limit", str(limit)], vault, env)
    if r.returncode != 0:
        raise SystemExit(f"backlog_pass: af backlog ready failed: {r.stderr.strip()}")
    return json.loads(r.stdout or "[]")


def next_monday(today: dt.date) -> dt.date:
    days = (7 - today.weekday()) % 7 or 7
    return today + dt.timedelta(days=days)


def loop_text(rows: list[dict]) -> str:
    return f"{LOOP_PREFIX} " + "; ".join(f"{r['id']} {r['title']}" for r in rows)


def loop_is_open(vault: Path) -> bool:
    page = vault / "workspace" / "life" / "open.md"
    return page.is_file() and LOOP_PREFIX in page.read_text(encoding="utf-8", errors="replace")


def main(argv=None, vault: Path = ROOT, today: dt.date | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--max-rows", type=int, default=3)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args(argv)

    env = dict(os.environ, AF_ROOT=str(vault))
    rows = ready_rows(vault, args.max_rows, env)
    if not rows:
        print("backlog_pass: no row is ready; no loop raised")
        return 0
    text = loop_text(rows)
    if args.dry_run:
        print(f"backlog_pass: would raise: {text}")
        return 0
    if loop_is_open(vault):
        print("backlog_pass: the last pass's loop is still open; no loop raised")
        return 0
    nudge = next_monday(today or dt.date.today()).isoformat()
    r = af(["open", "add", text, "--nudge", nudge, "--from", "backlog-pass"], vault, env)
    if r.returncode != 0:
        print(f"backlog_pass: af open add failed: {r.stderr.strip()}", file=sys.stderr)
        return 1
    print(f"backlog_pass: raised a loop for {len(rows)} row(s), nudge {nudge}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
