#!/usr/bin/env python3
"""Deterministic guards around the vendored ppt-master skill.

The overlay rules in system/vendor/ppt-master/AGENTS.md are read once at run
start and decay out of context over a long deck session. These hooks re-inject
each rule at the exact moment it is violated, so they hold regardless of
compaction. All guard logic lives here — vendored files are never patched
(refresh procedure: VENDOR.md). Wiring lives in .claude/settings.json; command
shapes matched here are pinned by system/tests/test_ppt_master_guard.py — re-run
after every vendor refresh.

PreToolUse (Bash|PowerShell):
  - project_manager.py init      -> deny when the project would land inside
                                    this repo's system/ tree (vendor default
                                    is cwd/projects, i.e. the skill folder)
PostToolUse (Bash|PowerShell):
  - svg_to_pptx.py               -> re-inject the export promotion contract, but only
                                    when the command actually invoked the exporter AND
                                    a fresh .pptx is on disk. A crash, a --help, or a
                                    grep that merely names the script produces no file,
                                    so the hook says nothing rather than asserting an
                                    export that did not happen.

The former paragraph-split lint (svg_paragraph_lint.py) was retired at the
52e85a0 vendor refresh: the vendor's own checker now reports the same
sibling-<text> paragraph runs via _check_fragmented_paragraph_text, with
exclusions ours lacked (preserved source txbody, list markers, sentence
terminators, hidden elements). Upstream classes it advisory, so AgentFrame's
"fix it before export" stance lives as an overlay rule in AGENTS.md rather than
as a second, cruder detector here.

Unparseable or unrelated command shapes pass through. `af doctor` remains the
backstop for staging failures.
"""

from __future__ import annotations

import json
import re
import shlex
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

STAGING_REASON = (
    "PPT Master staging guard: this init would create the deck project at {target} - "
    "inside the vendored skill tree, where it gets orphaned and gitignored. Per "
    "system/vendor/ppt-master/AGENTS.md, stage deck projects inside the calling "
    "campaign (workspace/projects/<slug>/phase-4-production/decks/<deck-name>/) or "
    "C:\\tmp for throwaway runs. Re-run with an explicit --dir, e.g.: "
    "python scripts/project_manager.py init <name> --dir \"<absolute path outside system/>\""
)

PROMOTE_CONTEXT = (
    "Deck export finished. Overlay contract (system/vendor/ppt-master/AGENTS.md, "
    "Outputs): promote the new .pptx - copy it from the working folder's exports/ "
    "into the calling deliverable folder, keeping its timestamped filename. The "
    "exports/ twin stays frozen as the agent's reference. Versioning and round-trip "
    "rules: library/process/deck-production.md."
)


def _tokens(command: str) -> list[str]:
    try:
        return shlex.split(command, posix=True)
    except ValueError:
        return command.split()


INTERPRETERS = {"python", "python3", "py", "python.exe", "python3.exe", "py.exe"}

# A completed export writes its .pptx into the project's exports/ folder. A long deck
# can take minutes, so the window is generous; it only has to exclude stale twins from
# an earlier run.
EXPORT_FRESH_WINDOW_S = 1800


def _script_index(tokens: list[str], name: str) -> int | None:
    """Index of a token that actually INVOKES `name`, not one that merely names it.

    `grep -rn spec_lock .../svg_to_pptx.py` carries the script name as data. Treating
    that as an export made the PostToolUse hook assert a finished deck on a search.
    A real invocation is either argv[0] or immediately preceded by an interpreter.
    """
    for i, token in enumerate(tokens):
        if not token.replace("\\", "/").endswith(name):
            continue
        if i == 0:
            return i
        prev = Path(tokens[i - 1].replace("\\", "/")).name.lower()
        if prev in INTERPRETERS:
            return i
    return None


def _fresh_export(project: Path) -> bool:
    """True when the project's exports/ holds a .pptx written by this run."""
    try:
        newest = max(
            (p.stat().st_mtime for p in (project / "exports").glob("*.pptx")),
            default=None,
        )
    except OSError:
        return False
    return newest is not None and (time.time() - newest) <= EXPORT_FRESH_WINDOW_S


def _exported_project(tokens: list[str], cwd: Path) -> Path | None:
    """Project directory for a command that ran the exporter, else None.

    A path we cannot resolve means we cannot observe the outcome, and the hook must
    stay silent rather than guess.
    """
    idx = _script_index(tokens, "svg_to_pptx.py")
    if idx is None:
        return None
    for token in tokens[idx + 1:]:
        if token.startswith("-"):
            continue
        candidate = cwd / token
        if candidate.is_dir():
            return candidate
    return None


def _is_under(path: Path, ancestor: Path) -> bool:
    try:
        path.resolve().relative_to(ancestor.resolve())
        return True
    except ValueError:
        return False


def _deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def _staging_guard(tokens: list[str], cwd: Path) -> dict | None:
    script_idx = _script_index(tokens, "project_manager.py")
    if script_idx is None or "init" not in tokens[script_idx + 1:]:
        return None

    target = None
    rest = tokens[script_idx + 1:]
    for i, tok in enumerate(rest):
        if tok == "--dir" and i + 1 < len(rest):
            target = rest[i + 1]
        elif tok.startswith("--dir="):
            target = tok.split("=", 1)[1]
    resolved = (cwd / target) if target else (cwd / "projects")

    if _is_under(resolved, ROOT / "system"):
        return _deny(STAGING_REASON.format(target=resolved))
    return None


def decide(payload: dict) -> dict | None:
    command = (payload.get("tool_input") or {}).get("command") or ""
    if "project_manager.py" not in command and "svg_to_pptx.py" not in command:
        return None
    cwd = Path(payload.get("cwd") or ".")
    event = payload.get("hook_event_name") or "PreToolUse"
    tokens = _tokens(command)

    if event == "PreToolUse":
        return _staging_guard(tokens, cwd)

    if event == "PostToolUse":
        project = _exported_project(tokens, cwd)
        if project is None or not _fresh_export(project):
            return None  # no observable export: say nothing rather than claim one
        return {
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": PROMOTE_CONTEXT,
            }
        }
    return None


def run(stdin_text: str) -> str | None:
    try:
        payload = json.loads(stdin_text)
    except Exception:
        return None
    try:
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
