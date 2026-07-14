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


def _make_surface_handler(project_root: Path):
    """Serve the local-surface SPA shell (dashboard + preview) at `/`.

    Reads `static/surface/index.html` per request so agent edits show up on
    refresh without a server restart.
    """
    from tornado import web

    index_path = Path(__file__).resolve().parents[1] / "static" / "surface" / "index.html"

    class SurfaceHandler(web.RequestHandler):
        def get(self):
            self.set_header("Content-Type", "text/html; charset=utf-8")
            self.set_header("Cache-Control", "no-store")
            if index_path.is_file():
                self.write(index_path.read_text(encoding="utf-8"))
            else:
                self.write(
                    "<h1>AgentFrame Local</h1><p>Surface UI not built yet. "
                    'Legacy hub: <a href="/hub">/hub</a>. API: <a href="/api/health">/api/health</a></p>'
                )

    return SurfaceHandler


class _HubServer:
    """Wrap `livereload.Server` so we can inject app handlers ahead of statics.

    livereload's `get_web_handlers` returns the static-file route as a
    catch-all `/(.*)`. Prepending more-specific handlers serves the surface
    SPA at `/`, the legacy hub at `/hub`, and the JSON API at `/api/*` without
    disturbing static serving for everything else (projects, livereload.js).
    """

    def __init__(
        self,
        project_root: Path,
        *,
        exclude_globs: list[str] | None = None,
        workspace_root: Path | None = None,
    ):
        from livereload import Server

        self._project_root = Path(project_root)
        # Static assets always serve from the repo; workspace scanning may point
        # at a separate seed workspace (e.g. for demo screenshots).
        self._workspace_root = Path(workspace_root) if workspace_root else self._project_root
        self._inner = Server(watcher=watcher.make_watcher(self._workspace_root))
        self._patch_handlers(exclude_globs=exclude_globs)

    def _patch_handlers(self, *, exclude_globs: list[str] | None = None) -> None:
        original_get = self._inner.get_web_handlers
        hub_handler = _make_hub_handler(self._workspace_root, exclude_globs=exclude_globs)
        surface_handler = _make_surface_handler(self._project_root)

        from .surface import api as surface_api

        api_routes = surface_api.make_handlers(self._workspace_root)

        def patched(script):
            base = list(original_get(script))
            return [
                (r"/", surface_handler),
                (r"/index\.html", surface_handler),
                (r"/hub", hub_handler),
            ] + api_routes + base

        self._inner.get_web_handlers = patched  # type: ignore[method-assign]

    @property
    def watcher(self):
        return self._inner.watcher

    def watch(self, *args, **kwargs):
        return self._inner.watch(*args, **kwargs)

    def serve(self, **kwargs):
        try:
            return self._inner.serve(**kwargs)
        finally:
            close = getattr(self._inner.watcher, "close", None)
            if close:
                close()


def build_server(
    project_root: str | Path,
    watch_globs: Iterable[str],
    *,
    exclude_globs: Iterable[str] | None = None,
    delay: float = 0.5,
    workspace_root: str | Path | None = None,
):
    """Construct a hub-aware server with the given watch globs registered.

    ``workspace_root`` (default ``project_root``) is where projects/automations
    are scanned; static assets always serve from ``project_root``.

    Imported lazily so `--help` works without `livereload` installed.
    """
    server = _HubServer(
        Path(project_root),
        exclude_globs=list(exclude_globs or []),
        workspace_root=Path(workspace_root) if workspace_root else None,
    )
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
    workspace_root: str | Path | None = None,
) -> None:
    """Start the preview server in the foreground.

    Static files serve from ``project_root`` (the repo); ``workspace_root``
    (default ``project_root``) is the workspace scanned for projects.
    """
    globs = list(watch_globs) if watch_globs is not None else list(watcher.DEFAULT_GLOBS)
    server = build_server(
        project_root, globs, exclude_globs=exclude_globs, delay=delay, workspace_root=workspace_root
    )
    server.serve(
        root=str(project_root),
        port=port,
        host=host,
        open_url_delay=None,
    )


__all__ = ["build_server", "serve"]
