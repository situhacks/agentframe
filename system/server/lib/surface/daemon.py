"""Start-or-open lifecycle for the local surface.

``start_or_open`` is the one entry point: if a healthy surface for THIS
workspace already answers on the remembered port, open/print its URL;
otherwise spawn a detached server, wait for health, and open it. The lock
file is advisory (pid/port/root/started_at) — the health endpoint is the
source of truth, so a stale lock never blocks a restart.
"""

from __future__ import annotations

import datetime
import json
import os
import socket
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

SERVER_DIR = Path(__file__).resolve().parents[2]
PROJECT_ROOT = SERVER_DIR.parents[1]
LOCK_PATH = SERVER_DIR / ".surface.lock"
LOG_PATH = SERVER_DIR / ".surface.log"
PORT_SCAN_RANGE = 20


def read_lock(lock_path: Path = LOCK_PATH) -> dict | None:
    try:
        data = json.loads(Path(lock_path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def write_lock(lock_path: Path = LOCK_PATH, *, port: int, root: str) -> dict:
    lock = {
        "pid": os.getpid(),
        "port": port,
        "root": str(root),
        "started_at": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
    }
    Path(lock_path).write_text(json.dumps(lock, indent=2), encoding="utf-8")
    return lock


def pick_port(preferred: int) -> int:
    """First bindable port from ``preferred`` upward."""
    for port in range(preferred, preferred + PORT_SCAN_RANGE):
        with socket.socket() as s:
            try:
                s.bind(("localhost", port))
                return port
            except OSError:
                continue
    raise RuntimeError(f"no free port in {preferred}..{preferred + PORT_SCAN_RANGE - 1}")


def is_healthy(port: int, *, expected_root: str, timeout: float = 6.0) -> bool:
    """True when /api/health answers AND serves this workspace.

    The timeout is generous because the livereload watcher's periodic glob
    scan can block the server's ioloop for ~3s; a short probe misreads a
    healthy-but-busy server as down and spawns a duplicate.
    """
    try:
        with urllib.request.urlopen(f"http://localhost:{port}/api/health", timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except (OSError, ValueError):
        return False
    got = str(data.get("workspace_root", "")).replace("\\", "/").rstrip("/").lower()
    want = str(expected_root).replace("\\", "/").rstrip("/").lower()
    return bool(data.get("ok")) and got == want


def start_or_open(*, preferred_port: int = 8080, open_browser: bool = True, root: Path = PROJECT_ROOT) -> dict:
    """Ensure a surface server is running; return {url, port, started}."""
    root = Path(root).resolve()

    lock = read_lock()
    candidates = []
    if lock and isinstance(lock.get("port"), int):
        candidates.append(lock["port"])
    if preferred_port not in candidates:
        candidates.append(preferred_port)
    for port in candidates:
        if is_healthy(port, expected_root=str(root)):
            url = f"http://localhost:{port}/"
            if open_browser:
                _open(url)
            return {"url": url, "port": port, "started": False}

    port = pick_port(preferred_port)
    run_py = SERVER_DIR / "run.py"
    creationflags = 0
    if os.name == "nt":
        creationflags = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    with open(LOG_PATH, "a", encoding="utf-8") as log:
        subprocess.Popen(
            [sys.executable, str(run_py), "--port", str(port), "--host", "localhost"],
            cwd=str(root),
            stdout=log,
            stderr=log,
            stdin=subprocess.DEVNULL,
            creationflags=creationflags,
            close_fds=True,
        )

    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if is_healthy(port, expected_root=str(root), timeout=2.0):
            break
        time.sleep(0.4)
    else:
        raise RuntimeError(f"surface server did not become healthy on port {port}; see {LOG_PATH}")

    url = f"http://localhost:{port}/"
    if open_browser:
        _open(url)
    return {"url": url, "port": port, "started": True}


def _open(url: str) -> None:
    import webbrowser

    try:
        webbrowser.open_new_tab(url)
    except Exception:
        pass
