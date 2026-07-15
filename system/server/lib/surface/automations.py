"""Read-only reconciliation of project automation declarations and daemon runtime state."""

from __future__ import annotations

import datetime as dt
import json
from pathlib import Path

from . import state


REGISTRY_REL = Path("system/daemon/local/registry.json")
RECEIPT_PAGE_SIZE = 50
RECEIPT_FILTERS = {"all", "issues", "done", "blocked", "failed"}


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
    receipts = []
    if not outbox.is_dir():
        return {"today": counts, "last_result": None, "receipts": receipts, "receipt_ids": []}
    for path in outbox.glob("*.result.json"):
        payload = _read_json(path)
        status_value = payload.get("status")
        finished = _parse_time(payload.get("finished_at"))
        if finished is None:
            try:
                finished = dt.datetime.fromtimestamp(path.stat().st_mtime, tz=now.tzinfo)
            except OSError:
                continue
        if finished.date() == now.date() and status_value in counts:
            counts[status_value] += 1
        row = {
            "task_id": payload.get("task_id"),
            "status": status_value,
            "summary": payload.get("summary"),
            "time": finished.isoformat(timespec="seconds"),
        }
        receipts.append(row)
        if latest is None or _receipt_sort_key(row) > _receipt_sort_key(latest):
            latest = row
    receipts.sort(key=_receipt_sort_key, reverse=True)
    return {
        "today": counts,
        "last_result": latest,
        "receipts": receipts,
        "receipt_ids": [row["task_id"] for row in receipts if row.get("task_id")],
    }


def _receipt_sort_key(row: dict) -> tuple[float, str]:
    parsed = _parse_time(row.get("time"))
    timestamp = parsed.timestamp() if parsed else 0.0
    return timestamp, str(row.get("task_id") or "")


def _task_summary(queue_root: Path, now: dt.datetime, receipt_ids: set[str]) -> dict:
    submitted_today = set()
    awaiting = []
    for folder in ("inbox", "processing", "archive"):
        base = queue_root / folder
        if not base.is_dir():
            continue
        for path in base.glob("*.task.json"):
            payload = _read_json(path)
            task_id = payload.get("id")
            if not task_id:
                continue
            requested = _parse_time(payload.get("requested_at"))
            if requested is None:
                try:
                    requested = dt.datetime.fromtimestamp(path.stat().st_mtime, tz=now.tzinfo)
                except OSError:
                    continue
            if requested.date() == now.date():
                submitted_today.add(task_id)
            if folder in {"inbox", "processing"} and task_id not in receipt_ids:
                awaiting.append({
                    "task_id": task_id,
                    "requested_at": requested.isoformat(timespec="seconds"),
                    "age_seconds": max(0, int((now - requested).total_seconds())),
                    "state": "running" if folder == "processing" else "queued",
                })
    awaiting.sort(key=lambda row: row["requested_at"])
    return {
        "requests_today": len(submitted_today),
        "awaiting": len(awaiting),
        "awaiting_tasks": awaiting,
        "oldest_awaiting_seconds": awaiting[0]["age_seconds"] if awaiting else None,
    }


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
        receipts = _receipt_summary(queue_root, now) if queue_root.is_absolute() else {
            "today": {"done": 0, "blocked": 0, "failed": 0}, "last_result": None,
            "receipts": [], "receipt_ids": []}
        receipt_ids = set(receipts.pop("receipt_ids"))
        tasks = _task_summary(queue_root, now, receipt_ids) if queue_root.is_absolute() else {
            "requests_today": 0, "awaiting": 0, "awaiting_tasks": [], "oldest_awaiting_seconds": None}
        current = status.get("current") or {}
        deployments[item["id"]] = {
            "deployment_id": item["id"],
            "label": item.get("label") or item["id"],
            "project": item.get("project"),
            "automation_id": item.get("automation_id"),
            "enabled": bool(item.get("enabled", True)),
            "body_profile": item.get("body_profile"),
            "runtime_state": _runtime_state(status, item["id"], poll_seconds, now),
            "queued": tasks["awaiting"],
            "current_task": current.get("task_id") if current.get("deployment_id") == item["id"] else None,
            **receipts,
            **tasks,
        }
    return deployments, {"registry_path": str(registry_path), "heartbeat_at": status.get("heartbeat_at")}


