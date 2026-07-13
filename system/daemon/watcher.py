"""Single-flight multi-queue host for AgentFrame managed runs."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path


TASK_ID_RE = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9._-]*$")
RESULT_STATUSES = {"done", "blocked", "failed"}
QUEUE_DIRS = ("inbox", "processing", "archive", "outbox", "logs")


class RegistryError(ValueError):
    pass


def now_iso() -> str:
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def atomic_json(path: Path, payload: dict) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def load_registry(path: Path) -> dict:
    path = Path(path).resolve()
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError) as exc:
        raise RegistryError(f"cannot read registry {path}: {exc}") from exc
    if data.get("schema_version") != 1:
        raise RegistryError("registry schema_version must be 1")
    workspace = Path(data.get("workspace_root", "")).expanduser()
    if not workspace.is_dir() or not (workspace / "AGENTS.md").is_file():
        raise RegistryError(f"workspace_root is not an AgentFrame workspace: {workspace}")
    profiles = data.get("body_profiles")
    automations = data.get("automations")
    if not isinstance(profiles, dict) or not isinstance(automations, list):
        raise RegistryError("registry requires body_profiles mapping and automations list")
    seen = set()
    for automation in automations:
        aid = automation.get("id") if isinstance(automation, dict) else None
        if not aid or aid in seen:
            raise RegistryError(f"automation id missing or duplicated: {aid}")
        seen.add(aid)
        if automation.get("body_profile") not in profiles:
            raise RegistryError(f"automation '{aid}' names unknown body_profile")
        queue_root = Path(automation.get("queue_root", "")).expanduser()
        if not queue_root.is_absolute():
            raise RegistryError(f"automation '{aid}' queue_root must be absolute")
    for name, profile in profiles.items():
        if not isinstance(profile, dict) or not profile.get("executable"):
            raise RegistryError(f"body profile '{name}' requires executable")
        if not isinstance(profile.get("args", []), list) or not isinstance(profile.get("env", {}), dict):
            raise RegistryError(f"body profile '{name}' args/env must be list/mapping")
    data["_path"] = str(path)
    data["_workspace"] = str(workspace.resolve())
    data["_status_path"] = str(path.with_name("status.json"))
    return data


def queue_paths(automation: dict) -> dict[str, Path]:
    root = Path(automation["queue_root"]).expanduser().resolve()
    return {name: root / name for name in QUEUE_DIRS}


def ensure_queues(registry: dict) -> None:
    for automation in registry["automations"]:
        for path in queue_paths(automation).values():
            path.mkdir(parents=True, exist_ok=True)


def read_task(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(data, dict):
        raise ValueError("task file must contain one JSON object")
    task_id = data.get("id")
    if data.get("schema_version") != 1:
        raise ValueError("schema_version must be 1")
    if not isinstance(task_id, str) or not TASK_ID_RE.fullmatch(task_id):
        raise ValueError("id must contain only letters, numbers, dot, underscore, or hyphen")
    if not isinstance(data.get("task"), str) or not data["task"].strip():
        raise ValueError("task must be a non-empty string")
    return data


def task_order(path: Path) -> tuple:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
        requested = str(data.get("requested_at") or "")
    except (OSError, ValueError):
        requested = ""
    return (requested or dt.datetime.fromtimestamp(path.stat().st_mtime).isoformat(), path.name)


def candidates(registry: dict) -> list[tuple[tuple, dict, Path]]:
    found = []
    for automation in registry["automations"]:
        if not automation.get("enabled", True):
            continue
        inbox = queue_paths(automation)["inbox"]
        for path in inbox.glob("*.task.json"):
            if path.is_file():
                found.append((task_order(path), automation, path))
    return sorted(found, key=lambda item: item[0])


def fill(value: str, variables: dict[str, str]) -> str:
    for key, replacement in variables.items():
        value = value.replace("{" + key + "}", replacement)
    return value


def validate_receipt(path: Path, task_id: str) -> dict:
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    if data.get("schema_version") != 1 or data.get("task_id") != task_id:
        raise ValueError("receipt schema_version/task_id mismatch")
    if data.get("status") not in RESULT_STATUSES:
        raise ValueError("receipt status must be done, blocked, or failed")
    if not isinstance(data.get("summary"), str) or not data["summary"].strip():
        raise ValueError("receipt summary must be non-empty")
    if not isinstance(data.get("outputs", []), list):
        raise ValueError("receipt outputs must be a list")
    return data


def failed_receipt(task_id: str, summary: str, operator_action: str | None = None) -> dict:
    return {
        "schema_version": 1,
        "task_id": task_id,
        "status": "failed",
        "summary": summary,
        "outputs": [],
        "operator_action": operator_action,
        "finished_at": now_iso(),
    }


def archive_path(paths: dict[str, Path], source: Path) -> Path:
    target = paths["archive"] / source.name
    if not target.exists():
        return target
    stamp = dt.datetime.now().strftime("%Y%m%dT%H%M%S%f")
    return paths["archive"] / f"{source.stem}.{stamp}{source.suffix}"


def recover_stranded(registry: dict) -> None:
    for automation in registry["automations"]:
        paths = queue_paths(automation)
        for task_path in sorted(paths["processing"].glob("*.task.json")):
            try:
                task_id = read_task(task_path)["id"]
            except (OSError, ValueError, json.JSONDecodeError):
                task_id = re.sub(r"[^a-zA-Z0-9._-]", "-", task_path.stem) or "interrupted"
            result_path = paths["outbox"] / f"{task_id}.result.json"
            try:
                validate_receipt(result_path, task_id)
            except (OSError, ValueError, json.JSONDecodeError):
                atomic_json(result_path, failed_receipt(
                    task_id, "Watcher restarted while this task was processing; it was not replayed automatically.",
                    "Review partial project changes, then submit a deliberate new task if retry is safe."))
            shutil.move(str(task_path), str(archive_path(paths, task_path)))


def terminate_tree(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    else:  # pragma: no cover - Windows is the primary deployment
        os.killpg(process.pid, signal.SIGKILL)


def run_body(command: list[str], cwd: Path, env: dict, timeout: int) -> tuple[int | None, str, str, bool]:
    process = subprocess.Popen(
        command,
        cwd=str(cwd),
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        start_new_session=(os.name != "nt"),
        creationflags=(subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        return process.returncode, stdout, stderr, False
    except subprocess.TimeoutExpired:
        terminate_tree(process)
        stdout, stderr = process.communicate()
        return None, stdout, stderr, True


def process_task(registry: dict, automation: dict, inbox_path: Path, kickoff: str) -> dict:
    paths = queue_paths(automation)
    claimed = paths["processing"] / inbox_path.name
    try:
        os.replace(inbox_path, claimed)
    except OSError:
        return {"processed": False, "reason": "claim-failed"}
    try:
        task = read_task(claimed)
        task_id = task["id"]
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        task_id = re.sub(r"[^a-zA-Z0-9._-]", "-", claimed.stem) or "invalid-task"
        result = paths["outbox"] / f"{task_id}.result.json"
        atomic_json(result, failed_receipt(task_id, f"Invalid task file: {exc}"))
        shutil.move(str(claimed), str(archive_path(paths, claimed)))
        return {"processed": True, "task_id": task_id, "status": "failed"}

    result_path = paths["outbox"] / f"{task_id}.result.json"
    if result_path.exists():
        try:
            validate_receipt(result_path, task_id)
        except (OSError, ValueError, json.JSONDecodeError):
            atomic_json(result_path, failed_receipt(
                task_id, "A prior receipt existed but was invalid; this duplicate task was not executed.",
                "Inspect the queue and submit a new task id if a deliberate retry is safe."))
        shutil.move(str(claimed), str(archive_path(paths, claimed)))
        return {"processed": True, "task_id": task_id, "status": "duplicate"}

    workspace = Path(registry["_workspace"])
    variables = {
        "workspace": str(workspace),
        "deployment_id": automation["id"],
        "project": automation["project"],
        "automation_id": automation["automation_id"],
        "task_file": str(claimed.resolve()),
        "result_file": str(result_path.resolve()),
    }
    variables["prompt"] = fill(kickoff, variables)
    profile = registry["body_profiles"][automation["body_profile"]]
    command = [fill(str(profile["executable"]), variables)] + [
        fill(str(arg), variables) for arg in profile.get("args", [])
    ]
    env = os.environ.copy()
    env.update({str(k): fill(str(v), variables) for k, v in profile.get("env", {}).items()})
    env["AGENTFRAME_MANAGED_RUN"] = "1"
    env["AGENTFRAME_TASK_FILE"] = variables["task_file"]
    env["AGENTFRAME_RESULT_FILE"] = variables["result_file"]

    timeout = int(profile.get("timeout_seconds", 1800))
    try:
        code, stdout, stderr, timed_out = run_body(command, workspace, env, timeout)
    except OSError as exc:
        code, stdout, stderr, timed_out = None, "", str(exc), False
    (paths["logs"] / f"{task_id}.stdout.log").write_text(stdout or "", encoding="utf-8")
    (paths["logs"] / f"{task_id}.stderr.log").write_text(stderr or "", encoding="utf-8")

    try:
        receipt = validate_receipt(result_path, task_id)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        if timed_out:
            summary = f"Managed body exceeded the {timeout}s timeout and was terminated."
        elif code not in (0, None):
            summary = f"Managed body exited with code {code} without a valid receipt."
        else:
            summary = f"Managed body did not write a valid receipt: {exc}"
        receipt = failed_receipt(task_id, summary, "Inspect daemon logs and project changes before retrying.")
        atomic_json(result_path, receipt)
    shutil.move(str(claimed), str(archive_path(paths, claimed)))
    return {"processed": True, "task_id": task_id, "status": receipt["status"], "exit_code": code}


def deployment_snapshot(registry: dict, current: dict | None = None) -> list[dict]:
    rows = []
    for automation in registry["automations"]:
        paths = queue_paths(automation)
        receipts = list(paths["outbox"].glob("*.result.json"))
        rows.append({
            "id": automation["id"],
            "label": automation.get("label") or automation["id"],
            "project": automation.get("project"),
            "automation_id": automation.get("automation_id"),
            "enabled": bool(automation.get("enabled", True)),
            "queued": len(list(paths["inbox"].glob("*.task.json"))),
            "processing": len(list(paths["processing"].glob("*.task.json"))),
            "receipts": len(receipts),
            "current_task": current.get("task_id") if current and current.get("deployment_id") == automation["id"] else None,
        })
    return rows


def write_status(registry: dict, state: str, started_at: str, current: dict | None = None) -> None:
    atomic_json(Path(registry["_status_path"]), {
        "schema_version": 1,
        "pid": os.getpid(),
        "state": state,
        "started_at": started_at,
        "heartbeat_at": now_iso(),
        "current": current,
        "deployments": deployment_snapshot(registry, current),
    })


def run_once(registry: dict, kickoff: str, started_at: str) -> dict:
    found = candidates(registry)
    if not found:
        write_status(registry, "idle", started_at)
        return {"processed": False, "reason": "empty"}
    _, automation, path = found[0]
    current = {"deployment_id": automation["id"], "task_file": str(path), "task_id": None,
               "started_at": now_iso()}
    try:
        current["task_id"] = read_task(path)["id"]
    except (OSError, ValueError, json.JSONDecodeError):
        pass
    write_status(registry, "busy", started_at, current)
    result = process_task(registry, automation, path, kickoff)
    write_status(registry, "idle", started_at)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description="AgentFrame managed automation watcher")
    parser.add_argument("--registry", required=True, type=Path)
    parser.add_argument("--once", action="store_true", help="process at most one task and exit")
    args = parser.parse_args()
    registry = load_registry(args.registry)
    kickoff_path = Path(registry["_workspace"]) / "system" / "daemon" / "kickoff-prompt.md"
    kickoff = kickoff_path.read_text(encoding="utf-8-sig")
    ensure_queues(registry)
    recover_stranded(registry)
    started_at = now_iso()
    if args.once:
        run_once(registry, kickoff, started_at)
        return
    poll_seconds = max(1, int(registry.get("poll_seconds", 5)))
    while True:
        run_once(registry, kickoff, started_at)
        time.sleep(poll_seconds)


if __name__ == "__main__":
    main()
