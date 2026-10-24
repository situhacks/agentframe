#!/usr/bin/env python3
"""Gate on the voice lint around the state buttons.

The zero-budget voice rules (banned tics, two dashes in a sentence, the
operator's hyphen turned into an em dash) kept shipping through late agent
passes that never ran the clean pass. Prose could not stop that; this hook
runs `system/voice_lint.py` at the two moments every draft passes through.

PreToolUse (Bash|PowerShell), `python system/af.py ready <project> <deliverable>`:
  deny when the head is user-voiced and the lint has hard findings; otherwise
  pass, attaching the soft report as context. `AF_VOICE_LINT=skip` anywhere on
  the command line is the operator's override and passes silently.

PostToolUse (Bash|PowerShell), `python system/af.py version <project> <deliverable>`:
  the new head holds the text of the version just closed, so lint it against the
  version before that and remind the agent to load the voice system before it
  edits. Never blocks.

Anything else passes through, and every error of our own fails open. `af doctor`
and the separate clean pass remain the backstops.
"""

from __future__ import annotations

import json
import os
import re
import shlex
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from system import voice_lint  # noqa: E402

INTERPRETERS = {"python", "python3", "py", "python.exe", "python3.exe", "py.exe"}
SKIP = re.compile(r"AF_VOICE_LINT\s*=\s*['\"]?skip", re.I)

READY_DENY = (
    "{report}\n"
    "Fix the hard findings in the head (anti-patterns.md § Banned tics and § Punctuation), "
    "then run af ready again. Operator override: prefix the command with AF_VOICE_LINT=skip."
)
VERSION_CONTEXT = (
    "{report}\n"
    "{new} now holds that text. If a VOICE BUNDLE for this register is already in your context with no "
    "compaction since, it is current: continue. Otherwise run `python system/af.py voice bundle {project} {deliverable} "
    "--tier core` for a copyedit pass over his text, or without --tier before drafting a new section, and read the "
    "file it names whole (voice/README.md and the rest of the voice system in one read). Hard findings above are "
    "yours to fix in this version, and every region you rewrite gets anti-patterns.md before it is surfaced."
)


def _tokens(command: str) -> list[str]:
    try:
        return shlex.split(command, posix=True)
    except ValueError:
        return command.split()


def _af_call(tokens: list[str]) -> tuple[str, str, str] | None:
    """(verb, project, deliverable) when the command actually invokes af.py."""
    for i, token in enumerate(tokens):
        norm = token.replace("\\", "/")
        if not (norm == "af.py" or norm.endswith("/af.py")):
            continue
        prev = Path(tokens[i - 1].replace("\\", "/")).name.lower() if i > 0 else None
        if i != 0 and prev not in INTERPRETERS:
            return None
        rest = [t for t in tokens[i + 1:] if not t.startswith("-") and t not in ("&&", ";", "|", "||")]
        if len(rest) < 3:
            return None
        return rest[0], rest[1], rest[2]
    return None


def _tool_output(payload: dict) -> str | None:
    resp = payload.get("tool_response")
    if resp is None:
        return None
    if isinstance(resp, dict):
        return " ".join(str(v) for v in resp.values() if isinstance(v, str))
    return str(resp)


def _deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def _context(event: str, text: str) -> dict:
    return {"hookSpecificOutput": {"hookEventName": event, "additionalContext": text}}


def decide(payload: dict) -> dict | None:
    command = (payload.get("tool_input") or {}).get("command") or ""
    if "af.py" not in command.replace("\\", "/"):
        return None
    event = payload.get("hook_event_name") or "PreToolUse"
    call = _af_call(_tokens(command))
    if call is None:
        return None
    verb, project, deliverable = call

    if verb == "ready" and event == "PreToolUse":
        if SKIP.search(command):
            return None
        head = voice_lint.resolve_head(project, deliverable)
        if not head:
            return None
        result = voice_lint.lint_file(head, prev="auto")
        if not result["user_voiced"]:
            return None
        report = voice_lint.format_report(result)
        if result["hard"]:
            return _deny(READY_DENY.format(report=report))
        if result["soft"]:
            return _context("PreToolUse", report)
        return None

    if verb == "version" and event == "PostToolUse":
        output = _tool_output(payload)
        if output is not None and "af version:" not in output:
            return None  # the button refused; nothing new to lint
        head = voice_lint.resolve_head(project, deliverable)
        if not head:
            return None
        closed = voice_lint.previous_version(head)  # the version just closed; same text as the head
        before = voice_lint.previous_version(closed) if closed else None
        result = voice_lint.lint_file(head, prev=before)
        if not result["user_voiced"]:
            return None
        result["path"] = closed or head
        report = voice_lint.format_report(result)
        return _context("PostToolUse", VERSION_CONTEXT.format(report=report, new=os.path.basename(head), project=project, deliverable=deliverable))

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
