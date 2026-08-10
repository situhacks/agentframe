#!/usr/bin/env python3
"""Cross-harness deterministic guards around the vendored ppt-master skill.

The overlay rules in system/skills/ppt-master/AGENTS.md are read once at run
start and decay out of context over a long deck session. These hooks re-inject
each rule at the exact moment it is violated, so they hold regardless of
compaction. All guard logic lives here — vendored files are never patched
(refresh procedure: VENDOR.md). Native wiring lives in the tracked Claude,
Cursor, and Codex project configs; command shapes matched here are pinned by
system/tests/test_ppt_master_guard.py — re-run after every vendor refresh.

PreToolUse (Bash|PowerShell):
  - project_manager.py init      -> deny when the project would land inside
                                    this repo's system/ tree (vendor default
                                    is cwd/projects, i.e. the skill folder)
PostToolUse (Bash|PowerShell):
  - svg_to_pptx.py               -> re-inject the export promotion contract.

The former paragraph-split lint (svg_paragraph_lint.py) was retired at the
52e85a0 vendor refresh: the vendor's own checker now reports the same
sibling-<text> paragraph runs via _check_fragmented_paragraph_text, with
exclusions ours lacked (preserved source txbody, list markers, sentence
terminators, hidden elements). Upstream classes it advisory, so AgentFrame's
"fix it before export" stance lives as an overlay rule in AGENTS.md rather than
as a second, cruder detector here.

Unparseable or unrelated command shapes pass through. Once a repo-contained
project declares an active sealed confirmation, its recognized confirm/export
commands fail closed on malformed contracts, drift, or session mismatch.
`af doctor` remains the backstop for unrelated staging failures.
"""

from __future__ import annotations

import json
import re
import shlex
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from system.tools import ppt_master_contract as contract  # noqa: E402

EVENT_ALIASES = {
    "PreToolUse": "PreToolUse",
    "preToolUse": "PreToolUse",
    "beforeShellExecution": "PreToolUse",
    "PostToolUse": "PostToolUse",
    "postToolUse": "PostToolUse",
    "afterShellExecution": "PostToolUse",
}

STAGING_REASON = (
    "PPT Master staging guard: this init would create the deck project at {target} - "
    "inside the vendored skill tree, where it gets orphaned and gitignored. Per "
    "system/skills/ppt-master/AGENTS.md, stage deck projects inside the calling "
    "campaign (workspace/projects/<slug>/phase-4-production/decks/<deck-name>/) or "
    "C:\\tmp for throwaway runs. Re-run with an explicit --dir, e.g.: "
    "python scripts/project_manager.py init <name> --dir \"<absolute path outside system/>\""
)

PROMOTE_CONTEXT = (
    "Deck export finished. Overlay contract (system/skills/ppt-master/AGENTS.md, "
    "Outputs): promote the new .pptx - copy it from the working folder's exports/ "
    "into the calling deliverable folder, keeping its timestamped filename. The "
    "exports/ twin stays frozen as the agent's reference. Versioning and round-trip "
    "rules: library/process/deck-production.md."
)

