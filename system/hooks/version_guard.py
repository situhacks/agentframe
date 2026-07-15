#!/usr/bin/env python3
"""Claude PreToolUse guard for mechanically unsafe versioned-file writes.

The guard is intentionally narrow. It denies direct edits to immutable lower
versions and locked/delivered heads, and denies hand-creating version files
that must go through ``af draft`` or ``af version``. It allows edits to the
current drafting head because surgical edits are a valid workflow and prose
judgment, not a hook, decides surgical versus replacement.

Fail-open on malformed hook payloads. ``af doctor`` and the CLI remain the
cross-harness backstops.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERSION_RE = re.compile(r"(.+)-v(\d+)\.md$")


def _deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def _within_managed_work(path: Path) -> bool:
    for root in (
        ROOT / "workspace" / "projects",
        ROOT / "workspace" / "pipeline" / "applications",
    ):
        try:
            path.resolve().relative_to(root.resolve())
            return True
        except ValueError:
            continue
    return False


def _status(path: Path) -> str | None:
    try:
        head = path.read_text(encoding="utf-8-sig")[:4000]
    except OSError:
        return None
    m = re.search(r"^status:\s*([A-Za-z_-]+)\s*$", head, re.M)
    return m.group(1) if m else None


def _versions(path: Path, name: str) -> list[int]:
    out = []
    try:
        siblings = path.parent.iterdir()
    except OSError:
        return out
    for sibling in siblings:
        m = VERSION_RE.fullmatch(sibling.name)
        if m and m.group(1) == name:
            out.append(int(m.group(2)))
    return out


def decide(payload: dict) -> dict | None:
    if (payload.get("hook_event_name") or "PreToolUse") != "PreToolUse":
        return None
    tool = payload.get("tool_name") or ""
    if tool not in {"Edit", "Write"}:
        return None
    tool_input = payload.get("tool_input") or {}
    raw = tool_input.get("file_path") or ""
    if not raw:
        return None
    path = Path(raw)
    if not path.is_absolute():
        path = Path(payload.get("cwd") or ROOT) / path
    if not _within_managed_work(path):
        return None

    m = VERSION_RE.fullmatch(path.name)
    if not m:
        return None
    name, number = m.group(1), int(m.group(2))
    versions = _versions(path, name)

    if not path.exists():
        if versions:
            return _deny(
                f"Do not hand-create {path.name}; '{name}' already has a version chain. "
                "Use `python system/af.py version <project> <row>` or add "
                "`--artifact <name>` for a nested artifact so numbering and tracker state stay safe."
            )
        return _deny(
            f"Do not hand-create first draft {path.name}. Use "
            "`python system/af.py draft <project> <row> --file <path>` or "
            "`--artifact <name>` so v1 and tracker state are created together."
        )

    if versions and number < max(versions):
        return _deny(
            f"{path.name} is an immutable prior version; {name}-v{max(versions)}.md is the head. "
            "Restore by creating a new version, never by editing history."
        )

    status = _status(path)
    if status in {"locked", "delivered"}:
        return _deny(
            f"{path.name} is {status}. Direct edits are not allowed. After operator confirmation, "
            "run `python system/af.py version` with the row or nested-artifact address; the command "
            "creates a drafting head and records the unlock/version event."
        )
    return None


def run(stdin_text: str) -> str | None:
    try:
        payload = json.loads(stdin_text)
        result = decide(payload)
    except Exception:
        return None
    return json.dumps(result) if result else None


if __name__ == "__main__":
    out = run(sys.stdin.read())
    if out:
        sys.stdout.write(out)
