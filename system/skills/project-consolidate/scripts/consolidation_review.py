#!/usr/bin/env python3
"""Ignored-file-safe review and rollback for project consolidation runs."""

from __future__ import annotations

import argparse
import datetime
import difflib
import hashlib
import json
import os
import re
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[4]
SNAPSHOT_MANIFEST = "snapshot-manifest.json"
REVIEW_MANIFEST = "review-manifest.json"
PROMOTION_MANIFEST = "promotion-manifest.json"
SNAPSHOT_SCHEMA = 1
REVIEW_SCHEMA = 2
SLUG_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


class ReviewError(RuntimeError):
    """A safe, user-actionable consolidation review failure."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _utc_now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


def _is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def _is_linklike(path: Path) -> bool:
    junction_check = getattr(path, "is_junction", None)
    return path.is_symlink() or bool(junction_check and junction_check())


def _validated_root(root: Path) -> Path:
    root = root.expanduser().resolve()
    if not (root / "workspace" / "projects").is_dir():
        raise ReviewError(f"AgentFrame project root not found under {root}")
    if not (root / "library" / "context").is_dir():
        raise ReviewError(f"AgentFrame operator context not found under {root}")
    return root


def _validated_slug(slug: str) -> str:
    if not isinstance(slug, str) or not SLUG_RE.fullmatch(slug):
        raise ReviewError(f"Invalid project slug: {slug!r}")
    return slug


def resolve_project_scope(root: Path, slug: str) -> Path:
    """Resolve exactly one active or completed project directory."""
    root = _validated_root(root)
    slug = _validated_slug(slug)
    candidates = [
        root / "workspace" / "projects" / slug,
        root / "workspace" / "projects" / "completed" / slug,
    ]
    matches = [path for path in candidates if (path / "project.md").is_file()]
    if not matches:
        raise ReviewError(
            f"Project {slug!r} was not found as an active or completed project"
        )
    if len(matches) > 1:
        raise ReviewError(
            f"Project {slug!r} exists in both active and completed locations"
        )
    return matches[0].resolve()


def _scope_rel(root: Path, path: Path) -> str:
    resolved = path.resolve()
    if not _is_relative_to(resolved, root):
        raise ReviewError(f"Scope resolves outside the AgentFrame root: {path}")
    return resolved.relative_to(root).as_posix()


def allowed_scopes(
    root: Path, slug: str, include_operator_context: bool = False
) -> list[Path]:
    """Return the named project plus the explicit global write scope."""
    root = _validated_root(root)
    project = resolve_project_scope(root, slug)
    context = root / "library" / "context"
    global_scope = context if include_operator_context else context / "people"
    if global_scope.exists() and not global_scope.is_dir():
        raise ReviewError(f"Global context scope is not a directory: {global_scope}")
    return [project, global_scope.resolve()]


def _assert_no_symlink_parts(root: Path, path: Path) -> None:
    """Reject symlink/junction-like escapes before reading or restoring a file."""
    root = root.resolve()
    lexical = Path(os.path.abspath(path))
    if not _is_relative_to(lexical, root):
        raise ReviewError(f"Path is outside the AgentFrame root: {path}")
    cursor = root
    for part in lexical.relative_to(root).parts:
        cursor = cursor / part
        if cursor.exists() and _is_linklike(cursor):
            raise ReviewError(f"Symlinks are not allowed in review scopes: {cursor}")
    resolved = lexical.resolve(strict=False)
    if not _is_relative_to(resolved, root):
        raise ReviewError(f"Path resolves outside the AgentFrame root: {path}")


def collect_markdown(root: Path, scopes: list[Path]) -> dict[str, bytes]:
    """Read every Markdown file in the explicit scopes, without consulting Git."""
    root = root.resolve()
    found: dict[str, bytes] = {}
    for scope in scopes:
        scope = Path(os.path.abspath(scope))
        if not scope.exists():
            continue
        _assert_no_symlink_parts(root, scope)
        resolved_scope = scope.resolve()
        for dirpath, dirnames, filenames in os.walk(scope, followlinks=False):
            base = Path(dirpath)
            for dirname in list(dirnames):
                candidate = base / dirname
                if _is_linklike(candidate):
                    raise ReviewError(
                        f"Symlinked directories are not allowed in review scopes: {candidate}"
                    )
            for filename in filenames:
                if Path(filename).suffix.lower() != ".md":
                    continue
                path = base / filename
                _assert_no_symlink_parts(root, path)
                if not _is_relative_to(path.resolve(), resolved_scope):
                    raise ReviewError(f"Markdown path escapes its review scope: {path}")
                if _is_linklike(path) or not path.is_file():
                    raise ReviewError(f"Unsafe Markdown path in review scope: {path}")
                rel = path.relative_to(root).as_posix()
                if rel in found:
                    raise ReviewError(f"Overlapping review scopes include {rel} twice")
                found[rel] = path.read_bytes()
    return found


def _write_json(path: Path, value: dict) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def snapshot_state(
    root: Path,
    project_slug: str,
    run_dir: Path | None = None,
    include_operator_context: bool = False,
) -> Path:
    """Create an immutable-by-hash before image for a single run."""
    root = _validated_root(root)
    scopes = allowed_scopes(root, project_slug, include_operator_context)
    if run_dir is None:
        run_dir = Path(
            tempfile.mkdtemp(prefix=f"agentframe-consolidate-{project_slug}-")
        )
    else:
        run_dir = run_dir.expanduser().resolve()
        for scope in scopes:
            if _is_relative_to(run_dir, scope):
                raise ReviewError(
                    f"Run directory cannot be inside a review scope: {run_dir}"
                )
        if run_dir.exists() and any(run_dir.iterdir()):
            raise ReviewError(f"Run directory must be empty: {run_dir}")
        run_dir.mkdir(parents=True, exist_ok=True)

    files = collect_markdown(root, scopes)
    before_root = run_dir / "before"
    entries: dict[str, dict[str, object]] = {}
    for rel, data in sorted(files.items()):
        destination = before_root.joinpath(*Path(rel).parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        entries[rel] = {"sha256": sha256_bytes(data), "size": len(data)}

    manifest = {
        "schema_version": SNAPSHOT_SCHEMA,
        "created_at": _utc_now(),
        "root": str(root),
        "project": project_slug,
        "scopes": [_scope_rel(root, scope) for scope in scopes],
        "files": entries,
    }
    _write_json(run_dir / SNAPSHOT_MANIFEST, manifest)
    return run_dir


def _safe_manifest_rel(value: object) -> str:
    if not isinstance(value, str):
        raise ReviewError("Manifest paths must be strings")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or path.suffix.lower() != ".md":
        raise ReviewError(f"Unsafe Markdown path in manifest: {value!r}")
    return path.as_posix()


def load_snapshot(run_dir: Path) -> dict:
    run_dir = run_dir.expanduser().resolve()
    path = run_dir / SNAPSHOT_MANIFEST
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReviewError(f"Cannot read snapshot manifest {path}: {exc}") from exc
    if manifest.get("schema_version") != SNAPSHOT_SCHEMA:
        raise ReviewError("Unsupported snapshot manifest schema")

    raw_root = manifest.get("root")
    if not isinstance(raw_root, str) or not Path(raw_root).is_absolute():
        raise ReviewError("Snapshot manifest is missing an absolute AgentFrame root")
    root = _validated_root(Path(raw_root))
    slug = _validated_slug(manifest.get("project", ""))
    scopes = manifest.get("scopes")
    files = manifest.get("files")
    if not isinstance(scopes, list) or not isinstance(files, dict):
        raise ReviewError("Snapshot manifest is missing scopes or files")

    active_rel = f"workspace/projects/{slug}"
    completed_rel = f"workspace/projects/completed/{slug}"
    allowed = {
        active_rel,
        completed_rel,
        "library/context/people",
        "library/context",
    }
    if len(scopes) != 2 or any(
        not isinstance(scope, str) or scope not in allowed for scope in scopes
    ):
        raise ReviewError(f"Snapshot contains an unapproved scope: {scopes!r}")
    if sum(scope in (active_rel, completed_rel) for scope in scopes) != 1:
        raise ReviewError("Snapshot must contain exactly one named project scope")
    if sum(scope in ("library/context/people", "library/context") for scope in scopes) != 1:
        raise ReviewError("Snapshot must contain exactly one global context scope")

    normalized_files: dict[str, dict[str, object]] = {}
    for raw_rel, meta in files.items():
        rel = _safe_manifest_rel(raw_rel)
        if not any(rel == scope or rel.startswith(scope + "/") for scope in scopes):
            raise ReviewError(f"Snapshot file is outside its declared scopes: {rel}")
        if (
            not isinstance(meta, dict)
            or not re.fullmatch(r"[0-9a-f]{64}", str(meta.get("sha256", "")))
            or not isinstance(meta.get("size"), int)
            or meta["size"] < 0
        ):
            raise ReviewError(f"Invalid hash metadata for {rel}")
        normalized_files[rel] = meta
    manifest["root"] = str(root)
    manifest["project"] = slug
    manifest["scopes"] = scopes
    manifest["files"] = normalized_files
    return manifest


def _snapshot_bytes(run_dir: Path, manifest: dict, rel: str) -> bytes:
    path = run_dir / "before" / Path(rel)
    try:
        data = path.read_bytes()
    except OSError as exc:
        raise ReviewError(f"Snapshot payload missing for {rel}: {exc}") from exc
    expected = manifest["files"][rel]
    if len(data) != expected["size"] or sha256_bytes(data) != expected["sha256"]:
        raise ReviewError(f"Snapshot payload hash mismatch for {rel}")
    return data


def current_state(run_dir: Path, manifest: dict) -> dict[str, bytes]:
    root = Path(manifest["root"])
    scopes = [root.joinpath(*Path(rel).parts) for rel in manifest["scopes"]]
    return collect_markdown(root, scopes)


def state_changes(
    run_dir: Path, manifest: dict | None = None
) -> tuple[list[str], list[str], list[str], dict[str, bytes]]:
    run_dir = run_dir.expanduser().resolve()
    manifest = manifest or load_snapshot(run_dir)
    current = current_state(run_dir, manifest)
    before_paths = set(manifest["files"])
    current_paths = set(current)
    added = sorted(current_paths - before_paths)
    deleted = sorted(before_paths - current_paths)
    changed = sorted(
        rel
        for rel in before_paths & current_paths
        if sha256_bytes(current[rel]) != manifest["files"][rel]["sha256"]
    )
    return added, changed, deleted, current


def _unified_diff(
    rel: str, before: bytes, after: bytes, *, added: bool = False, deleted: bool = False
) -> str:
    before_hash = sha256_bytes(before)
    after_hash = sha256_bytes(after)
    try:
        before_text = before.decode("utf-8")
        after_text = after.decode("utf-8")
    except UnicodeDecodeError:
        return (
            f"--- {'/dev/null' if added else f'a/{rel}'}\n"
            f"+++ {'/dev/null' if deleted else f'b/{rel}'}\n"
            f"Binary Markdown changed: {before_hash} -> {after_hash}\n"
        )
    fromfile = "/dev/null" if added else f"a/{rel}"
    tofile = "/dev/null" if deleted else f"b/{rel}"
    lines = difflib.unified_diff(
        before_text.splitlines(keepends=True),
        after_text.splitlines(keepends=True),
        fromfile=fromfile,
        tofile=tofile,
        lineterm="\n",
    )
    rendered = "".join(lines)
    if not rendered and (added or deleted):
        rendered = f"--- {fromfile}\n+++ {tofile}\n"
    if rendered and not rendered.endswith("\n"):
        rendered += "\n"
    return rendered


def _change_records(
    manifest: dict, current: dict[str, bytes]
) -> dict[str, dict[str, object]]:
    before_paths = set(manifest["files"])
    current_paths = set(current)
    records: dict[str, dict[str, object]] = {}
    for rel in sorted(before_paths | current_paths):
        if rel not in before_paths:
            kind = "added"
        elif rel not in current_paths:
            kind = "deleted"
        elif sha256_bytes(current[rel]) != manifest["files"][rel]["sha256"]:
            kind = "changed"
        else:
            continue
        exists = kind != "deleted"
        data = current.get(rel)
        records[rel] = {
            "change_kind": kind,
            "exists": exists,
            "sha256": sha256_bytes(data) if exists and data is not None else None,
            "size": len(data) if exists and data is not None else None,
        }
    return records


def _render_review_from_state(
    run_dir: Path,
    manifest: dict,
    records: dict[str, dict[str, object]],
    current: dict[str, bytes],
) -> str:
    """Render the exact in-memory post state represented by records."""
    counts = {
        kind: sum(meta["change_kind"] == kind for meta in records.values())
        for kind in ("added", "changed", "deleted")
    }
    sections = [
        f"Summary: {counts['added']} added, {counts['changed']} changed, "
        f"{counts['deleted']} deleted\n"
    ]
    for rel, meta in records.items():
        kind = meta["change_kind"]
        before = b"" if kind == "added" else _snapshot_bytes(run_dir, manifest, rel)
        after = b"" if kind == "deleted" else current[rel]
        sections.append(
            _unified_diff(
                rel,
                before,
                after,
                added=kind == "added",
                deleted=kind == "deleted",
            )
        )
    if not records:
        sections.append("No Markdown changes in the snapshotted scopes.\n")
    return "\n".join(section.rstrip("\n") for section in sections) + "\n"


def render_review(run_dir: Path) -> str:
    """Return a stable, human-readable diff covering ignored files."""
    run_dir = run_dir.expanduser().resolve()
    manifest = load_snapshot(run_dir)
    for rel in manifest["files"]:
        _snapshot_bytes(run_dir, manifest, rel)
    current = current_state(run_dir, manifest)
    records = _change_records(manifest, current)
    return _render_review_from_state(run_dir, manifest, records, current)


def _snapshot_manifest_hash(run_dir: Path) -> str:
    path = run_dir / SNAPSHOT_MANIFEST
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as exc:
        raise ReviewError(f"Cannot hash snapshot manifest {path}: {exc}") from exc


def _review_path(raw: str, manifest: dict) -> str:
    rel = _safe_manifest_rel(raw)
    if not any(
        rel == scope or rel.startswith(scope + "/") for scope in manifest["scopes"]
    ):
        raise ReviewError(f"Review path is outside the snapshot scopes: {rel}")
    return rel


def _review_state_digest(snapshot_hash: str, records: dict) -> str:
    payload = {
        "snapshot_manifest_sha256": snapshot_hash,
        "scope_changes": records,
    }
    frozen = json.dumps(
        payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")
    return sha256_bytes(frozen)


def seal_review(run_dir: Path, paths: list[str]) -> str:
    """Render and seal one exact full-scope review state plus owned paths."""
    run_dir = run_dir.expanduser().resolve()
    manifest = load_snapshot(run_dir)
    if not paths:
        raise ReviewError("Seal at least one exact changed Markdown path")
    normalized = [_review_path(raw, manifest) for raw in paths]
    if len(set(normalized)) != len(normalized):
        raise ReviewError("Review paths must not be repeated")
    for rel in manifest["files"]:
        _snapshot_bytes(run_dir, manifest, rel)

    current = current_state(run_dir, manifest)
    scope_changes = _change_records(manifest, current)
    sealed_paths: dict[str, dict[str, object]] = {}
    for rel in normalized:
        record = scope_changes.get(rel)
        if record is None:
            raise ReviewError(
                f"Review path is not currently added, changed, or deleted: {rel}"
            )
        if record["change_kind"] != "added":
            _snapshot_bytes(run_dir, manifest, rel)
        sealed_paths[rel] = record

    snapshot_hash = _snapshot_manifest_hash(run_dir)
    review_digest = _review_state_digest(snapshot_hash, scope_changes)
    seal = {
        "schema_version": REVIEW_SCHEMA,
        "sealed_at": _utc_now(),
        "snapshot_manifest_sha256": snapshot_hash,
        "review_state_sha256": review_digest,
        "scope_changes": scope_changes,
        "paths": {rel: sealed_paths[rel] for rel in sorted(sealed_paths)},
    }
    _preflight_sealed_paths(manifest, seal, current)
    rendered_review = _render_review_from_state(
        run_dir, manifest, scope_changes, current
    )
    _write_json(run_dir / REVIEW_MANIFEST, seal)

    unsealed = sorted(set(scope_changes) - set(sealed_paths))
    sections = [
        f"Review state SHA-256: {review_digest}\n",
        rendered_review,
    ]
    lines = [f"Sealed review set: {len(sealed_paths)} path(s)"]
    lines.extend(
        f"SEALED {sealed_paths[rel]['change_kind']} {rel}"
        for rel in sorted(sealed_paths)
    )
    lines.append(f"Unsealed scope changes: {len(unsealed)}")
    lines.extend(
        f"UNSEALED {scope_changes[rel]['change_kind']} {rel}" for rel in unsealed
    )
    sections.append("\n".join(lines) + "\n")
    return "\n".join(section.rstrip("\n") for section in sections) + "\n"


def _validated_review_records(
    records: object, manifest: dict, *, label: str
) -> dict[str, dict[str, object]]:
    if not isinstance(records, dict) or not records:
        raise ReviewError(f"{label} contains no paths")
    normalized: dict[str, dict[str, object]] = {}
    for raw_rel, meta in records.items():
        rel = _review_path(raw_rel, manifest)
        if not isinstance(meta, dict) or set(meta) != {
            "change_kind",
            "exists",
            "sha256",
            "size",
        }:
            raise ReviewError(f"Invalid {label} metadata for {rel}")
        kind = meta.get("change_kind")
        exists = meta.get("exists")
        if kind not in ("added", "changed", "deleted") or not isinstance(exists, bool):
            raise ReviewError(f"Invalid {label} change kind or existence for {rel}")
        was_present = rel in manifest["files"]
        if (kind == "added" and was_present) or (
            kind in ("changed", "deleted") and not was_present
        ):
            raise ReviewError(f"{label} change kind contradicts the snapshot for {rel}")
        if kind == "deleted":
            if exists or meta.get("sha256") is not None or meta.get("size") is not None:
                raise ReviewError(f"Deleted {label} path has invalid post state: {rel}")
        elif (
            not exists
            or not re.fullmatch(r"[0-9a-f]{64}", str(meta.get("sha256", "")))
            or not isinstance(meta.get("size"), int)
            or isinstance(meta.get("size"), bool)
            or meta["size"] < 0
        ):
            raise ReviewError(f"{label} path has invalid post state: {rel}")
        if (
            kind == "changed"
            and meta["sha256"] == manifest["files"][rel]["sha256"]
            and meta["size"] == manifest["files"][rel]["size"]
        ):
            raise ReviewError(f"{label} marks an unchanged path as changed: {rel}")
        normalized[rel] = meta
    return normalized


def load_review_seal(run_dir: Path, manifest: dict | None = None) -> dict:
    run_dir = run_dir.expanduser().resolve()
    manifest = manifest or load_snapshot(run_dir)
    path = run_dir / REVIEW_MANIFEST
    try:
        seal = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReviewError(
            f"Review set is not sealed or cannot be read at {path}: {exc}"
        ) from exc
    if seal.get("schema_version") != REVIEW_SCHEMA:
        raise ReviewError("Unsupported review manifest schema")
    snapshot_hash = _snapshot_manifest_hash(run_dir)
    if seal.get("snapshot_manifest_sha256") != snapshot_hash:
        raise ReviewError("Snapshot manifest changed after the review set was sealed")

    scope_changes = _validated_review_records(
        seal.get("scope_changes"), manifest, label="Full review state"
    )
    expected_digest = _review_state_digest(snapshot_hash, scope_changes)
    if seal.get("review_state_sha256") != expected_digest:
        raise ReviewError("Stored full review-state digest is invalid")
    sealed_paths = _validated_review_records(
        seal.get("paths"), manifest, label="Sealed review set"
    )
    for rel, meta in sealed_paths.items():
        if scope_changes.get(rel) != meta:
            raise ReviewError(
                f"Sealed path does not match the full review state: {rel}"
            )

    seal["scope_changes"] = scope_changes
    seal["paths"] = sealed_paths
    return seal


def _preflight_sealed_paths(
    manifest: dict, seal: dict, current: dict[str, bytes]
) -> None:
    root = Path(manifest["root"])
    for rel, meta in seal["paths"].items():
        path = root.joinpath(*Path(rel).parts)
        _assert_no_symlink_parts(root, path)
        if path.exists() and path.is_dir():
            raise ReviewError(f"Cannot operate on Markdown over a directory: {path}")
        if meta["exists"]:
            if not path.is_file() or _is_linklike(path):
                raise ReviewError(f"Sealed path no longer exists as a safe file: {rel}")
            data = current.get(rel)
            if (
                data is None
                or len(data) != meta["size"]
                or sha256_bytes(data) != meta["sha256"]
            ):
                raise ReviewError(
                    f"Sealed path changed after review; resolve manually: {rel}"
                )
        elif path.exists():
            raise ReviewError(
                f"Sealed deleted path reappeared after review; resolve manually: {rel}"
            )


def _verify_review_state(
    run_dir: Path, manifest: dict | None = None, seal: dict | None = None
) -> tuple[dict, dict, dict[str, bytes]]:
    """Recompute and verify the entire full-scope state bound by the seal."""
    run_dir = run_dir.expanduser().resolve()
    manifest = manifest or load_snapshot(run_dir)
    seal = seal or load_review_seal(run_dir, manifest)
    if _snapshot_manifest_hash(run_dir) != seal["snapshot_manifest_sha256"]:
        raise ReviewError("Snapshot manifest changed after the review set was sealed")
    current = current_state(run_dir, manifest)
    current_records = _change_records(manifest, current)
    current_digest = _review_state_digest(
        seal["snapshot_manifest_sha256"], current_records
    )
    if (
        current_digest != seal["review_state_sha256"]
        or current_records != seal["scope_changes"]
    ):
        paths = sorted(set(current_records) | set(seal["scope_changes"]))
        drifted = [
            rel
            for rel in paths
            if current_records.get(rel) != seal["scope_changes"].get(rel)
        ]
        preview = ", ".join(drifted[:5])
        if len(drifted) > 5:
            preview += f", +{len(drifted) - 5} more"
        raise ReviewError(
            "Full review state changed after sealing "
            f"(expected {seal['review_state_sha256']}, got {current_digest}; "
            f"drift: {preview or 'unknown'}). Rerender and reseal."
        )
    _preflight_sealed_paths(manifest, seal, current)
    return manifest, seal, current


def verify_review(run_dir: Path) -> str:
    """Verify the exact full diff state and sealed subset without mutating."""
    _, seal, _ = _verify_review_state(run_dir)
    unsealed = sorted(set(seal["scope_changes"]) - set(seal["paths"]))
    lines = [f"Review state verified: {seal['review_state_sha256']}"]
    lines.append(f"Sealed paths: {len(seal['paths'])}")
    lines.extend(
        f"SEALED {meta['change_kind']} {rel}"
        for rel, meta in seal["paths"].items()
    )
    lines.append(f"Unsealed scope changes: {len(unsealed)}")
    lines.extend(
        f"UNSEALED {seal['scope_changes'][rel]['change_kind']} {rel}"
        for rel in unsealed
    )
    return "\n".join(lines) + "\n"


def _atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".consolidation-restore-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(data)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def restore_snapshot(run_dir: Path) -> tuple[int, int]:
    """Restore sealed paths only after full review-state verification."""
    run_dir = run_dir.expanduser().resolve()
    manifest = load_snapshot(run_dir)
    seal = load_review_seal(run_dir, manifest)
    root = Path(manifest["root"])
    originals = {
        rel: _snapshot_bytes(run_dir, manifest, rel)
        for rel, meta in seal["paths"].items()
        if meta["change_kind"] != "added"
    }
    _verify_review_state(run_dir, manifest, seal)

    removed = 0
    for rel, meta in seal["paths"].items():
        if meta["change_kind"] != "added":
            continue
        root.joinpath(*Path(rel).parts).unlink()
        removed += 1
    for rel, data in originals.items():
        _atomic_write(root.joinpath(*Path(rel).parts), data)

    for rel, meta in seal["paths"].items():
        path = root.joinpath(*Path(rel).parts)
        if meta["change_kind"] == "added":
            if path.exists():
                raise ReviewError(f"Rollback verification failed for added path: {rel}")
            continue
        data = path.read_bytes()
        before = manifest["files"][rel]
        if len(data) != before["size"] or sha256_bytes(data) != before["sha256"]:
            raise ReviewError(f"Rollback verification failed for restored path: {rel}")
    return len(originals), removed


def discover_project_people(root: Path) -> list[dict[str, str]]:
    """Discover active and completed project overlays through explicit paths."""
    root = _validated_root(root)
    projects_root = root / "workspace" / "projects"
    records: list[dict[str, str]] = []
    locations = [
        ("active", projects_root),
        ("completed", projects_root / "completed"),
    ]
    for state, base in locations:
        if not base.is_dir():
            continue
        for project in sorted(base.iterdir(), key=lambda path: path.name):
            if not project.is_dir() or (state == "active" and project.name == "completed"):
                continue
            if not (project / "project.md").is_file():
                continue
            people = project / "knowledge" / "people"
            if not people.is_dir():
                continue
            for overlay in sorted(people.glob("*.md"), key=lambda path: path.name):
                _assert_no_symlink_parts(root, overlay)
                records.append(
                    {
                        "state": state,
                        "project": project.name,
                        "person": overlay.stem,
                        "path": overlay.relative_to(root).as_posix(),
                    }
                )
    return records


def render_people_inventory(root: Path, project_slug: str) -> str:
    root = _validated_root(root)
    project = resolve_project_scope(root, project_slug)
    all_records = discover_project_people(root)
    target_prefix = project.relative_to(root).as_posix() + "/knowledge/people/"
    target_people = {
        record["person"]
        for record in all_records
        if record["path"].startswith(target_prefix)
    }
    lines = ["state\tproject\tperson\tpath"]
    for record in all_records:
        if record["person"] in target_people:
            lines.append(
                "\t".join(
                    record[key] for key in ("state", "project", "person", "path")
                )
            )
    global_people = root / "library" / "context" / "people"
    for profile in sorted(global_people.glob("*/profile.md")):
        if profile.parent.name in target_people:
            lines.append(
                "global\t-\t"
                + profile.parent.name
                + "\t"
                + profile.relative_to(root).as_posix()
            )
    return "\n".join(lines) + "\n"


def _candidate_files(run_dir: Path, root: Path) -> dict[str, bytes]:
    candidate_root = run_dir / "promotion-candidates"
    if not candidate_root.is_dir():
        raise ReviewError(f"Promotion candidate directory does not exist: {candidate_root}")
    if _is_linklike(candidate_root):
        raise ReviewError("Promotion candidate directory cannot be a symlink or junction")
    found: dict[str, bytes] = {}
    for dirpath, dirnames, filenames in os.walk(candidate_root, followlinks=False):
        base = Path(dirpath)
        if any(_is_linklike(base / dirname) for dirname in dirnames):
            raise ReviewError("Symlinked promotion candidate directories are not allowed")
        for filename in filenames:
            path = base / filename
            if _is_linklike(path):
                raise ReviewError(f"Symlinked promotion candidate is not allowed: {path}")
            rel = path.relative_to(candidate_root).as_posix()
            parts = Path(rel).parts
            valid = (
                len(parts) == 5
                and parts[:3] == ("library", "context", "people")
                and SLUG_RE.fullmatch(parts[3])
                and parts[4] == "profile.md"
            )
            if not valid:
                raise ReviewError(
                    "Promotion candidates must use "
                    "library/context/people/{slug}/profile.md"
                )
            target = root.joinpath(*parts)
            _assert_no_symlink_parts(root, target)
            if target.exists():
                raise ReviewError(
                    f"Promotion target already exists; update it live instead: {rel}"
                )
            found[rel] = path.read_bytes()
    if not found:
        raise ReviewError("No promotion candidates were staged")
    return found


def _promotion_batch_hash(files: dict) -> str:
    frozen = json.dumps(
        files, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")
    return sha256_bytes(frozen)


def stage_promotions(run_dir: Path) -> str:
    """Freeze candidate hashes and render the exact batch awaiting approval."""
    run_dir = run_dir.expanduser().resolve()
    snapshot = load_snapshot(run_dir)
    root = Path(snapshot["root"])
    candidates = _candidate_files(run_dir, root)
    files = {
        rel: {"sha256": sha256_bytes(data), "size": len(data)}
        for rel, data in sorted(candidates.items())
    }
    manifest = {
        "schema_version": 1,
        "staged_at": _utc_now(),
        "batch_sha256": _promotion_batch_hash(files),
        "files": files,
    }
    _write_json(run_dir / PROMOTION_MANIFEST, manifest)
    sections = [
        f"Promotion batch: {len(candidates)} candidate(s)\n"
        f"Frozen batch SHA-256: {manifest['batch_sha256']}\n"
    ]
    for rel, data in sorted(candidates.items()):
        sections.append(_unified_diff(rel, b"", data, added=True))
    return "\n".join(section.rstrip("\n") for section in sections) + "\n"


def apply_promotions(run_dir: Path, approval_receipt: str) -> int:
    """Bind an approval excerpt to, then apply, an unchanged frozen batch."""
    if not approval_receipt or not approval_receipt.strip():
        raise ReviewError(
            "A nonempty exact operator approval excerpt is required as the receipt"
        )
    run_dir = run_dir.expanduser().resolve()
    snapshot = load_snapshot(run_dir)
    root = Path(snapshot["root"])
    path = run_dir / PROMOTION_MANIFEST
    try:
        staged = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReviewError(f"Cannot read promotion manifest {path}: {exc}") from exc
    if staged.get("schema_version") != 1 or not isinstance(staged.get("files"), dict):
        raise ReviewError("Invalid promotion manifest")
    batch_sha256 = _promotion_batch_hash(staged["files"])
    if staged.get("batch_sha256") != batch_sha256:
        raise ReviewError("Frozen promotion batch manifest changed after staging")

    candidates = _candidate_files(run_dir, root)
    if set(candidates) != set(staged["files"]):
        raise ReviewError("Promotion candidate set changed after review; stage it again")
    for rel, data in candidates.items():
        meta = staged["files"][rel]
        if (
            not isinstance(meta, dict)
            or not isinstance(meta.get("size"), int)
            or len(data) != meta.get("size")
            or sha256_bytes(data) != meta.get("sha256")
        ):
            raise ReviewError(
                f"Promotion candidate changed after review: {rel}; stage it again"
            )

    for rel, data in sorted(candidates.items()):
        target = root.joinpath(*Path(rel).parts)
        if target.exists():
            raise ReviewError(f"Promotion target appeared after review: {rel}")
        _atomic_write(target, data)
    staged["applied_at"] = _utc_now()
    staged["approval"] = {
        "receipt": approval_receipt,
        "batch_sha256": batch_sha256,
        "recorded_at": _utc_now(),
        "authentication": "recorded but not authenticated by this helper",
    }
    _write_json(path, staged)
    return len(candidates)


def read_approval_receipt(path: Path, run_dir: Path) -> str:
    """Read a receipt as data so operator text is never interpolated into a shell."""
    resolved = path.expanduser().resolve()
    expected = run_dir.expanduser().resolve() / "operator-approval.txt"
    if resolved != expected:
        raise ReviewError(f"Approval receipt must be the run-local file {expected}")
    try:
        receipt = resolved.read_text(encoding="utf-8-sig")
    except OSError as exc:
        raise ReviewError(f"Cannot read approval receipt {resolved}: {exc}") from exc
    if not receipt.strip():
        raise ReviewError("Approval receipt file is empty")
    return receipt.strip()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Review and restore ignored Markdown touched by a dream pass"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    snapshot = sub.add_parser("snapshot", help="capture before bytes and hashes")
    snapshot.add_argument("--root", type=Path, default=REPO_ROOT)
    snapshot.add_argument("--project", required=True)
    snapshot.add_argument("--run-dir", type=Path)
    snapshot.add_argument("--include-operator-context", action="store_true")

    diff = sub.add_parser("diff", help="render an ignored-file-aware unified diff")
    diff.add_argument("--run-dir", type=Path, required=True)

    seal = sub.add_parser(
        "seal-review", help="seal exact pass-owned changes for selective rollback"
    )
    seal.add_argument("--run-dir", type=Path, required=True)
    seal.add_argument("--path", action="append", required=True)

    verify = sub.add_parser(
        "verify-review", help="verify the sealed full-scope review state"
    )
    verify.add_argument("--run-dir", type=Path, required=True)

    restore = sub.add_parser("restore", help="restore only the sealed review set")
    restore.add_argument("--run-dir", type=Path, required=True)

    people = sub.add_parser("people", help="inventory active and completed overlays")
    people.add_argument("--root", type=Path, default=REPO_ROOT)
    people.add_argument("--project", required=True)

    stage = sub.add_parser(
        "stage-promotions", help="hash and render new global-person candidates"
    )
    stage.add_argument("--run-dir", type=Path, required=True)

    apply = sub.add_parser(
        "apply-promotions", help="apply an unchanged, operator-approved batch"
    )
    apply.add_argument("--run-dir", type=Path, required=True)
    apply.add_argument(
        "--approval-receipt-file",
        type=Path,
        required=True,
        help="UTF-8 file containing the exact operator approval excerpt",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "snapshot":
            run_dir = snapshot_state(
                args.root,
                args.project,
                args.run_dir,
                args.include_operator_context,
            )
            print(run_dir)
            print(render_review(run_dir), end="")
        elif args.command == "diff":
            print(render_review(args.run_dir), end="")
        elif args.command == "seal-review":
            print(seal_review(args.run_dir, args.path), end="")
        elif args.command == "verify-review":
            print(verify_review(args.run_dir), end="")
        elif args.command == "restore":
            restored, removed = restore_snapshot(args.run_dir)
            print(
                f"Selective rollback complete: restored {restored}, "
                f"removed {removed} sealed addition(s)"
            )
        elif args.command == "people":
            print(render_people_inventory(args.root, args.project), end="")
        elif args.command == "stage-promotions":
            print(stage_promotions(args.run_dir), end="")
        elif args.command == "apply-promotions":
            count = apply_promotions(
                args.run_dir,
                read_approval_receipt(args.approval_receipt_file, args.run_dir),
            )
            print(
                f"Applied {count} approved promotion(s); the receipt is batch-bound "
                "but operator identity is not authenticated by this helper"
            )
        return 0
    except ReviewError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
