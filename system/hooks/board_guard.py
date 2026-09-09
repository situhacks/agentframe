#!/usr/bin/env python3
"""Tokenless board maintenance from harness hooks.

``--event sessionstart``: run one deterministic ``af board sync`` and print at
most one short line when cards are waiting on the operator.

``--event stop``: if the stopping session owns an orchestrator-dispatched card
that is still In progress and no receipt exists, write a backstop ``blocked``
receipt from the last assistant message, then sync. Human-driven chats are
never touched here; their staleness is judged by the sync's time rule.

The hook never blocks and never fails the session: every path exits 0.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from system import board as workboard  # noqa: E402


def _payload() -> dict:
    try:
        raw = sys.stdin.read()
    except OSError:
        return {}
    if not raw or not raw.strip():
        return {}
    try:
        data = json.loads(raw)
    except ValueError:
        return {}
    return data if isinstance(data, dict) else {}


def _session_id(payload: dict) -> str | None:
    value = payload.get("session_id") or payload.get("conversation_id")
    return str(value) if value else None


def backstop_receipt(root: Path, session_id: str, payload: dict) -> str | None:
    board = workboard.load(root, strict=False)
    for card in board.lanes["In progress"]:
        prefix = card.fields.get("session")
        if card.fields.get("by", "orchestrator") != "orchestrator" or not prefix:
            continue
        if not session_id.startswith(prefix):
            continue
        receipt = workboard.paths(root)["receipts"] / f"{card.id}.result.json"
        if receipt.exists():
            return None
        last = " ".join(str(payload.get("last_assistant_message") or "").split())
        summary = last[:300] or "worker stopped without writing a receipt"
        workboard.write_receipt(root, card.id, session=card.owner, status="blocked",
                                summary=summary, auto="stop-hook")
        return card.id
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description="AgentFrame board hook")
    ap.add_argument("--event", choices=("sessionstart", "stop"), required=True)
    ap.add_argument("--harness", default="claude")
    ap.add_argument("--cursor-native", action="store_true")
    args = ap.parse_args()
    try:
        if not workboard.exists(ROOT):
            return 0
        payload = _payload()
        if args.event == "stop":
            session_id = _session_id(payload)
            if session_id:
                backstop_receipt(ROOT, session_id, payload)
        board = workboard.load(ROOT, strict=False)
        rows = workboard.roster(ROOT, timeout=6.0)
        workboard.sync(ROOT, board, rows=rows)
        workboard.save(ROOT, board)
        if args.event == "sessionstart":
            waiting = len(board.lanes["Needs you"])
            if waiting:
                print(f"AgentFrame board: {waiting} card(s) need you (python system/af.py board list).")
    except Exception:  # hooks never fail the session
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
