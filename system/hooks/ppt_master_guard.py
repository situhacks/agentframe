#!/usr/bin/env python3
"""Deterministic guards around the vendored ppt-master skill.

The overlay rules in system/skills/ppt-master/AGENTS.md are read once at run
start and decay out of context over a long deck session. These hooks re-inject
each rule at the exact moment it is violated, so they hold regardless of
compaction. All guard logic lives here — vendored files are never patched
(refresh procedure: VENDOR.md). Wired via .claude/settings.json (tracked,
ships with the repo); command shapes matched here are pinned by
system/tests/test_ppt_master_guard.py — re-run after every vendor refresh.

PreToolUse (Bash|PowerShell):
  - project_manager.py init      -> deny when the project would land inside
                                    this repo's system/ tree (vendor default
                                    is cwd/projects, i.e. the skill folder)
  - svg_to_pptx.py <project>     -> paragraph-split lint over svg_output/;
                                    deny with the corrective authoring form.
                                    Escape hatch: AF_PPT_LINT=off in command.
PostToolUse (Bash|PowerShell):
  - svg_to_pptx.py               -> re-inject the export promotion contract.

Fail-open by design: unparseable payloads or command shapes pass through;
`af doctor` is the backstop that catches anything these guards miss.
"""

from __future__ import annotations

import json
import re
import shlex
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(Path(__file__).resolve().parent))
import svg_paragraph_lint as lint  # noqa: E402

LINT_OFF = re.compile(r"AF_PPT_LINT\s*=\s*['\"]?off", re.IGNORECASE)
MAX_REASON_FINDINGS = 6

STAGING_REASON = (
    "PPT Master staging guard: this init would create the deck project at {target} - "
    "inside the vendored skill tree, where it gets orphaned and gitignored. Per "
    "system/skills/ppt-master/AGENTS.md, stage deck projects inside the calling "
    "campaign (workspace/projects/<slug>/phase-4-production/decks/<deck-name>/) or "
    "C:\\tmp for throwaway runs. Re-run with an explicit --dir, e.g.: "
    "python scripts/project_manager.py init <name> --dir \"<absolute path outside system/>\""
)

LINT_REASON_HEAD = (
    "Paragraph-split lint: {n} block(s) in svg_output/ are authored as one <text> per "
    "visual line - the converter cannot merge sibling <text> elements, so each line "
    "exports as its own PowerPoint text box.\n"
)

LINT_REASON_TAIL = (
    "\nRewrite each flagged block as ONE <text> with dy-stacked <tspan> lines, e.g.:\n"
    '<text x="80" y="190" font-size="18" fill="#333333">\n'
    '  <tspan x="80" dy="0">First line of the paragraph</tspan>\n'
    '  <tspan x="80" dy="32">second line continues here</tspan>\n'
    "</text>\n"
    "Then re-run the export. If a flagged block is genuinely separate labels (not a "
    "paragraph), prefix the command with AF_PPT_LINT=off to skip this lint once."
)

PROMOTE_CONTEXT = (
    "Deck export finished. Overlay contract (system/skills/ppt-master/AGENTS.md, "
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
    script_idx = next(
        (i for i, t in enumerate(tokens) if t.replace("\\", "/").endswith("project_manager.py")),
        None,
    )
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


def _export_project_path(tokens: list[str], cwd: Path) -> Path | None:
    script_idx = next(
        (i for i, t in enumerate(tokens) if t.replace("\\", "/").endswith("svg_to_pptx.py")),
        None,
    )
    if script_idx is None:
        return None
    nxt = tokens[script_idx + 1] if script_idx + 1 < len(tokens) else None
    if nxt is None or nxt.startswith("-"):
        return None  # fail open: unusual shape, af doctor backstops
    return cwd / nxt


def _export_lint(command: str, tokens: list[str], cwd: Path) -> dict | None:
    if LINT_OFF.search(command):
        return None
    project = _export_project_path(tokens, cwd)
    if project is None:
        return None
    svg_dir = project / "svg_output"
    if not svg_dir.is_dir():
        return None
    findings = lint.check_paths([svg_dir])
    if not findings:
        return None
    shown = findings[:MAX_REASON_FINDINGS]
    if len(findings) > len(shown):
        shown.append(f"... +{len(findings) - len(shown)} more")
    return _deny(
        LINT_REASON_HEAD.format(n=len(findings)) + "\n".join(shown) + LINT_REASON_TAIL
    )


def decide(payload: dict) -> dict | None:
    command = (payload.get("tool_input") or {}).get("command") or ""
    if "project_manager.py" not in command and "svg_to_pptx.py" not in command:
        return None
    cwd = Path(payload.get("cwd") or ".")
    event = payload.get("hook_event_name") or "PreToolUse"
    tokens = _tokens(command)

    if event == "PreToolUse":
        return _staging_guard(tokens, cwd) or _export_lint(command, tokens, cwd)

    if event == "PostToolUse" and "svg_to_pptx.py" in command:
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
        return None  # fail open, never block unrelated work on a guard bug
    return json.dumps(result) if result else None


if __name__ == "__main__":
    out = run(sys.stdin.read())
    if out:
        sys.stdout.write(out)
