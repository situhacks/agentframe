#!/usr/bin/env python3
"""Keep the retrieval index fresh without anyone remembering to.

Session start: when ``system/index/vault.db`` exists and its last update is
older than ``REFRESH_AFTER_HOURS``, spawn ``af index update`` detached and
return at once. Never blocks the session, never fails it, never builds from
cold: a first build takes tens of minutes with embeddings and stays an explicit
``af index update --rebuild``. A lock file stops two sessions that start within
minutes of each other from running two updates.
"""

from __future__ import annotations

import argparse
import datetime
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DB = ROOT / "system" / "index" / "vault.db"
LOCK = ROOT / "system" / "index" / "refresh.lock"
LOG = ROOT / "system" / "index" / "refresh.log"
REFRESH_AFTER_HOURS = 24
LOCK_STALE_HOURS = 2

# The child removes the lock when the update ends, so a finished refresh never
# blocks the next one; a crashed child leaves a lock that goes stale on its own.
_SHIM = ("import os, subprocess, sys; "
         "rc = subprocess.run([sys.executable] + sys.argv[1:-1]).returncode; "
         "os.path.exists(sys.argv[-1]) and os.remove(sys.argv[-1]); "
         "sys.exit(rc)")


def last_update(db: Path) -> datetime.datetime | None:
    con = sqlite3.connect(f"file:{db.as_posix()}?mode=ro", uri=True)
    try:
        row = con.execute("SELECT value FROM meta WHERE key='updated_at'").fetchone()
    finally:
        con.close()
    if not row:
        return None
    try:
        return datetime.datetime.fromisoformat(str(row[0]))
    except ValueError:
        return None


def decide(now: datetime.datetime | None = None) -> str:
    """'spawn' when a refresh is due, otherwise the reason it is not."""
    now = now or datetime.datetime.now()
    if not DB.is_file():
        return "no index yet (af index update --rebuild builds the first one)"
    if LOCK.is_file():
        lock_age_h = (now.timestamp() - LOCK.stat().st_mtime) / 3600
        if lock_age_h < LOCK_STALE_HOURS:
            return "a refresh is already running"
    updated = last_update(DB)
    if updated is not None:
        if updated.tzinfo is not None:
            updated = updated.astimezone().replace(tzinfo=None)
        age_h = (now - updated).total_seconds() / 3600
        if age_h < REFRESH_AFTER_HOURS:
            return f"fresh ({age_h:.0f}h old)"
    return "spawn"


def spawn() -> None:
    LOCK.parent.mkdir(parents=True, exist_ok=True)
    LOCK.write_text(f"{os.getpid()} {datetime.datetime.now().isoformat(timespec='seconds')}\n",
                    encoding="utf-8")
    kwargs = dict(stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, cwd=str(ROOT), close_fds=True)
    if sys.platform == "win32":
        kwargs["creationflags"] = (subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
                                   | subprocess.CREATE_NO_WINDOW)
    else:
        kwargs["start_new_session"] = True
    with open(LOG, "ab") as log:  # the child inherits its own handle; the parent's closes here
        subprocess.Popen([sys.executable, "-c", _SHIM, str(ROOT / "system" / "af.py"),
                          "index", "update", str(LOCK)], stdout=log, **kwargs)


def main() -> int:
    ap = argparse.ArgumentParser(description="AgentFrame index refresh hook")
    ap.add_argument("--harness", default="claude")
    ap.add_argument("--cursor-native", action="store_true")
    ap.parse_args()
    try:
        if decide() == "spawn":
            spawn()
            print("AgentFrame index: last refresh over a day old; refreshing in the background "
                  "(python system/af.py index status).")
    except Exception:  # a hook never fails the session
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
