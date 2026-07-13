"""Read-only reconciliation of project automation declarations and daemon runtime state."""

from __future__ import annotations

import datetime as dt
import json
from pathlib import Path

from . import state


REGISTRY_REL = Path("system/daemon/local/registry.json")


def _read_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def _parse_time(value) -> dt.datetime | None:
    if not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.datetime.now().astimezone().tzinfo)
    return parsed


def _runtime_state(status: dict, deployment_id: str, poll_seconds: int, now: dt.datetime) -> str:
    heartbeat = _parse_time(status.get("heartbeat_at"))
    if not heartbeat or (now - heartbeat).total_seconds() > max(30, poll_seconds * 3):
        return "offline"
    current = status.get("current") or {}
    if status.get("state") == "busy" and current.get("deployment_id") == deployment_id:
        return "busy"
    return "online"


def _receipt_summary(queue_root: Path, now: dt.datetime) -> dict:
    outbox = queue_root / "outbox"
    counts = {"done": 0, "blocked": 0, "failed": 0}
    latest = None
    if not outbox.is_dir():
        return {"today": counts, "last_result": None}
    for path in outbox.glob("*.result.json"):
        payload = _read_json(path)
        status_value = payload.get("status")
        try:
            modified = dt.datetime.fromtimestamp(path.stat().st_mtime, tz=now.tzinfo)
        except OSError:
            continue
        if modified.date() == now.date() and status_value in counts:
            counts[status_value] += 1
        row = {
            "task_id": payload.get("task_id"),
            "status": status_value,
            "summary": payload.get("summary"),
            "time": modified.isoformat(timespec="seconds"),
        }
        if latest is None or row["time"] > latest["time"]:
            latest = row
    return {"today": counts, "last_result": latest}


def _runtime_rows(root: Path, now: dt.datetime) -> tuple[dict, dict]:
    registry_path = Path(root) / REGISTRY_REL
    registry = _read_json(registry_path)
    if registry.get("schema_version") != 1:
        return {}, {}
    status = _read_json(registry_path.with_name("status.json"))
    poll_seconds = int(registry.get("poll_seconds") or 5)
    deployments = {}
    for item in registry.get("automations") or []:
        if not isinstance(item, dict) or not item.get("id"):
            continue
        queue_root = Path(str(item.get("queue_root") or ""))
        queued = len(list((queue_root / "inbox").glob("*.task.json"))) if queue_root.is_absolute() else 0
        receipts = _receipt_summary(queue_root, now) if queue_root.is_absolute() else {
            "today": {"done": 0, "blocked": 0, "failed": 0}, "last_result": None}
        current = status.get("current") or {}
        deployments[item["id"]] = {
            "deployment_id": item["id"],
            "label": item.get("label") or item["id"],
            "project": item.get("project"),
            "automation_id": item.get("automation_id"),
            "enabled": bool(item.get("enabled", True)),
            "runtime_state": _runtime_state(status, item["id"], poll_seconds, now),
            "queued": queued,
            "current_task": current.get("task_id") if current.get("deployment_id") == item["id"] else None,
            **receipts,
        }
    return deployments, {"registry_path": str(registry_path), "heartbeat_at": status.get("heartbeat_at")}


def build_model(root: Path, now: dt.datetime | None = None) -> dict:
    root = Path(root)
    now = now or dt.datetime.now().astimezone()
    deployments, meta = _runtime_rows(root, now)
    claimed = set()
    rows = []
    for project in state.scan_projects(root):
        for automation_id, declared in project.get("automations", {}).items():
            if not isinstance(declared, dict):
                continue
            deployment_id = declared.get("deployment_id")
            runtime = deployments.get(deployment_id) if deployment_id else None
            if runtime:
                claimed.add(deployment_id)
            desired = declared.get("status")
            runtime_state = runtime.get("runtime_state") if runtime else "not-deployed"
            queued = runtime.get("queued", 0) if runtime else 0
            issues = []
            if desired == "ready" and not runtime:
                issues.append("ready-not-deployed")
            if desired == "active" and not runtime:
                issues.append("active-not-deployed")
            elif desired == "active" and runtime_state == "offline":
                issues.append("active-offline")
            if desired == "paused" and queued:
                issues.append("paused-with-queue")
            if runtime and not runtime.get("enabled") and desired == "active":
                issues.append("active-runtime-disabled")
            rows.append({
                "project": project["slug"],
                "project_name": project.get("name"),
                "automation_id": automation_id,
                "job": declared.get("job"),
                "file": declared.get("file"),
                "desired_status": desired,
                "deployment_id": deployment_id,
                "runtime_state": runtime_state,
                "queued": queued,
                "current_task": runtime.get("current_task") if runtime else None,
                "today": runtime.get("today") if runtime else {"done": 0, "blocked": 0, "failed": 0},
                "last_result": runtime.get("last_result") if runtime else None,
                "issues": issues,
            })
    for deployment_id, runtime in deployments.items():
        if deployment_id in claimed:
            continue
        rows.append({
            "project": runtime.get("project"),
            "project_name": None,
            "automation_id": runtime.get("automation_id"),
            "job": runtime.get("label"),
            "file": None,
            "desired_status": "undeclared",
            "deployment_id": deployment_id,
            "runtime_state": runtime.get("runtime_state"),
            "queued": runtime.get("queued", 0),
            "current_task": runtime.get("current_task"),
            "today": runtime.get("today"),
            "last_result": runtime.get("last_result"),
            "issues": ["runtime-orphan"],
        })
    rows.sort(key=lambda row: (not row["issues"], row.get("project") or "", row.get("automation_id") or ""))
    return {"generated_at": now.isoformat(timespec="seconds"), "rows": rows, **meta}