CONTRACT_VALID_CONTEXT = (
    "AgentFrame found a valid sealed Strategist confirmation at {path}. The "
    "interactive confirmation server is redundant and must not launch. Run "
    "`python system/tools/ppt_master_contract.py materialize \"{path}\" "
    "--session-binding {binding}`, read the result, and continue the vendor "
    "workflow after the confirmation gate."
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


def _confirm_project_path(tokens: list[str], cwd: Path) -> Path | None:
    script_idx = next(
        (
            i for i, token in enumerate(tokens)
            if token.replace("\\", "/").endswith("confirm_ui/server.py")
        ),
        None,
    )
    if script_idx is None or script_idx + 1 >= len(tokens):
        return None
    raw = tokens[script_idx + 1]
    if raw.startswith("-"):
        return None
    rest = tokens[script_idx + 2:]
    if "--shutdown" in rest:
        return None
    launch_stage1 = "--daemon" in rest and "--wait" in rest and "--wait-only" not in rest
    # Vendor 52e85a0 collapsed the confirmation to two waits: --wait-stage accepts
    # only {stage1, final} and defaults to final. The pre-refresh "stage2" wait no
    # longer exists upstream, so treat any recognized stage as a guarded wait.
    stage_arg = None
    if "--wait-stage" in rest:
        after = rest[rest.index("--wait-stage") + 1:rest.index("--wait-stage") + 2]
        stage_arg = after[0] if after else None
    wait_only = "--wait-only" in rest and stage_arg in (None, "stage1", "final")
    if not (launch_stage1 or wait_only):
        return None
    return cwd / raw


def _payload_binding(payload: dict, harness: str | None) -> str | None:
    if harness not in {"claude", "codex", "cursor"}:
        return None
    if harness == "cursor":
        session_id = payload.get("session_id") or payload.get("conversation_id")
    else:
        session_id = payload.get("session_id")
    if not session_id:
        return None
    try:
        return contract.autonomy_contract.normalize_session_binding(
            f"{harness}:{session_id}"
        )
    except ValueError:
        return None


def _contract_candidates(project: Path) -> list[Path]:
    if not _is_under(project, ROOT):
        return []
    return sorted(project.parent.glob(f"{project.name}.*{contract.SUFFIX}"))


def _candidate_owner(candidate: Path, project: Path) -> tuple[str | None, str | None]:
    """Return the owning run's status and binding, even for malformed wrappers."""
    run_id = candidate.name[
        len(project.name) + 1:-len(contract.SUFFIX)
    ]
    try:
        projects = ROOT / "workspace" / "projects"
        project_rel = project.resolve().relative_to(projects.resolve())
        owner = projects / project_rel.parts[0]
        run = owner / "knowledge" / "autonomy" / f"{run_id}.md"
        text = run.read_text(encoding="utf-8-sig")
    except (OSError, ValueError, IndexError):
        return None, None
    if not text.startswith("---\n"):
        return None, None
    end = text.find("\n---\n", 4)
    if end < 0:
        return None, None
    fm = text[4:end]
    return (
        contract.autonomy_contract.scalar(fm, "status"),
        contract.autonomy_contract.scalar(fm, "bound_session"),
    )


def _contract_guard(
    project: Path,
    payload: dict,
    harness: str | None,
    *,
    confirmation_launch: bool,
) -> dict | None:
    candidates = _contract_candidates(project)
    if not candidates:
        return None
    binding = _payload_binding(payload, harness)
    running = []
    for candidate in candidates:
        status, owner_binding = _candidate_owner(candidate, project)
        if status == "running":
            running.append((candidate, owner_binding))
    if not running:
        return None  # historical sealed records do not suppress the normal UI
    if binding is None:
        return _deny(
            "A sealed PPT confirmation was declared, but this hook payload has no "
            "trusted harness session id. Noninteractive confirmation is unsupported "
            "on this surface; use the normal vendor gate."
        )
    active = [
        candidate for candidate, owner_binding in running
        if owner_binding == binding
    ]
    if binding is not None and not active:
        return _deny(
            "A sealed PPT confirmation is active for this project, but it is bound "
            "to a different harness session. Use the owning session or the normal "
            "vendor gate after that run is blocked/completed."
        )
    if len(active) != 1:
        return _deny(
            f"PPT confirmation contract is ambiguous: found {len(active)} active "
            f"sealed siblings for {project} and this session."
        )
    path = active[0]
    try:
        contract.validate_contract(
            path,
            expected_session=binding,
            require_materialized=not confirmation_launch,
        )
    except Exception as exc:
        return _deny(f"Sealed PPT confirmation is invalid: {exc}")
    if confirmation_launch:
        return _deny(CONTRACT_VALID_CONTEXT.format(path=path, binding=binding))
    return None


def decide(payload: dict, harness: str | None = None) -> dict | None:
    command = (
        (payload.get("tool_input") or {}).get("command")
        or payload.get("command")
        or ""
    )
    if (
        "project_manager.py" not in command
        and "svg_to_pptx.py" not in command
        and "confirm_ui/server.py" not in command.replace("\\", "/")
    ):
        return None
    cwd = Path(payload.get("cwd") or ".")
    event = EVENT_ALIASES.get(payload.get("hook_event_name") or "PreToolUse")
    tokens = _tokens(command)

    if event == "PreToolUse":
        confirm_project = _confirm_project_path(tokens, cwd)
        if confirm_project is not None:
            result = _contract_guard(
                confirm_project,
                payload,
                harness,
                confirmation_launch=True,
            )
            if result:
                return result
        export_project = _export_project_path(tokens, cwd)
        if export_project is not None:
            result = _contract_guard(
                export_project,
                payload,
                harness,
                confirmation_launch=False,
            )
            if result:
                return result
        return _staging_guard(tokens, cwd)

    if event == "PostToolUse" and "svg_to_pptx.py" in command:
        return {
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": PROMOTE_CONTEXT,
            }
        }
    return None


def _cursor_payload(payload: dict) -> bool:
    return bool(payload.get("cursor_version")) or payload.get("hook_event_name") in {
        "preToolUse",
        "postToolUse",
        "beforeShellExecution",
        "afterShellExecution",
    }


def _adapt_result(payload: dict, result: dict | None) -> dict | None:
    if not result or not _cursor_payload(payload):
        return result
    output = result.get("hookSpecificOutput") or {}
    decision = output.get("permissionDecision")
    if decision:
        reason = output.get("permissionDecisionReason") or "Blocked by AgentFrame policy."
        return {
            "permission": decision,
            "user_message": reason,
            "agent_message": reason,
        }
    context = output.get("additionalContext")
    if context:
        return {"additional_context": context}
    return result


def _recognized_active_contract(payload: dict) -> bool:
    """Best-effort discriminator for the guard's narrow fail-closed surface."""
    command = (
        (payload.get("tool_input") or {}).get("command")
        or payload.get("command")
        or ""
    )
    tokens = _tokens(command)
    cwd = Path(payload.get("cwd") or ".")
    project = (
        _confirm_project_path(tokens, cwd)
        or _export_project_path(tokens, cwd)
    )
    if project is None:
        return False
    for candidate in _contract_candidates(project):
        status, _ = _candidate_owner(candidate, project)
        if status == "running":
            return True
    return False


def run(stdin_text: str, harness: str | None = None) -> str | None:
    try:
        payload = json.loads(stdin_text)
    except Exception:
        return None
    try:
        result = _adapt_result(payload, decide(payload, harness=harness))
    except Exception as exc:
        try:
            if _recognized_active_contract(payload):
                result = _adapt_result(
                    payload,
                    _deny(f"Active sealed PPT confirmation guard failed: {exc}"),
                )
            else:
                return None
        except Exception:
            return None
    return json.dumps(result) if result else None


def dispatch(stdin_text: str, argv: list[str]) -> str:
    """Render one process response and suppress Cursor's imported Claude twin."""
    try:
        payload = json.loads(stdin_text)
    except Exception:
        return "{}"
    if _cursor_payload(payload) and "--cursor-native" not in argv:
        return "{}"
    harness = next(
        (name for name in ("claude", "codex", "cursor") if name in argv),
        None,
    )
    return run(stdin_text, harness=harness) or "{}"


if __name__ == "__main__":
    sys.stdout.write(dispatch(sys.stdin.read(), sys.argv[1:]))
