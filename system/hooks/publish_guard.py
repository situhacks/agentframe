#!/usr/bin/env python3
"""Cross-harness gate: unready copy must not reach an external publication surface.

``af publish`` refuses a non-ready head, but the button is not on the path that
actually ships an essay. Copy reaches Substack through the MCP server and the
operator publishes from the web editor, so every gate hanging off readiness --
the humanizer pass, the voice mini-retro, the template's own readiness criteria
-- is skipped by doing the obvious thing.

This hook intercepts the last moment AgentFrame can still see the content:
pushing a body into an existing Substack draft. It resolves the call's
``draft_id`` against ``substack_draft:`` in the project trackers and refuses when
the owning row sits below ``ready``.

Scope is deliberately narrow, because a gate that fires constantly gets bypassed:

  - ``update_draft`` carries ``draft_id``, a join key verified present on both
    sides. Only calls carrying title, subtitle, or body are content-bearing;
    an audience-only update passes.
  - ``create_draft`` has no key -- the tracker has no id until the draft exists
    -- so the first push is not gated here. ``af doctor``'s state-truth check
    reports that drift afterward, which is also the only thing that works when
    the push happens in the web UI, where no hook can see it.

Fail open on an unparseable payload, on a draft id no tracker claims, and on a
call with nothing content-bearing in it. ``af doctor`` remains the backstop.
"""

from __future__ import annotations

import glob
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(ROOT))
from system import af  # noqa: E402

EVENT_ALIASES = {
    "PreToolUse": "PreToolUse",
    "preToolUse": "PreToolUse",
}

# Harnesses spell the same MCP tool slightly differently; match the verb, not the prefix.
GATED_TOOL_RE = re.compile(r"substack.*update_draft$|update_draft.*substack", re.I)

CONTENT_FIELDS = ("body", "title", "subtitle")

# Trackers that can own a publication row.
TRACKER_GLOBS = (
    "workspace/projects/*/project.md",
    "workspace/pipeline/applications/*/application.md",
)

REASON = (
    "Substack push refused: {source} row '{slug}' is '{status}', not ready. Pushing the "
    "body into draft {draft_id} is the last point AgentFrame can see this copy - the "
    "operator publishes from the Substack editor after this, where nothing can check it. "
    "Readiness is what runs the humanizer pass, the voice mini-retro, and the template's "
    "readiness criteria; skipping it here skips all three on the one artifact that reaches "
    "the public feed.\n"
    "Run `python system/af.py ready {project} {slug}` first, or version and finish the head "
    "if it is not actually done. If this push is deliberate and the row is wrong, fix the "
    "row rather than working around this."
)


def _deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def _draft_owner(draft_id: str):
    """Return (source_label, project_slug, row_slug, status) for a tracked draft id."""
    for pattern in TRACKER_GLOBS:
        for path in sorted(glob.glob(str(ROOT / pattern))):
            try:
                fm, _ = af.split_fm(af.read(path), path)
            except Exception:
                continue
            for slug in af.all_rows(fm):
                span = af.row_span(fm, slug)
                if not span:
                    continue
                block = fm[span[0]:span[1]]
                for field, value in af.PUBLISHED_FIELD_RE.findall(block):
                    if not field.endswith("_draft"):
                        continue
                    if str(af.clean_value(value)).strip() != draft_id:
                        continue
                    return (
                        os.path.basename(path),
                        os.path.basename(os.path.dirname(path)),
                        slug,
                        af.row_get(fm, slug, "status"),
                    )
    return None


def decide(payload: dict) -> dict | None:
    if EVENT_ALIASES.get(payload.get("hook_event_name") or "PreToolUse") != "PreToolUse":
        return None
    tool = payload.get("tool_name") or ""
    if not GATED_TOOL_RE.search(tool):
        return None

    tool_input = payload.get("tool_input") or {}
    if not any(str(tool_input.get(f) or "").strip() for f in CONTENT_FIELDS):
        return None  # metadata-only update carries no copy

    raw = tool_input.get("draft_id")
    if raw in (None, ""):
        return None
    draft_id = str(raw).strip()
    if draft_id.endswith(".0"):  # JSON numbers arrive as floats on some harnesses
        draft_id = draft_id[:-2]

    owner = _draft_owner(draft_id)
    if owner is None:
        return None  # no tracker claims this draft: not ours to gate
    source, project, slug, status = owner
    if status in ("ready", "published"):
        return None
    return _deny(
        REASON.format(
            source=source,
            slug=slug,
            status=status,
            draft_id=draft_id,
            project=project,
        )
    )


def _cursor_payload(payload: dict) -> bool:
    return bool(payload.get("cursor_version")) or payload.get("hook_event_name") == "preToolUse"


def _adapt_result(payload: dict, result: dict | None) -> dict | None:
    if not result or not _cursor_payload(payload):
        return result
    reason = result["hookSpecificOutput"]["permissionDecisionReason"]
    return {"permission": "deny", "user_message": reason, "agent_message": reason}


def run(stdin_text: str) -> str | None:
    try:
        payload = json.loads(stdin_text)
        result = _adapt_result(payload, decide(payload))
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
    return run(stdin_text) or "{}"


if __name__ == "__main__":
    sys.stdout.write(dispatch(sys.stdin.read(), sys.argv[1:]))
