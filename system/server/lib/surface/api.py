"""Tornado JSON handlers for the local surface.

Mounted by ``system/server/lib/server.py`` in front of livereload's static
route. Everything reads through the tested ``state``/``artifacts``/``snapshot``
modules; handlers own HTTP shape only.
"""

from __future__ import annotations

import asyncio
import datetime
import json
import os
import subprocess
import urllib.parse
from pathlib import Path

from tornado import web

from . import artifacts, convert, snapshot, state

CACHE_DIR = Path(__file__).resolve().parents[2] / ".cache" / "convert"

STARTED_AT = datetime.datetime.now().astimezone().isoformat(timespec="seconds")

MEDIA_TYPES = {
    ".md": "markdown",
    ".txt": "text",
    ".html": "html",
    ".png": "image",
    ".jpg": "image",
    ".jpeg": "image",
    ".webp": "image",
    ".gif": "image",
    ".svg": "image",
    ".pdf": "pdf",
    ".mp4": "video",
    ".mov": "video",
    ".webm": "video",
    ".pptx": "office",
    ".docx": "office",
}


class _JsonHandler(web.RequestHandler):
    def initialize(self, root: Path, cache: snapshot.SnapshotCache):
        self.root = Path(root)
        self.cache = cache

    def emit(self, payload: dict, status: int = 200) -> None:
        self.set_status(status)
        self.set_header("Content-Type", "application/json; charset=utf-8")
        self.set_header("Cache-Control", "no-store")
        self.finish(json.dumps(payload, default=str))

    def fail(self, status: int, message: str) -> None:
        self.emit({"error": message}, status=status)

    def find_project(self, slug: str) -> dict | None:
        for project in state.scan_projects(self.root, include_completed=True):
            if project["slug"] == slug:
                return project
        return None


class HealthHandler(_JsonHandler):
    def get(self):
        self.emit(
            {
                "ok": True,
                "pid": os.getpid(),
                "started_at": STARTED_AT,
                "workspace_root": str(self.root.resolve()),
            }
        )


class SnapshotHandler(_JsonHandler):
    def get(self):
        snap = self.cache.get()
        client_etag = self.get_argument("etag", None)
        if client_etag and client_etag == snap["etag"]:
            self.emit({"unchanged": True, "etag": snap["etag"]})
            return
        self.emit(snap)


class ActivityHandler(_JsonHandler):
    def get(self):
        try:
            cursor = int(self.get_argument("cursor", "0"))
            limit = int(self.get_argument("limit", "50"))
        except ValueError:
            self.fail(400, "cursor and limit must be integers")
            return
        self.emit(self.cache.activity_page(cursor=cursor, limit=limit))


class ProjectsHandler(_JsonHandler):
    def get(self):
        self.emit({"projects": self.cache.get()["projects"]})


class ProjectHandler(_JsonHandler):
    def get(self, slug):
        for project in self.cache.get()["projects"]:
            if project["slug"] == slug:
                self.emit({"project": project})
                return
        self.fail(404, f"no active project '{slug}'")


class ArtifactsHandler(_JsonHandler):
    def get(self, slug):
        project = self.find_project(slug)
        if project is None:
            self.fail(404, f"no active project '{slug}'")
            return
        pdir = Path(project["dir"])
        groups = artifacts.artifact_groups(pdir, project["deliverables"])
        claimed = self._claimed(pdir, groups)
        untracked_count = len(artifacts.untracked_files(pdir, claimed))
        self.emit({"project": slug, "groups": groups, "untracked_count": untracked_count})

    @staticmethod
    def _claimed(pdir: Path, groups: list[dict]) -> set[str]:
        claimed: set[str] = set()
        for g in groups:
            if not g["current"]:
                continue
            detail = artifacts.group_detail(pdir, g["current"])
            claimed.add(g["current"])
            claimed.update(detail["versions"])
            claimed.update(detail["manifest_media"])
            claimed.update(detail["exports"])
            claimed.update(detail["folder_media"])
        return claimed


class ProjectFilesHandler(_JsonHandler):
    def get(self, slug):
        project = self.find_project(slug)
        if project is None:
            self.fail(404, f"no active project '{slug}'")
            return
        file_class = self.get_argument("class", None)
        narrow_type = self.get_argument("type", None)
        if file_class not in ("text", "media"):
            self.fail(400, "class must be text or media")
            return
        if narrow_type and narrow_type not in ("image", "pdf", "video", "html", "office"):
            self.fail(400, "type must be image, pdf, video, html, or office")
            return
        pdir = Path(project["dir"])
        files = artifacts.project_files(pdir, project["deliverables"], file_class, narrow_type)
        self.emit({"project": slug, "files": files})


class ArtifactDetailHandler(_JsonHandler):
    def get(self, slug, group_id):
        project = self.find_project(slug)
        if project is None:
            self.fail(404, f"no active project '{slug}'")
            return
        pdir = Path(project["dir"])
        if group_id == "untracked":
            groups = artifacts.artifact_groups(pdir, project["deliverables"])
            claimed = ArtifactsHandler._claimed(pdir, groups)
            self.emit({"project": slug, "untracked": artifacts.untracked_files(pdir, claimed)})
            return
        for g in artifacts.artifact_groups(pdir, project["deliverables"]):
            if g["slug"] == group_id:
                detail = artifacts.group_detail(pdir, g["current"]) if g["current"] else {
                    "current": None,
                    "versions": [],
                    "exports": [],
                }
                self.emit({"project": slug, "group": {**g, **detail}})
                return
        self.fail(404, f"no artifact group '{group_id}' in '{slug}'")


