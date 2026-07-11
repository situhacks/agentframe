"""Watch-glob registration for the preview server.

Encapsulates the rule set for which files trigger a browser refresh, so
`run.py` stays declarative and the globs can be reused by tooling that
wants to know what the server cares about.
"""

from __future__ import annotations

from collections import deque
from inspect import signature
import logging
from pathlib import Path
import re
import threading
from typing import Iterable

import yaml

logger = logging.getLogger("livereload")

DEFAULT_GLOBS: tuple[str, ...] = (
    "workspace/projects/*/phase-*/**/*.html",
    "workspace/projects/*/phase-*/**/*.css",
    "workspace/projects/*/phase-3-planning/design-language/**",
    "workspace/projects/*/phase-4-production/posts/**/visuals/**",
    "workspace/projects/*/phase-4-production/posts/**/video/**",
    "workspace/projects/*/phase-4-production/posts/**/edit/**",
    "workspace/projects/*/phase-3-planning/design-language/preview/assets/tokens.css",
)
DEFAULT_EXCLUDE_GLOBS: tuple[str, ...] = ()


def load_globs(config_path: str | Path) -> list[str]:
    """Load watch globs from `system/server/config.yaml`.

    Falls back to DEFAULT_GLOBS if the file is missing or empty.
    """
    p = Path(config_path)
    if not p.exists():
        return list(DEFAULT_GLOBS)

    data = yaml.safe_load(p.read_text(encoding="utf-8")) or {}
    globs = data.get("watch_globs") or []
    if not globs:
        return list(DEFAULT_GLOBS)
    return [str(g) for g in globs]


def load_exclude_globs(config_path: str | Path) -> list[str]:
    """Load exclusion globs for hub discovery from `system/server/config.yaml`."""
    p = Path(config_path)
    if not p.exists():
        return list(DEFAULT_EXCLUDE_GLOBS)

    data = yaml.safe_load(p.read_text(encoding="utf-8")) or {}
    globs = data.get("exclude_globs") or []
    if not globs:
        return list(DEFAULT_EXCLUDE_GLOBS)
    return [str(g) for g in globs]


def _segment_regex(segment: str) -> str:
    """Translate one path segment without allowing ``*`` to cross ``/``."""
    out: list[str] = []
    for char in segment:
        if char == "*":
            out.append("[^/]*")
        elif char == "?":
            out.append("[^/]")
        else:
            out.append(re.escape(char))
    return "".join(out)


def _compile_glob(pattern: str) -> re.Pattern[str]:
    """Compile the small glob vocabulary used by preview watch patterns."""
    parts = pattern.replace("\\", "/").strip("/").split("/")
    chunks = ["^"]
    for index, part in enumerate(parts):
        last = index == len(parts) - 1
        if part == "**":
            chunks.append(".*" if last else "(?:[^/]+/)*")
            continue
        chunks.append(_segment_regex(part))
        if not last:
            chunks.append("/")
    chunks.append("$")
    return re.compile("".join(chunks), re.IGNORECASE)


