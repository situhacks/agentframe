"""Start-or-open lifecycle for the Workspace Dashboard.

``start_or_open`` reuses a healthy surface for this workspace or starts one
detached, then opens the requested view. ``stop`` terminates only a process
whose health endpoint confirms it serves this workspace. The lock file is
advisory (pid/port/root/started_at); health is the source of truth.
"""

from __future__ import annotations

import datetime
import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.parse
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


def read_health(port: int, *, timeout: float = 1.0) -> dict | None:
    """Return the health payload when the surface answers promptly."""
    try:
        with urllib.request.urlopen(f"http://localhost:{port}/api/health", timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def is_healthy(port: int, *, expected_root: str, timeout: float = 1.0) -> bool:
    """True when /api/health answers promptly AND serves this workspace."""
    data = read_health(port, timeout=timeout)
    if data is None:
        return False
    got = str(data.get("workspace_root", "")).replace("\\", "/").rstrip("/").lower()
    want = str(expected_root).replace("\\", "/").rstrip("/").lower()
    return bool(data.get("ok")) and got == want


def surface_url(
    port: int,
    *,
    view: str = "dashboard",
    project: str | None = None,
    file: str | None = None,
) -> str:
    """Build a dashboard, calendar, or artifact-preview URL."""
    if view not in {"dashboard", "calendar", "preview"}:
        raise ValueError(f"unknown surface view: {view}")
    params = {}
    if project:
        params["project"] = project
    if file:
        params["file"] = file
    query = f"?{urllib.parse.urlencode(params)}" if params else ""
    return f"http://localhost:{port}/#/{view}{query}"


def start_or_open(
    *,
    preferred_port: int = 8080,
    open_browser: bool = True,
    root: Path = PROJECT_ROOT,
    view: str = "dashboard",
    project: str | None = None,
    file: str | None = None,
) -> dict:
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
            url = surface_url(port, view=view, project=project, file=file)
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

    url = surface_url(port, view=view, project=project, file=file)
    if open_browser:
        _open(url)
    return {"url": url, "port": port, "started": True}


def stop(
    *,
    preferred_port: int = 8080,
    root: Path = PROJECT_ROOT,
    lock_path: Path = LOCK_PATH,
) -> dict:
    """Stop this workspace's healthy surface server, if one is running."""
    root = Path(root).resolve()
    lock = read_lock(lock_path)
    candidates = []
    if lock and isinstance(lock.get("port"), int):
        candidates.append(lock["port"])
    if preferred_port not in candidates:
        candidates.append(preferred_port)

    want = str(root).replace("\\", "/").rstrip("/").lower()
    for port in candidates:
        health = read_health(port)
        if not health or not health.get("ok"):
            continue
        got = str(health.get("workspace_root", "")).replace("\\", "/").rstrip("/").lower()
        pid = health.get("pid")
        if got != want or not isinstance(pid, int) or pid <= 0:
            continue

        os.kill(pid, signal.SIGTERM)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            current = read_health(port, timeout=0.3)
            if not current or current.get("pid") != pid:
                break
            time.sleep(0.1)
        else:
            raise RuntimeError(f"surface server pid {pid} did not stop")

        try:
            Path(lock_path).unlink(missing_ok=True)
        except OSError:
            pass
        return {"stopped": True, "port": port, "pid": pid}

    return {"stopped": False}


def _open(url: str) -> None:
    import webbrowser

    try:
        webbrowser.open_new_tab(url)
    except Exception:
        pass
