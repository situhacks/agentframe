#!/usr/bin/env python3
"""Inject one exact, sealed bounded-autonomy contract at session start.

The hook grants no authority by itself. It only rehydrates a run that was
started through ``af autonomy start`` with the same harness session id and whose
static contract plus frozen files still match its stored digest.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from system import autonomy_contract  # noqa: E402


PIN_LIMIT = 7500


def _split_frontmatter(text: str) -> str | None:
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 4)
    return text[4:end] if end >= 0 else None


def _session_id(payload: dict, harness: str) -> str | None:
    if harness == "cursor":
        return payload.get("session_id") or payload.get("conversation_id")
    return payload.get("session_id")


def _binding(payload: dict, harness: str) -> str | None:
    session_id = _session_id(payload, harness)
    if not session_id:
        return None
    try:
        return autonomy_contract.normalize_session_binding(
            f"{harness}:{session_id}"
        )
    except ValueError:
        return None


def _running_matches(binding: str) -> list[tuple[Path, str]]:
    matches = []
    projects = ROOT / "workspace" / "projects"
    if not projects.is_dir():
        return matches
    for path in projects.rglob("knowledge/autonomy/*.md"):
        try:
            fm = _split_frontmatter(path.read_text(encoding="utf-8-sig"))
        except OSError:
            continue
        if (
            fm
            and autonomy_contract.scalar(fm, "schema_version")
            == autonomy_contract.SCHEMA_VERSION
            and autonomy_contract.scalar(fm, "status") == "running"
            and autonomy_contract.scalar(fm, "bound_session") == binding
        ):
            matches.append((path, fm))
    return matches


def _session_key_context(binding: str | None, diagnosis: str | None = None) -> str:
    key = binding or "unavailable"
    text = (
        f"AgentFrame session binding key: {key}. This key is not autonomy "
        "authority. Only `python system/af.py autonomy start ... "
        f"--session-binding {key}` can bind a sealed run."
    )
    if diagnosis:
        text += f" No autonomy authority was loaded: {diagnosis}."
    return text


def _pin(path: Path, fm: str) -> str:
    level = autonomy_contract.scalar(fm, "autonomy_level")
    if level == "plan-only":
        authority = (
            "Plan and inspect only. Do not implement, mutate project artifacts, "
            "or cause external effects."
        )
    elif level == "assisted":
        authority = (
            "Execute only the approved work units inside allowed_paths. Stop for "
            "judgment outside the charter or for any prohibited effect."
        )
    else:
        authority = (
            "Execute approved work units and routine verification without "
            "mid-run questions. Stop on ambiguity outside the charter, budget "
            "exhaustion, failed verification, or any prohibited effect."
        )

    fields = {
        "run_file": path.relative_to(ROOT).as_posix(),
        "run_id": autonomy_contract.scalar(fm, "run_id"),
        "project": autonomy_contract.scalar(fm, "project"),
        "level": level,
        "goal": autonomy_contract.scalar(fm, "goal"),
        "done_when": autonomy_contract.scalar(fm, "done_when"),
        "allowed_paths": autonomy_contract.list_value(fm, "allowed_paths"),
        "verification": autonomy_contract.list_value(fm, "verification"),
        "max_iterations": autonomy_contract.scalar(fm, "max_iterations"),
        "max_subagents": autonomy_contract.scalar(fm, "max_subagents"),
        "completion_gate": autonomy_contract.scalar(fm, "completion_gate"),
        "frozen_context": autonomy_contract.list_value(fm, "frozen_context"),
        "contract_sha256": autonomy_contract.scalar(fm, "contract_sha256"),
        "prohibited_effects": autonomy_contract.list_value(
            fm, "prohibited_effects"
        ),
    }
    return (
        "AGENTFRAME BOUNDED AUTONOMY PIN\n"
        f"{authority}\n"
        f"{json.dumps(fields, ensure_ascii=False, sort_keys=True)}\n"
        "The hash is tamper evidence, not operator authentication. Re-read the "
        "run file before each state transition; use af autonomy checkpoint for "
        "all outcomes. Never infer permission to communicate externally, mark "
        "readiness/publication/completion, merge, purchase, change permissions, "
        "expand scope, or overwrite operator-edited artifacts."
    )


def context_for(payload: dict, harness: str) -> str:
    binding = _binding(payload, harness)
    if binding is None:
        return _session_key_context(None, "the harness supplied no usable session id")
    matches = _running_matches(binding)
    if len(matches) != 1:
        diagnosis = (
            "no exact running contract matched"
            if not matches
            else f"{len(matches)} running contracts matched the same session"
        )
        return _session_key_context(binding, diagnosis)

    path, fm = matches[0]
    stored, observed, issues = autonomy_contract.stored_and_observed(fm, ROOT)
    if issues or stored is None or observed is None or stored != observed:
        detail = "; ".join(issues) if issues else (
            f"contract hash drift stored={stored or 'null'} "
            f"observed={observed or 'unavailable'}"
        )
        return _session_key_context(binding, detail)

    pin = _pin(path, fm)
    if len(pin) > PIN_LIMIT:
        return _session_key_context(
            binding,
            f"the complete pin is {len(pin)} characters, above the {PIN_LIMIT} cap",
        )
    return pin


def _cursor_payload(payload: dict) -> bool:
    return bool(payload.get("cursor_version")) or payload.get("hook_event_name") == "sessionStart"


def run(stdin_text: str, harness: str) -> str:
    try:
        payload = json.loads(stdin_text)
        context = context_for(payload, harness)
    except Exception as exc:
        context = _session_key_context(None, f"guard error: {exc}")
    if harness == "cursor":
        return json.dumps({"additional_context": context})
    return json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": context,
            }
        }
    )


def dispatch(stdin_text: str, argv: list[str]) -> str:
    try:
        payload = json.loads(stdin_text)
    except Exception:
        return "{}"
    if _cursor_payload(payload) and "--cursor-native" not in argv:
        return "{}"
    harness = None
    for name in sorted(autonomy_contract.SESSION_HARNESSES):
        if name in argv:
            harness = name
    if harness is None:
        return "{}"
    return run(stdin_text, harness)


if __name__ == "__main__":
    sys.stdout.write(dispatch(sys.stdin.read(), sys.argv[1:]))