class PreviewHandler(_JsonHandler):
    def get(self):
        slug = self.get_argument("project", None)
        rel = self.get_argument("file", None)
        if not slug or not rel:
            self.fail(400, "project and file are required")
            return
        project = self.find_project(slug)
        if project is None:
            self.fail(404, f"no active project '{slug}'")
            return
        pdir = Path(project["dir"])
        resolved = snapshot.resolve_in_project(pdir, rel)
        if resolved is None:
            self.fail(403, "file resolves outside the project root")
            return
        if not resolved.is_file():
            self.fail(404, f"file not found: {rel}")
            return
        st = resolved.stat()
        server_path = "/" + resolved.relative_to(self.root.resolve()).as_posix()
        self.emit(
            {
                "project": slug,
                "file": rel,
                "url": urllib.parse.quote(server_path),
                "os_path": str(resolved),
                "type": MEDIA_TYPES.get(resolved.suffix.lower(), "unsupported"),
                "size": st.st_size,
                "mtime": int(st.st_mtime),
            }
        )


class _JsonBodyHandler(_JsonHandler):
    def payload(self) -> dict:
        try:
            data = json.loads(self.request.body or b"{}")
        except ValueError:
            return {}
        return data if isinstance(data, dict) else {}

    def resolve_payload_path(self, *, want_dir: bool = False):
        """(project, resolved_path) from the JSON body, or None after failing the request."""
        data = self.payload()
        slug, rel = data.get("project"), data.get("file") or data.get("dir")
        if not slug or not rel:
            self.fail(400, "project and file/dir are required")
            return None
        project = self.find_project(slug)
        if project is None:
            self.fail(404, f"no active project '{slug}'")
            return None
        resolved = snapshot.resolve_in_project(Path(project["dir"]), rel)
        if resolved is None:
            self.fail(403, "path resolves outside the project root")
            return None
        if want_dir and not resolved.is_dir():
            self.fail(404, f"not a folder: {rel}")
            return None
        if not want_dir and not resolved.is_file():
            self.fail(404, f"file not found: {rel}")
            return None
        return slug, resolved


class ConvertHandler(_JsonBodyHandler):
    async def post(self):
        result = self.resolve_payload_path()
        if result is None:
            return
        slug, resolved = result
        if resolved.suffix.lower() not in (".pptx", ".docx"):
            self.fail(400, "only .pptx and .docx convert to PDF")
            return
        loop = asyncio.get_running_loop()
        try:
            pdf = await loop.run_in_executor(None, convert.convert_to_pdf, resolved, CACHE_DIR)
        except convert.ConversionError as exc:
            self.fail(503, str(exc))
            return
        server_path = "/" + pdf.relative_to(self.root.resolve()).as_posix()
        self.emit({"project": slug, "url": urllib.parse.quote(server_path)})


class RevealHandler(_JsonBodyHandler):
    def post(self):
        result = self.resolve_payload_path()
        if result is None:
            return
        _, resolved = result
        if os.name == "nt":
            subprocess.Popen(["explorer", f"/select,{resolved}"])
        elif os.uname().sysname == "Darwin":  # pragma: no cover
            subprocess.Popen(["open", "-R", str(resolved)])
        else:  # pragma: no cover
            subprocess.Popen(["xdg-open", str(resolved.parent)])
        self.emit({"ok": True})


class RevealRootHandler(_JsonBodyHandler):
    def post(self):
        resolved = self.root.resolve()
        if os.name == "nt":
            subprocess.Popen(["explorer", str(resolved)])
        elif os.uname().sysname == "Darwin":  # pragma: no cover
            subprocess.Popen(["open", str(resolved)])
        else:  # pragma: no cover
            subprocess.Popen(["xdg-open", str(resolved)])
        self.emit({"ok": True})


class HideHandler(_JsonBodyHandler):
    def post(self):
        result = self.resolve_payload_path(want_dir=True)
        if result is None:
            return
        _, resolved = result
        marker = resolved / ".preview-hide"
        marker.write_text("hidden via AgentFrame local surface\n", encoding="utf-8")
        self.emit({"ok": True, "marker": str(marker)})


def make_handlers(project_root: Path) -> list[tuple]:
    """Route table for the surface API, bound to one workspace root."""
    root = Path(project_root)
    cache = snapshot.SnapshotCache(root)
    kw = {"root": root, "cache": cache}
    return [
        (r"/api/health", HealthHandler, kw),
        (r"/api/snapshot", SnapshotHandler, kw),
        (r"/api/activity", ActivityHandler, kw),
        (r"/api/projects", ProjectsHandler, kw),
        (r"/api/projects/([^/]+)/files", ProjectFilesHandler, kw),
        (r"/api/projects/([^/]+)/artifacts/([^/]+)", ArtifactDetailHandler, kw),
        (r"/api/projects/([^/]+)/artifacts", ArtifactsHandler, kw),
        (r"/api/projects/([^/]+)", ProjectHandler, kw),
        (r"/api/preview", PreviewHandler, kw),
        (r"/api/convert", ConvertHandler, kw),
        (r"/api/reveal", RevealHandler, kw),
        (r"/api/reveal-root", RevealRootHandler, kw),
        (r"/api/hide", HideHandler, kw),
    ]
