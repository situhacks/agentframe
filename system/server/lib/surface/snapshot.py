"""Cached dashboard snapshot with etag invalidation.

Invalidation is a stat signature over the narrow watch set (``project.md``,
``activity.md``, governance docs, and the projects folder itself) — recomputed
per request, which is a dozen ``stat`` calls, cheaper and less fragile than a
watcher thread. Clients poll ``GET /api/snapshot?etag=`` and get a tiny
``{"unchanged": true}`` when nothing moved.
"""

from __future__ import annotations

import datetime
import hashlib
from pathlib import Path

from . import artifacts, state

GOVERNANCE_DOCS = ("raid-log.md", "decision-log.md", "workback-schedule.md")
WATCHED_FILENAMES = ("project.md", "activity.md")
ARCHIVE_FILENAME = artifacts.ARCHIVE_REL_PATH
TIMELINE_STATUS_RANK = {"active": 0, "complete": 1, "cancelled": 2}


def _timeline_sort_key(project: dict) -> tuple:
    rank = TIMELINE_STATUS_RANK.get(project.get("status"), 3)
    return (rank, str(project.get("created_at") or ""), project.get("slug") or "")


def resolve_in_project(project_dir: Path, rel: str) -> Path | None:
    """Resolve ``rel`` inside the project; None when it escapes or is absolute."""
    project_dir = Path(project_dir).resolve()
    try:
        candidate = (project_dir / rel).resolve()
    except (OSError, ValueError):
        return None
    if candidate == project_dir or project_dir not in candidate.parents:
        return None
    return candidate


def _deliverable_payload(slug: str, row: dict) -> dict:
    return {
        "slug": slug,
        "file": str(row["file"]),
        "last_updated": state._iso(row.get("last_updated")),
        "status": row.get("status"),
        "review": row.get("review"),
        "job": row.get("job"),
    }


def _latest_deliverable(deliverables: dict, *, status: str | None = None) -> dict | None:
    best = None
    for index, (slug, row) in enumerate((deliverables or {}).items()):
        if not isinstance(row, dict) or not row.get("file"):
            continue
        if status is not None and row.get("status") != status:
            continue
        key = (str(state._iso(row.get("last_updated")) or ""), index)
        if best is None or key >= best[0]:
            best = (key, slug, row)
    if best is None:
        return None
    _, slug, row = best
    return _deliverable_payload(slug, row)


def _current_deliverable(deliverables: dict) -> dict | None:
    """Deterministic project pulse: in-flight work, otherwise latest real state."""
    return _latest_deliverable(deliverables, status="drafting") or _latest_deliverable(deliverables)


def _next_attention(attention: list[dict]) -> dict | None:
    if not attention:
        return None
    item = min(attention, key=lambda row: (row.get("date") is None, row.get("date") or ""))
    return {key: item.get(key) for key in ("date", "kind", "text", "file")}


def _project_updated_at(project: dict, latest: dict | None) -> str | None:
    candidates = [
        str(value)
        for value in (
            state._iso(project.get("last_activity")),
            latest.get("last_updated") if latest else None,
        )
        if value
    ]
    return max(candidates) if candidates else None


def humanize_project_updated(value: str | None) -> str | None:
    """Month/day/time label for project last-updated cells."""
    if not value:
        return None
    text = str(value)
    try:
        if len(text) == 10:
            dt = datetime.datetime.strptime(text, "%Y-%m-%d")
            return f"{dt.strftime('%b')} {dt.day}"
        dt = datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return text
    time_label = dt.strftime("%I:%M %p").lstrip("0")
    return f"{dt.strftime('%b')} {dt.day}, {time_label}"


def _governance_flags(project_dir: Path) -> dict:
    knowledge = Path(project_dir) / "knowledge"
    return {doc: (knowledge / doc).is_file() for doc in GOVERNANCE_DOCS}


def _read_activity(project_dir: Path) -> str:
    activity = Path(project_dir) / "activity.md"
    if not activity.is_file():
        return ""
    try:
        return activity.read_text(encoding="utf-8")
    except OSError:
        return ""


def _timeline_deliverables(project: dict) -> list[dict]:
    """Current + archived tracker rows shaped for the cosmetic calendar."""
    rows = dict(artifacts.load_archived_rows(Path(project["dir"])))
    archived_slugs = set(rows)
    rows.update(project.get("deliverables") or {})
    payload = []
    for slug, row in rows.items():
        if not isinstance(row, dict):
            continue
        item = {
            "slug": slug,
            "file": str(row.get("file")) if row.get("file") else None,
            "last_updated": state._iso(row.get("last_updated")),
            "status": row.get("status"),
            "review": row.get("review"),
            "job": row.get("job"),
            "archived": slug in archived_slugs and slug not in (project.get("deliverables") or {}),
        }
        payload.append(item)
    payload.sort(key=lambda item: (str(item.get("last_updated") or ""), item["slug"]))
    return payload


def _timeline_project(project: dict, activity_text: str) -> dict:
    pdir = Path(project["dir"])
    activity = []
    for entry in state.parse_activity(activity_text):
        if not entry.get("timestamp"):
            continue
        activity.append(
            {
                "timestamp": entry["timestamp"],
                "event": entry.get("event"),
                "text": entry.get("text"),
                "file": state.detect_file_ref(entry["raw"], pdir),
            }
        )
    attention = []
    for item in state.parse_attention(activity_text):
        if item.get("checked") or not item.get("date"):
            continue
        attention.append({key: item.get(key) for key in ("date", "kind", "text", "file")})
    return {
        "slug": project["slug"],
        "name": project.get("name"),
        "status": project.get("status"),
        "domain": project.get("domain"),
        "created_at": project.get("created_at"),
        "last_activity": project.get("last_activity"),
        "shipped_at": project.get("shipped_at"),
        "completed_at": project.get("completed_at"),
        "cancelled_at": project.get("cancelled_at"),
        "deliverables": _timeline_deliverables(project),
        "activity": activity,
        "attention": attention,
    }


