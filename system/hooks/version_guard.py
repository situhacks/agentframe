#!/usr/bin/env python3
"""Guard for mechanically unsafe versioned-file writes.

The guard is intentionally narrow. It denies direct edits to immutable lower
versions and published heads, and denies hand-creating version files
that must go through ``af draft`` or ``af version``. It allows Edit calls on
the current drafting head, because that is how the new head gets written after
``af version`` cuts it; the rule that a content change versions FIRST is prose
(``deliverable-versioning.md``), since a hook cannot see whether the head it is
being asked to edit is a fresh snapshot or the only one. A full-file Write over
a head that already has body content is denied — workspace files have no git
history, so a clobbered draft is unrecoverable; the deny reason names both
legitimate exits.

Fail-open on malformed hook payloads. ``af doctor`` and the CLI remain the
backstops.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERSION_RE = re.compile(r"(.+)-v(\d+)\.md$")
FRONTMATTER_RE = re.compile(r"\A---\n.*?\n---\n?", re.S)
PPT_CONFIRMATION_SUFFIX = ".agentframe-confirmation.json"


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
        ROOT / "workspace" / "studio" / "posts",
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


def _has_drafted_body(path: Path) -> bool:
    try:
        text = path.read_text(encoding="utf-8-sig")
    except OSError:
        return False
    return bool(FRONTMATTER_RE.sub("", text, count=1).strip())


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


def _resolve_path(raw: str, payload: dict) -> Path:
    path = Path(raw.strip().strip('"'))
    if not path.is_absolute():
        path = Path(payload.get("cwd") or ROOT) / path
    return path


def _targets(payload: dict) -> list[tuple[Path, str]]:
    """Return ``(path, operation)`` pairs for one Edit or Write call."""
    tool = payload.get("tool_name") or ""
    tool_input = payload.get("tool_input") or {}
    if tool not in {"Edit", "Write"}:
        return []
    raw = tool_input.get("file_path") or tool_input.get("path") or ""
    if not raw:
        return []
    operation = "edit" if tool == "Edit" else "write"
    return [(_resolve_path(raw, payload), operation)]


def _decide_target(path: Path, operation: str) -> dict | None:
    if path.name.endswith(PPT_CONFIRMATION_SUFFIX):
        try:
            path.resolve().relative_to(ROOT.resolve())
        except ValueError:
            return None
        if path.exists():
            return _deny(
                f"{path.name} is a sealed PPT confirmation contract and is immutable. "
                "Create a new run-bound approval instead of editing or overwriting "
                "this record."
            )
        return _deny(
            f"Do not hand-create {path.name}. Use "
            "`python system/tools/ppt_master_contract.py seal ...`; the adapter is "
            "the exclusive creator and refuses overwrite."
        )

    if not _within_managed_work(path):
        return None

    if path.exists() and _status(path) == "published":
        if VERSION_RE.fullmatch(path.name):
            exit_path = (
                "run `python system/af.py version` with the row or nested-artifact address; "
                "the command creates a new drafting head while preserving the published version."
            )
        else:
            exit_path = "create a new tracked edition; this unversioned published record cannot be reopened."
        return _deny(
            f"{path.name} is published and immutable. Direct edits are not allowed. {exit_path}"
        )

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

    if operation == "write" and _has_drafted_body(path):
        return _deny(
            f"Full-file Write would clobber {path.name}'s drafted content, and workspace files have "
            "no git history to restore from. If this head is already the fresh copy from "
            "`af version`, write the change as targeted Edit calls on it. Otherwise snapshot first "
            "(`python system/af.py version <project> <row>`, `--artifact <name>` for nested), then "
            "apply the rewrite as one Edit replacing the new head's body."
        )
    return None


def decide(payload: dict) -> dict | None:
    if (payload.get("hook_event_name") or "PreToolUse") != "PreToolUse":
        return None
    for path, operation in _targets(payload):
        result = _decide_target(path, operation)
        if result:
            return result
    return None


def run(stdin_text: str) -> str | None:
    try:
        payload = json.loads(stdin_text)
        result = decide(payload)
    except Exception:
        return None
    return json.dumps(result) if result else None


def dispatch(stdin_text: str) -> str:
    """Render one process response; always valid JSON."""
    return run(stdin_text) or "{}"


if __name__ == "__main__":
    # Command-line arguments are ignored, so stale wiring that still passes flags keeps working.
    sys.stdout.write(dispatch(sys.stdin.read()))