class WatchdogWatcher:
    """Event-driven watcher compatible with livereload's watcher contract.

    ``livereload`` polls recursive globs on Tornado's request loop on Windows.
    This adapter lets watchdog own filesystem discovery in a background thread
    and schedules only the cheap reload callback on Tornado's loop.
    """

    def __init__(self, root: str | Path):
        self.root = Path(root).resolve()
        self._tasks: dict[str, dict] = {}
        self._changes: list[tuple[str, float | str | None]] = []
        self.filepath: str | None = None
        self._pending: deque[tuple[str, tuple[str, ...]]] = deque()
        self._pending_paths: set[str] = set()
        self._lock = threading.Lock()
        self._callback = None
        self._callback_pending = False
        self._loop = None
        self._observer = None

    def watch(self, path, func=None, delay=0, ignore=None) -> None:
        key = str(path)
        pattern = self._relative_pattern(key)
        self._tasks[key] = {
            "func": func,
            "delay": delay,
            "ignore": ignore,
            "pattern": pattern,
            "regex": _compile_glob(pattern),
        }

    def _relative_pattern(self, pattern: str) -> str:
        normalized = pattern.replace("\\", "/")
        candidate = Path(pattern)
        if candidate.is_absolute():
            try:
                normalized = candidate.resolve().relative_to(self.root).as_posix()
            except ValueError:
                normalized = candidate.resolve().as_posix()
        return normalized.strip("/")

    def _watch_bases(self) -> list[Path]:
        bases: list[Path] = []
        for item in self._tasks.values():
            stable: list[str] = []
            for part in item["pattern"].split("/"):
                if any(mark in part for mark in ("*", "?", "[")):
                    break
                stable.append(part)
            candidate = self.root.joinpath(*stable) if stable else self.root
            if candidate.is_file():
                candidate = candidate.parent
            while not candidate.exists() and candidate != self.root:
                candidate = candidate.parent
            if candidate != self.root and self.root not in candidate.parents:
                candidate = self.root
            if any(candidate == base or base in candidate.parents for base in bases):
                continue
            bases = [base for base in bases if candidate not in base.parents]
            bases.append(candidate)
        return bases or [self.root]

    def start(self, callback) -> bool:
        from tornado.ioloop import IOLoop
        from watchdog.events import FileSystemEventHandler
        from watchdog.observers import Observer

        owner = self

        class Handler(FileSystemEventHandler):
            def on_any_event(self, event):
                if event.is_directory or event.event_type not in {"created", "modified", "deleted", "moved"}:
                    return
                owner._queue_changed_path(event.src_path)
                dest = getattr(event, "dest_path", None)
                if dest:
                    owner._queue_changed_path(dest)

        self._callback = callback
        self._loop = IOLoop.current()
        self._observer = Observer()
        handler = Handler()
        for base in self._watch_bases():
            self._observer.schedule(handler, str(base), recursive=True)
        self._observer.start()
        callback()  # consume livereload's initial ``__livereload__`` change
        return True

    def _queue_changed_path(self, changed_path: str | Path) -> None:
        try:
            rel = Path(changed_path).resolve().relative_to(self.root).as_posix()
        except ValueError:
            return
        matches = tuple(
            key
            for key, item in self._tasks.items()
            if item["regex"].match(rel)
            and not (item["ignore"] and item["ignore"](str(changed_path)))
        )
        if not matches:
            return
        should_schedule = False
        with self._lock:
            if rel not in self._pending_paths:
                self._pending.append((rel, matches))
                self._pending_paths.add(rel)
            if self._callback and self._loop and not self._callback_pending:
                self._callback_pending = True
                should_schedule = True
        if should_schedule:
            self._loop.add_callback(self._schedule_dispatch)

    def _schedule_dispatch(self) -> None:
        # A single save can emit several adjacent OS events. Let them settle so
        # livereload receives one logical change instead of several reloads.
        self._loop.call_later(0.05, self._dispatch)

    def _dispatch(self) -> None:
        if self._callback:
            self._callback()
        reschedule = False
        with self._lock:
            self._callback_pending = False
            if self._pending:
                self._callback_pending = True
                reschedule = True
        if reschedule:
            self._schedule_dispatch()

    def examine(self):
        if self._changes:
            return self._changes.pop()
        with self._lock:
            pending = list(self._pending)
            self._pending.clear()
            self._pending_paths.clear()
        if not pending:
            return None, None

        paths_by_task: dict[str, list[str]] = {}
        for path, keys in pending:
            for key in keys:
                paths_by_task.setdefault(key, []).append(path)

        delays: list[float] = []
        forever = False
        for key, paths in paths_by_task.items():
            item = self._tasks[key]
            func = item["func"]
            if func:
                if len(signature(func).parameters) > 0:
                    func(paths)
                else:
                    func()
            delay = item["delay"]
            if delay == "forever":
                forever = True
            elif isinstance(delay, (int, float)) and delay > 0:
                delays.append(float(delay))

        self.filepath = pending[-1][0]
        if delays:
            return self.filepath, max(delays)
        return self.filepath, "forever" if forever else None

    def close(self) -> None:
        if self._observer is not None:
            self._observer.stop()
            self._observer.join(timeout=2)
            self._observer = None


class StaticWatcher:
    """Livereload-compatible no-op used when watchdog is unavailable."""

    def __init__(self):
        self._tasks: dict[str, dict] = {}
        self._changes: list[tuple[str, float | str | None]] = []
        self.filepath = None

    def watch(self, path, func=None, delay=0, ignore=None) -> None:
        self._tasks[str(path)] = {"func": func, "delay": delay, "ignore": ignore}

    def start(self, callback) -> bool:
        callback()
        return True

    def examine(self):
        return self._changes.pop() if self._changes else (None, None)


def make_watcher(root: str | Path):
    """Use watchdog when installed; keep serving without watches otherwise."""
    try:
        import watchdog  # noqa: F401
    except ImportError:
        logger.warning("watchdog unavailable; automatic browser refresh is disabled")
        return StaticWatcher()
    return WatchdogWatcher(root)


def register(server, globs: Iterable[str], *, delay: float = 0.5) -> None:
    """Register each glob with a `livereload.Server` instance."""
    for g in globs:
        server.watch(g, delay=delay)


__all__ = [
    "DEFAULT_GLOBS",
    "DEFAULT_EXCLUDE_GLOBS",
    "StaticWatcher",
    "WatchdogWatcher",
    "load_globs",
    "load_exclude_globs",
    "make_watcher",
    "register",
]
