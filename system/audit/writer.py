"""Deterministic SQLite writer for AgentFrame Marketing system changes.

Markdown remains canonical for campaign state and activity. The audit database is
only the narrow, append-only record of system/process/template/persona changes.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

PROJECT_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DB_PATH = PROJECT_ROOT / "system" / "audit" / "agentframe.db"
LEGACY_DB_PATH = PROJECT_ROOT / "system" / "audit" / "marketingos.db"
SCHEMA_PATH = PROJECT_ROOT / "system" / "audit" / "schema.sql"

ACTOR_VALUES = {"agent", "user", "system"}


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def _migrate_legacy_default_db(path: Path) -> None:
    if path != DEFAULT_DB_PATH or path.exists() or not LEGACY_DB_PATH.exists():
        return

    path.parent.mkdir(parents=True, exist_ok=True)
    LEGACY_DB_PATH.replace(path)
    for suffix in ("-wal", "-shm"):
        legacy_sidecar = Path(f"{LEGACY_DB_PATH}{suffix}")
        new_sidecar = Path(f"{path}{suffix}")
        if legacy_sidecar.exists() and not new_sidecar.exists():
            legacy_sidecar.replace(new_sidecar)


def _normalize_db_path(db_path: str | Path | None) -> Path:
    path = Path(db_path) if db_path is not None else DEFAULT_DB_PATH
    _migrate_legacy_default_db(path)
    return path


def _validate_actor(actor: str) -> None:
    if actor not in ACTOR_VALUES:
        allowed = ", ".join(sorted(ACTOR_VALUES))
        raise ValueError(f"actor must be one of: {allowed}")


def _normalize_payload(payload: dict[str, Any] | None) -> str:
    return json.dumps(payload or {}, sort_keys=True)


def _load_payload(*, payload_json: str, payload_json_file: str | None) -> dict[str, Any]:
    if payload_json_file is None:
        return json.loads(payload_json)

    return json.loads(Path(payload_json_file).read_text(encoding="utf-8-sig"))


def _connect(db_path: str | Path | None = None) -> sqlite3.Connection:
    path = _normalize_db_path(db_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    conn.row_factory = sqlite3.Row
    return conn


@contextmanager
def _connection(db_path: str | Path | None = None):
    conn = _connect(db_path)
    try:
        yield conn
    finally:
        conn.close()


def ensure_db(db_path: str | Path | None = None) -> Path:
    path = _normalize_db_path(db_path)
    schema = SCHEMA_PATH.read_text(encoding="utf-8")

    with _connection(path) as conn:
        conn.executescript(schema)
        conn.commit()

    return path


def append_system_change(
    *,
    db_path: str | Path | None = None,
    change_type: str,
    actor: str,
    mode: str | None = None,
    target_kind: str | None = None,
    target_path: str | None = None,
    campaign_slug: str | None = None,
    reason: str | None = None,
    summary: str | None = None,
    payload: dict[str, Any] | None = None,
    created_at: str | None = None,
    source: str = "live",
) -> int:
    if not change_type.strip():
        raise ValueError("change_type is required")
    if not ((reason and reason.strip()) or (summary and summary.strip())):
        raise ValueError("system_changes rows require a reason or summary")
    _validate_actor(actor)

    ensure_db(db_path)
    row_created_at = created_at or _utc_now()

    with _connection(db_path) as conn:
        cursor = conn.execute(
            """
            INSERT INTO system_changes (
                created_at,
                actor,
                mode,
                change_type,
                target_kind,
                target_path,
                campaign_slug,
                reason,
                summary,
                payload_json,
                source
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                row_created_at,
                actor,
                mode,
                change_type,
                target_kind,
                target_path,
                campaign_slug,
                reason,
                summary,
                _normalize_payload(payload),
                source,
            ),
        )
        conn.commit()
        return int(cursor.lastrowid)


def query_recent_system_changes(
    *,
    db_path: str | Path | None = None,
    limit: int = 20,
    target_path: str | None = None,
) -> list[dict[str, Any]]:
    ensure_db(db_path)
    if limit <= 0:
        raise ValueError("limit must be positive")

    query = """
        SELECT id, created_at, actor, mode, change_type, target_kind,
               target_path, campaign_slug, reason, summary, payload_json, source
        FROM system_changes
    """
    params: list[Any] = []
    if target_path is not None:
        query += " WHERE target_path = ?"
        params.append(target_path)
    query += " ORDER BY created_at DESC, id DESC LIMIT ?"
    params.append(limit)

    with _connection(db_path) as conn:
        rows = conn.execute(query, tuple(params)).fetchall()

    return [
        {
            "id": row["id"],
            "created_at": row["created_at"],
            "actor": row["actor"],
            "mode": row["mode"],
            "change_type": row["change_type"],
            "target_kind": row["target_kind"],
            "target_path": row["target_path"],
            "campaign_slug": row["campaign_slug"],
            "reason": row["reason"],
            "summary": row["summary"],
            "payload": json.loads(row["payload_json"]),
            "source": row["source"],
        }
        for row in rows
    ]


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="AgentFrame Marketing system-change audit writer")
    parser.add_argument(
        "--db-path",
        default=str(DEFAULT_DB_PATH),
        help="Override the SQLite database path",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("init", help="Create the audit database if needed")

    sc = subparsers.add_parser("system-change", help="Append a system-change row")
    sc.add_argument("--change-type", required=True)
    sc.add_argument("--actor", required=True)
    sc.add_argument("--mode")
    sc.add_argument("--target-kind")
    sc.add_argument("--target-path")
    sc.add_argument("--campaign-slug")
    sc.add_argument("--reason")
    sc.add_argument("--summary")
    sc.add_argument("--created-at")
    sc.add_argument("--source", default="live")
    sc.add_argument(
        "--payload-json",
        default="{}",
        help="JSON object string for payload_json",
    )
    sc.add_argument(
        "--payload-json-file",
        help="Read payload_json from a UTF-8 JSON file to avoid shell quoting issues",
    )

    return parser


def main() -> int:
    parser = _build_parser()
    args = parser.parse_args()
    db_path = Path(args.db_path)

    if args.command == "init":
        ensure_db(db_path)
        print(db_path)
        return 0

    if args.command == "system-change":
        payload = _load_payload(
            payload_json=args.payload_json,
            payload_json_file=args.payload_json_file,
        )
        row_id = append_system_change(
            db_path=db_path,
            change_type=args.change_type,
            actor=args.actor,
            mode=args.mode,
            target_kind=args.target_kind,
            target_path=args.target_path,
            campaign_slug=args.campaign_slug,
            reason=args.reason,
            summary=args.summary,
            payload=payload,
            created_at=args.created_at,
            source=args.source,
        )
        print(row_id)
        return 0

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
