"""Thin wrapper around `livereload.Server` for the preview server.

Keeps `run.py` short and lets tests import a configured server without
actually starting it.

Adds a custom Tornado handler at `/` and `/index.html` that renders the
preview hub (sidebar of all project previews + iframe canvas). Without
this, the project root has no `index.html` and `/` returns 404.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterable

from . import watcher


def _make_hub_handler(project_root: Path, *, exclude_globs: list[str] | None = None):
    """Build a Tornado RequestHandler bound to a specific project_root.

    Returns the handler class (Tornado expects a class in the route table).
    """
    from tornado import web

    from . import hub as hub_module

    root = Path(project_root)
    hub_exclude_globs = list(exclude_globs or [])

    class HubHandler(web.RequestHandler):
        def get(self):
            include_intermediates = self.get_argument("intermediates", "0").lower() in (
                "1",
                "true",
                "yes",
                "on",
            )
            model = hub_module.scan_workspace(
                root,
                exclude_globs=hub_exclude_globs,
                include_intermediates=include_intermediates,
            )
            html = hub_module.render_hub_html(model)
            self.set_header("Content-Type", "text/html; charset=utf-8")
            self.set_header("Cache-Control", "no-store")
            self.write(html)

    return HubHandler


class _HubServer:
    """Wrap `livereload.Server` so we can inject a hub handler at `/`.

    livereload's `get_web_handlers` returns the static-file route as a
    catch-all `/(.*)`. Prepending a more-specific handler for `/` and
    `/index.html` lets us serve the dashboard without disturbing static
    serving for everything else (projects, demo, livereload.js, etc.).
    """

    def __init__(self, project_root: Path, *, exclude_globs: list[str] | None = None):
        from livereload import Server

        self._project_root = Path(project_root)
        self._inner = Server()
        self._patch_handlers(exclude_globs=exclude_globs)

    def _patch_handlers(self, *, exclude_globs: list[str] | None = None) -> None:
        original_get = self._inner.get_web_handlers
        hub_handler = _make_hub_handler(self._project_root, exclude_globs=exclude_globs)

        def patched(script):
            base = list(original_get(script))
            return [
                (r"/", hub_handler),
                (r"/index\.html", hub_handler),
            ] + base

        self._inner.get_web_handlers = patched  # type: ignore[method-assign]

    @property
    def watcher(self):
        return self._inner.watcher

    def watch(self, *args, **kwargs):
        return self._inner.watch(*args, **kwargs)

    def serve(self, **kwargs):
        return self._inner.serve(**kwargs)


def build_server(
    project_root: str | Path,
    watch_globs: Iterable[str],
    *,
    exclude_globs: Iterable[str] | None = None,
    delay: float = 0.5,
):
    """Construct a hub-aware server with the given watch globs registered.

    Imported lazily so `--help` works without `livereload` installed.
    """
    server = _HubServer(Path(project_root), exclude_globs=list(exclude_globs or []))
    watcher.register(server, watch_globs, delay=delay)
    return server


def serve(
    project_root: str | Path,
    *,
    port: int = 8080,
    host: str = "localhost",
    watch_globs: Iterable[str] | None = None,
    exclude_globs: Iterable[str] | None = None,
    delay: float = 0.5,
) -> None:
    """Start the preview server in the foreground."""
    globs = list(watch_globs) if watch_globs is not None else list(watcher.DEFAULT_GLOBS)
    server = build_server(project_root, globs, exclude_globs=exclude_globs, delay=delay)
    server.serve(
        root=str(project_root),
        port=port,
        host=host,
        open_url_delay=None,
    )


__all__ = ["build_server", "serve"]
