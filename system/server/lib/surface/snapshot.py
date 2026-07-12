"""Cached dashboard snapshot with etag invalidation.

Invalidation is a stat signature over the narrow watch set (``project.md``,
``activity.md``, governance docs, ``pipeline.md``, and project folders) — recomputed
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
PIPELINE_TERMINAL_STAGES = {"offer", "rejected", "ghosted", "dropped"}


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


_BLOCK_GAP_MIN = 90
_BLOCK_PAD_MIN = 30


def _minutes(timestamp: str) -> int | None:
    """Minutes-into-day for a 'YYYY-MM-DD HH:MM' timestamp, else None."""
    try:
        hh, mm = timestamp[11:13], timestamp[14:16]
        return int(hh) * 60 + int(mm)
    except (ValueError, IndexError):
        return None


def _finish_block(date: str, events: list[dict]) -> dict:
    start = max(0, events[0]["time"] - _BLOCK_PAD_MIN)
    # a block is never shorter than one hour, so single events stay legible
    end = min(1439, max(events[-1]["time"] + _BLOCK_PAD_MIN, start + 60))
    return {"date": date, "start": start, "end": end, "events": events}


def _work_blocks(activity: list[dict]) -> list[dict]:
    """Cluster timed activity into padded per-day work blocks (see spec)."""
    by_day: dict[str, list[dict]] = {}
    for entry in activity:
        ts = entry.get("timestamp")
        if not ts:
            continue
        minute = _minutes(str(ts))
        if minute is None:
            continue
        by_day.setdefault(str(ts)[:10], []).append(
            {"time": minute, "label": entry.get("event") or entry.get("text") or "activity",
             "file": entry.get("file")}
        )
    blocks: list[dict] = []
    for date, events in by_day.items():
        events.sort(key=lambda e: e["time"])
        current: list[dict] = []
        for ev in events:
            if current and ev["time"] - current[-1]["time"] > _BLOCK_GAP_MIN:
                blocks.append(_finish_block(date, current))
                current = []
            current.append(ev)
        if current:
            blocks.append(_finish_block(date, current))
    blocks.sort(key=lambda b: (b["date"], b["start"]))
    return blocks


def _worked_days(deliverables: list[dict], activity: list[dict]) -> list[str]:
    """Sorted unique YYYY-MM-DD strings for days with any logged event."""
    days = set()
    for item in deliverables:
        value = item.get("last_updated")
        if value:
            days.add(str(value)[:10])
    for entry in activity:
        ts = entry.get("timestamp")
        if ts:
            days.add(str(ts)[:10])
    return sorted(d for d in days if len(d) == 10 and d[4] == "-")


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
    deliverables = _timeline_deliverables(project)
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
        "deliverables": deliverables,
        "activity": activity,
        "attention": attention,
        "worked_days": _worked_days(deliverables, activity),
        "work_blocks": _work_blocks(activity),
    }


def _pipeline_timeline(root: Path) -> dict | None:
    """Aggregate live career-case dates into one read-only calendar lane."""
    board = Path(root) / "workspace" / "pipeline" / "pipeline.md"
    if not board.is_file():
        return None
    try:
        fm = state.parse_frontmatter(board.read_text(encoding="utf-8"))
    except OSError:
        return None
    rows = fm.get("applications")
    if not isinstance(rows, dict):
        return None

    attention = []
    worked_days = set()
    for slug, row in rows.items():
        if not isinstance(row, dict):
            continue
        for key in ("saved", "applied"):
            day = str(state._iso(row.get(key)) or "")[:10]
            if state.DATE_RE.match(day):
                worked_days.add(day)
        if row.get("stage") in PIPELINE_TERMINAL_STAGES:
            continue
        label = " · ".join(str(value) for value in (row.get("company"), row.get("role")) if value) or str(slug)
        for field, kind in (("deadline", "career case deadline"), ("next_nudge", "career follow-up")):
            day = str(state._iso(row.get(field)) or "")[:10]
            if state.DATE_RE.match(day):
                attention.append({"date": day, "kind": kind, "text": label, "file": None})

    created_at = state._iso(fm.get("created_at"))
    last_activity = state._iso(fm.get("last_activity"))
    if not attention and not rows:
        return None
    if last_activity:
        day = str(last_activity)[:10]
        if state.DATE_RE.match(day):
            worked_days.add(day)
    attention.sort(key=lambda item: (item["date"], item["kind"], item["text"]))
    return {
        "slug": "@career-pipeline",
        "name": "Career cases",
        "previewable": False,
        "status": "active",
        "domain": "careers",
        "created_at": created_at,
        "last_activity": last_activity,
        "shipped_at": None,
        "completed_at": None,
        "cancelled_at": None,
        "deliverables": [],
        "activity": [],
        "attention": attention,
        "worked_days": sorted(worked_days),
        "work_blocks": [],
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
    pipeline_timeline = _pipeline_timeline(root)
    if pipeline_timeline:
        timeline_projects.append(pipeline_timeline)
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
        pipeline = self._root / "workspace" / "pipeline" / "pipeline.md"
        try:
            st = pipeline.stat()
            h.update(f"pipeline.md:{st.st_mtime_ns}:{st.st_size};".encode())
        except OSError:
            h.update(b"pipeline.md:absent;")
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
