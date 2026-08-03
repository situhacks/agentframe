"""Shared validation and hashing for AgentFrame bounded-autonomy contracts.

This module is deliberately read-only. ``system/af.py`` owns state transitions;
session and PPT hooks import these helpers so every enforcement surface computes
the same contract digest.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from pathlib import Path


SCHEMA_VERSION = "2026-08-03-v2"
SESSION_HARNESSES = {"claude", "codex", "cursor"}
PROHIBITED_EFFECTS = (
    "external-communications-or-transmission",
    "deliverable-readiness-or-publication",
    "project-completion",
    "merge",
    "purchases",
    "permission-changes",
    "scope-expansion",
    "overwrite-operator-edited-artifacts",
)
STATIC_FIELDS = (
    "run_id",
    "project",
    "autonomy_level",
    "goal",
    "done_when",
    "context_sources",
    "frozen_context",
    "allowed_paths",
    "verification",
    "max_iterations",
    "max_subagents",
    "planner_tier",
    "executor_tier",
    "reviewer_tier",
    "reviewer_mode",
    "completion_gate",
    "prohibited_effects",
    "bound_session",
)
LIST_FIELDS = {
    "context_sources",
    "frozen_context",
    "allowed_paths",
    "verification",
    "prohibited_effects",
}
INT_FIELDS = {"max_iterations", "max_subagents"}


def scalar(frontmatter: str, key: str) -> str | None:
    match = re.search(rf"^\s*{re.escape(key)}:[ \t]*(.*?)\s*$", frontmatter, re.M)
    if not match:
        return None
    value = re.sub(r"\s+#.*$", "", match.group(1)).strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        value = value[1:-1]
    return value


def list_value(frontmatter: str, key: str) -> list[str]:
    """Read either an inline or a simple top-level YAML string list."""
    inline = re.search(rf"^{re.escape(key)}:\s*\[(.*?)\]\s*$", frontmatter, re.M)
    if inline:
        return [
            item.strip().strip("'\"")
            for item in inline.group(1).split(",")
            if item.strip()
        ]
    block = re.search(
        rf"^{re.escape(key)}:\s*$\n((?:[ \t]+-[ \t]+.*(?:\n|$))*)",
        frontmatter,
        re.M,
    )
    if not block:
        return []
    return [
        line.strip()[2:].strip().strip("'\"")
        for line in block.group(1).splitlines()
        if line.strip().startswith("- ")
    ]


def canonical_json(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def normalize_session_binding(value: str) -> str:
    match = re.fullmatch(r"([a-z]+):([A-Za-z0-9][A-Za-z0-9._:-]{2,255})", value or "")
    if not match or match.group(1) not in SESSION_HARNESSES:
        raise ValueError(
            "session binding must be <claude|codex|cursor>:<session-id>"
        )
    return f"{match.group(1)}:{match.group(2)}"


def _is_reparse_point(path: Path) -> bool:
    try:
        stat = path.lstat()
    except OSError:
        return False
    attrs = getattr(stat, "st_file_attributes", 0)
    reparse_flag = getattr(os.stat_result, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)
    return path.is_symlink() or bool(attrs & reparse_flag)


def _has_reparse_component(root: Path, candidate: Path) -> bool:
    try:
        parts = candidate.relative_to(root).parts
    except ValueError:
        return True
    current = root
    for part in parts:
        current = current / part
        if _is_reparse_point(current):
            return True
    return False


def resolve_frozen_files(
    root: str | Path,
    raw_paths: list[str],
) -> tuple[list[tuple[str, bytes]], list[str]]:
    """Resolve repo-relative regular files without following indirections."""
    root = Path(root).resolve()
    files: list[tuple[str, bytes]] = []
    issues: list[str] = []
    seen: set[str] = set()
    for raw in raw_paths:
        normalized = raw.replace("\\", "/").strip()
        parts = normalized.split("/")
        if (
            not normalized
            or Path(normalized).is_absolute()
            or any(part in {"", ".", ".."} for part in parts)
        ):
            issues.append(f"frozen_context entry must be repo-relative: {raw!r}")
            continue
        candidate = root / normalized
        try:
            resolved = candidate.resolve(strict=True)
            relative = resolved.relative_to(root).as_posix()
        except (OSError, ValueError):
            issues.append(f"frozen_context file is missing or outside the repo: {normalized}")
            continue
        if relative in seen:
            issues.append(f"frozen_context contains duplicate path: {relative}")
            continue
        seen.add(relative)
        if relative != normalized:
            issues.append(
                f"frozen_context path must use its normalized repo-relative spelling: "
                f"{normalized} -> {relative}"
            )
            continue
        if _has_reparse_component(root, candidate):
            issues.append(
                f"frozen_context may not traverse a symlink/reparse point: {relative}"
            )
            continue
        if not resolved.is_file():
            issues.append(f"frozen_context entry is not a regular file: {relative}")
            continue
        try:
            files.append((relative, resolved.read_bytes()))
        except OSError as exc:
            issues.append(f"cannot read frozen_context file {relative}: {exc}")
    return files, issues


def contract_payload(
    frontmatter: str,
    root: str | Path,
) -> tuple[dict, list[str]]:
    issues: list[str] = []
    contract: dict[str, object] = {}
    for field in STATIC_FIELDS:
        if field in LIST_FIELDS:
            contract[field] = list_value(frontmatter, field)
        elif field in INT_FIELDS:
            raw = scalar(frontmatter, field)
            try:
                contract[field] = int(raw) if raw is not None else None
            except ValueError:
                contract[field] = raw
        else:
            contract[field] = scalar(frontmatter, field)

    frozen = contract.get("frozen_context")
    files, file_issues = resolve_frozen_files(
        root,
        frozen if isinstance(frozen, list) else [],
    )
    issues.extend(file_issues)
    payload = {
        "contract": contract,
        "frozen_files": [
            {"path": path, "sha256": sha256_bytes(data)}
            for path, data in files
        ],
    }
    return payload, issues


def observed_sha256(
    frontmatter: str,
    root: str | Path,
) -> tuple[str | None, list[str]]:
    payload, issues = contract_payload(frontmatter, root)
    if issues:
        return None, issues
    return sha256_bytes(canonical_json(payload)), []


def stored_and_observed(
    frontmatter: str,
    root: str | Path,
) -> tuple[str | None, str | None, list[str]]:
    stored = scalar(frontmatter, "contract_sha256")
    if stored in {None, "", "null"}:
        stored = None
    observed, issues = observed_sha256(frontmatter, root)
    return stored, observed, issues