def _reconcile(root: Path, now: dt.datetime) -> tuple[list[dict], dict, dict]:
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
                "requests_today": runtime.get("requests_today", 0) if runtime else 0,
                "awaiting_tasks": runtime.get("awaiting_tasks", []) if runtime else [],
                "oldest_awaiting_seconds": runtime.get("oldest_awaiting_seconds") if runtime else None,
                "body_profile": runtime.get("body_profile") if runtime else None,
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
            "requests_today": runtime.get("requests_today", 0),
            "awaiting_tasks": runtime.get("awaiting_tasks", []),
            "oldest_awaiting_seconds": runtime.get("oldest_awaiting_seconds"),
            "body_profile": runtime.get("body_profile"),
            "current_task": runtime.get("current_task"),
            "today": runtime.get("today"),
            "last_result": runtime.get("last_result"),
            "issues": ["runtime-orphan"],
        })
    rows.sort(key=lambda row: (not row["issues"], row.get("project") or "", row.get("automation_id") or ""))
    return rows, deployments, meta


def _all_receipts(rows: list[dict], deployments: dict) -> list[dict]:
    identities = {}
    for row in rows:
        deployment_id = row.get("deployment_id")
        if deployment_id and deployment_id not in identities:
            identities[deployment_id] = {
                "automation_id": row.get("automation_id"),
                "deployment_id": deployment_id,
                "project": row.get("project"),
                "project_name": row.get("project_name"),
            }

    recent_receipts = []
    for deployment_id, runtime in deployments.items():
        identity = identities.get(deployment_id) or {
            "automation_id": runtime.get("automation_id"),
            "deployment_id": deployment_id,
            "project": runtime.get("project"),
            "project_name": None,
        }
        for receipt in runtime.get("receipts", []):
            recent_receipts.append({**identity, **receipt})
    recent_receipts.sort(key=_receipt_sort_key, reverse=True)
    return recent_receipts


def receipt_page(
    root: Path,
    cursor: int = 0,
    limit: int = RECEIPT_PAGE_SIZE,
    status: str = "all",
    now: dt.datetime | None = None,
) -> dict:
    root = Path(root)
    now = now or dt.datetime.now().astimezone()
    cursor = max(0, int(cursor))
    limit = max(1, min(int(limit), 200))
    if status not in RECEIPT_FILTERS:
        raise ValueError(f"status must be one of: {', '.join(sorted(RECEIPT_FILTERS))}")

    rows, deployments, _ = _reconcile(root, now)
    receipts = _all_receipts(rows, deployments)
    if status == "issues":
        receipts = [row for row in receipts if row.get("status") in {"blocked", "failed"}]
    elif status != "all":
        receipts = [row for row in receipts if row.get("status") == status]

    page = receipts[cursor:cursor + limit]
    next_cursor = cursor + limit if cursor + limit < len(receipts) else None
    return {"items": page, "next_cursor": next_cursor, "total": len(receipts), "status": status}


def build_model(root: Path, now: dt.datetime | None = None) -> dict:
    root = Path(root)
    now = now or dt.datetime.now().astimezone()
    rows, deployments, meta = _reconcile(root, now)
    recent_receipts = _all_receipts(rows, deployments)
    attention = []
    for row in rows:
        identity = {
            "automation_id": row.get("automation_id"),
            "deployment_id": row.get("deployment_id"),
            "project": row.get("project"),
            "project_name": row.get("project_name"),
        }
        for issue in row.get("issues") or []:
            attention.append({**identity, "kind": issue, "summary": "Declared and observed state do not match."})
        for task in row.get("awaiting_tasks") or []:
            attention.append({
                **identity, "kind": "unanswered", "task_id": task["task_id"],
                "time": task["requested_at"], "age_seconds": task["age_seconds"],
                "summary": f"Request is still {task['state']} with no terminal receipt.",
            })
    for receipt in recent_receipts:
        parsed = _parse_time(receipt.get("time"))
        if receipt.get("status") in {"blocked", "failed"} and parsed and parsed.date() == now.date():
            attention.append({
                **receipt, "kind": receipt["status"],
                "summary": receipt.get("summary") or "Terminal receipt needs review.",
            })
    attention.sort(key=lambda row: row.get("time") or "", reverse=True)
    first_page = recent_receipts[:RECEIPT_PAGE_SIZE]
    return {
        "generated_at": now.isoformat(timespec="seconds"),
        "rows": rows,
        "attention": attention,
        "recent_receipts": first_page,
        "receipts_total": len(recent_receipts),
        "receipts_next_cursor": RECEIPT_PAGE_SIZE if len(recent_receipts) > RECEIPT_PAGE_SIZE else None,
        **meta,
    }