def humanize_timestamp(timestamp: str | None, now: datetime.datetime | None = None) -> str | None:
    """Consistent month/day/time label for dashboard activity rows."""
    if not timestamp:
        return None
    try:
        dt = datetime.datetime.strptime(timestamp, "%Y-%m-%d %H:%M")
    except ValueError:
        return timestamp
    time_label = dt.strftime("%I:%M %p").lstrip("0")
    return f"{dt.strftime('%b')} {dt.day}, {time_label}"


def build_snapshot(root: Path, activity_limit: int = 50) -> dict:
    projects = state.scan_projects(root)
    timeline_source = state.scan_projects(root, include_completed=True)
    out_projects = []
    attention_items = []
    activity_entries = []

    for project in projects:
        pdir = Path(project["dir"])
        activity_text = _read_activity(pdir)
        attention = [a for a in state.parse_attention(activity_text) if not a["checked"]]
        governance = _governance_flags(pdir)
        latest = _latest_deliverable(project["deliverables"])
        current = _current_deliverable(project["deliverables"])
        updated_at = _project_updated_at(project, latest)
        out_projects.append(
            {
                "slug": project["slug"],
                "name": project["name"],
                "domain": project["domain"],
                "flow": project["flow"],
                "current_phase": project["current_phase"],
                "last_activity": project["last_activity"],
                "last_updated": updated_at,
                "last_updated_label": humanize_project_updated(updated_at),
                "attention_count": len(attention),
                "latest_deliverable": latest,
                "current_deliverable": current,
                "next_attention": _next_attention(attention),
                "governance_status": "governed" if any(governance.values()) else "ungoverned",
                "governance": governance,
            }
        )
        for item in attention:
            attention_items.append({"project": project["slug"], "project_name": project["name"], **item})
        for entry in state.parse_activity(activity_text):
            entry["project"] = project["slug"]
            entry["project_name"] = project["name"]
            entry["file"] = state.detect_file_ref(entry["raw"], pdir)
            entry["time_label"] = humanize_timestamp(entry["timestamp"])
            activity_entries.append(entry)

    attention_items.sort(key=lambda a: (a["date"] is None, a["date"] or ""))
    out_projects.sort(key=lambda p: (p.get("last_updated") or "", p.get("slug") or ""), reverse=True)
    activity_entries = _newest_first(activity_entries)
    timeline_projects = []
    for project in timeline_source:
        timeline_projects.append(_timeline_project(project, _read_activity(Path(project["dir"]))))
    timeline_projects.sort(key=_timeline_sort_key)

    page = activity_entries[:activity_limit]
    return {
        "updated_at": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
        "workspace_root": str(Path(root).resolve()),
        "projects": out_projects,
        "timeline_projects": timeline_projects,
        "attention": attention_items,
        "recent_activity": {
            "items": page,
            "next_cursor": activity_limit if len(activity_entries) > activity_limit else None,
        },
        "_all_activity": activity_entries,
    }


def _newest_first(entries: list[dict]) -> list[dict]:
    timestamped = [e for e in entries if e["timestamp"] is not None]
    rest = [e for e in entries if e["timestamp"] is None]
    timestamped.sort(key=lambda e: e["timestamp"], reverse=True)
    return timestamped + rest


class SnapshotCache:
    """Rebuilds the snapshot only when the watched-state stat signature moves."""

    def __init__(self, root: Path):
        self._root = Path(root)
        self._signature: str | None = None
        self._snapshot: dict | None = None
        self._all_activity: list[dict] = []

    def _current_signature(self) -> str:
        h = hashlib.md5()
        entries = state.project_directories(self._root, include_completed=True)
        h.update(str([str(e.relative_to(self._root)) for e in entries]).encode())
        for folder in entries:
            for name in WATCHED_FILENAMES:
                f = folder / name
                try:
                    st = f.stat()
                    h.update(f"{name}:{st.st_mtime_ns}:{st.st_size};".encode())
                except OSError:
                    h.update(f"{name}:absent;".encode())
            for doc in GOVERNANCE_DOCS:
                f = folder / "knowledge" / doc
                try:
                    st = f.stat()
                    h.update(f"{doc}:{st.st_mtime_ns}:{st.st_size};".encode())
                except OSError:
                    h.update(f"{doc}:absent;".encode())
            archive = folder / ARCHIVE_FILENAME
            try:
                st = archive.stat()
                h.update(f"archive:{st.st_mtime_ns}:{st.st_size};".encode())
            except OSError:
                h.update(b"archive:absent;")
        return h.hexdigest()

    def get(self) -> dict:
        """Snapshot dict with ``etag``; cached until watched files change."""
        sig = self._current_signature()
        if self._snapshot is None or sig != self._signature:
            snap = build_snapshot(self._root)
            self._all_activity = snap.pop("_all_activity")
            snap["etag"] = sig
            self._snapshot = snap
            self._signature = sig
        return self._snapshot

    def activity_page(self, cursor: int = 0, limit: int = 50) -> dict:
        self.get()
        entries = self._all_activity
        cursor = max(0, int(cursor))
        limit = max(1, min(int(limit), 200))
        page = entries[cursor : cursor + limit]
        next_cursor = cursor + limit if cursor + limit < len(entries) else None
        return {"items": page, "next_cursor": next_cursor}
