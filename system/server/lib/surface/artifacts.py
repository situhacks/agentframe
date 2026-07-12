"""Artifact index for the Preview rail.

Primary source: ``project.md`` deliverable rows merged with archived rows from
``knowledge/_archive/deliverables-archive.md``. Version discovery is
stem-scoped: only siblings sharing the current file's stem join a group, so a
multi-stem folder (e.g. a post folder holding ``post-FINAL.md`` plus ingredient
trails) groups cleanly with no special case. No legacy (``*-vF.md``) handling.
"""

from __future__ import annotations

import os
import re
from pathlib import Path

from . import state

VERSION_STEM_RE = re.compile(r"^(?P<base>.+)-v(?P<n>\d+)$")
TIMESTAMP_STEM_RE = re.compile(r"^(?P<base>.+)_(?P<ts>\d{8}_\d{6})$")

HIDDEN_DIRS = {
    "node_modules",
    "sources",
    "knowledge",
    "references",
    "archive",
    "backup",
    "history",
    "raw",
    "stills",
}

PREVIEWABLE_EXTS = {
    ".md",
    ".txt",
    ".html",
    ".png",
    ".jpg",
    ".jpeg",
    ".webp",
    ".gif",
    ".svg",
    ".pdf",
    ".mp4",
    ".mov",
    ".webm",
    ".pptx",
    ".docx",
}

TEXT_EXTS = {".md", ".txt"}

TYPE_BY_EXT = {
    ".md": "text",
    ".txt": "text",
    ".html": "html",
    ".png": "image",
    ".jpg": "image",
    ".jpeg": "image",
    ".webp": "image",
    ".gif": "image",
    ".svg": "image",
    ".pdf": "pdf",
    ".mp4": "video",
    ".mov": "video",
    ".webm": "video",
    ".pptx": "office",
    ".docx": "office",
}

TYPE_ORDER = ("text", "image", "pdf", "video", "html", "office")

ARCHIVE_REL_PATH = Path("knowledge") / "_archive" / "deliverables-archive.md"


def _rel_posix(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def load_archived_rows(project_dir: Path) -> dict:
    archive = Path(project_dir) / ARCHIVE_REL_PATH
    if not archive.is_file():
        return {}
    try:
        fm = state.parse_frontmatter(archive.read_text(encoding="utf-8"))
    except OSError:
        return {}
    rows = fm.get("deliverables")
    return rows if isinstance(rows, dict) else {}


DESIGN_LANGUAGE_RE = re.compile(r"^design-language-v(\d+)\.md$", re.IGNORECASE)


def _find_design_language(project_dir: Path) -> Path | None:
    """Locate the highest-versioned design-language-v{N}.md under any phase folder."""
    root = Path(project_dir)
    best: tuple[int, Path] | None = None
    for md in root.glob("phase-*/**/design-language/design-language-v*.md"):
        m = DESIGN_LANGUAGE_RE.match(md.name)
        if not m:
            continue
        n = int(m.group(1))
        if best is None or n > best[0]:
            best = (n, md)
    return best[1] if best else None


def design_group(project_dir: Path) -> dict | None:
    """A pinned 'Design' group for the project's current design language.

    Present only when a design-language-v{N}.md exists. ``current`` points at
    the storybook HTML when the frontmatter names one and the file exists;
    otherwise it falls back to the design-language markdown so the Design pin
    still opens something. ``folder`` carries the design-language directory so
    the rail can list the whole folder under the ``all`` filter.
    """
    root = Path(project_dir)
    md = _find_design_language(root)
    if md is None:
        return None
    try:
        fm = state.parse_frontmatter(md.read_text(encoding="utf-8"))
    except OSError:
        fm = {}
    folder = md.parent
    storybook_rel = fm.get("storybook")
    current = None
    if storybook_rel:
        candidate = folder / storybook_rel
        if candidate.is_file():
            current = _rel_posix(candidate, root)
    if current is None:
        current = _rel_posix(md, root)
    return {
        "kind": "design",
        "slug": "design-language",
        "label": "Design",
        "status": fm.get("status"),
        "current": current,
        "last_updated": state._iso(fm.get("last_updated")),
        "version_count": 1,
        "has_exports": False,
        "types": _type_tags(root, current),
        "archived": False,
        "folder": _rel_posix(folder, root),
        "pinned": True,
    }


def _sibling_versions(project_dir: Path, current_rel: str) -> list[str]:
    """Stem-scoped version chain for the current file, ascending; [] if missing."""
    root = Path(project_dir)
    current = root / current_rel
    if not current.is_file():
        return []
    folder, stem, ext = current.parent, current.stem, current.suffix

    m = VERSION_STEM_RE.match(stem)
    if m:
        base = m.group("base")
        chain = []
        for f in folder.iterdir():
            if not f.is_file() or f.suffix != ext:
                continue
            fm = VERSION_STEM_RE.match(f.stem)
            if fm and fm.group("base") == base:
                chain.append((int(fm.group("n")), f))
        chain.sort(key=lambda pair: pair[0])
        return [_rel_posix(f, root) for _, f in chain]

    m = TIMESTAMP_STEM_RE.match(stem)
    if m:
        base = m.group("base")
        chain = []
        for f in folder.iterdir():
            if not f.is_file() or f.suffix != ext:
                continue
            fm = TIMESTAMP_STEM_RE.match(f.stem)
            if fm and fm.group("base") == base:
                chain.append((fm.group("ts"), f))
        chain.sort(key=lambda pair: pair[0])
        return [_rel_posix(f, root) for _, f in chain]

    return [current_rel]


def _exports(project_dir: Path, current_rel: str) -> list[str]:
    root = Path(project_dir)
    current = root / current_rel
    exports_dir = current.parent / "exports"
    if not exports_dir.is_dir():
        return []
    return sorted(_rel_posix(f, root) for f in exports_dir.iterdir() if f.is_file())


def _sequence(value) -> list[str]:
    if isinstance(value, list):
        return [str(v) for v in value if str(v).strip()]
    if isinstance(value, str) and value.strip():
        return [value.strip()]
    return []


def _safe_project_rel(root: Path, path: Path) -> str | None:
    root = root.resolve()
    try:
        resolved = path.resolve()
    except (OSError, ValueError):
        return None
    if resolved == root or root not in resolved.parents:
        return None
    return resolved.relative_to(root).as_posix()


def _resolve_manifest_rel(project_dir: Path, owner_rel: str, value: str) -> str | None:
    """Resolve a manifest path inside the project.

    Project-root paths are canonical. Owner-folder relative paths are accepted
    so existing post-local `media/foo.png` entries still surface.
    """
    root = Path(project_dir)
    raw = str(value).strip().strip('"').strip("'")
    if not raw or raw.startswith(("http://", "https://")):
        return None
    raw_path = Path(raw)
    candidates = [raw_path] if raw_path.is_absolute() else [root / raw_path, (root / owner_rel).parent / raw_path]
    for candidate in candidates:
        rel = _safe_project_rel(root, candidate)
        if rel and (root / rel).is_file():
            return rel
    return None


def _manifest_media(project_dir: Path, current_rel: str) -> list[str]:
    root = Path(project_dir)
    current = root / current_rel
    if not current.is_file():
        return []
    try:
        fm = state.parse_frontmatter(current.read_text(encoding="utf-8"))
    except OSError:
        return []
    out: list[str] = []
    seen: set[str] = set()
    for key in ("shipped_media", "exports"):
        for value in _sequence(fm.get(key)):
            rel = _resolve_manifest_rel(root, current_rel, value)
            if rel and rel not in seen and Path(rel).suffix.lower() in PREVIEWABLE_EXTS:
                seen.add(rel)
                out.append(rel)
    return out


def _walk_previewable(folder: Path, root: Path) -> list[str]:
    if not folder.is_dir():
        return []
    out: list[str] = []
    for dirpath, dirnames, filenames in os.walk(folder):
        dirnames[:] = [
            d
            for d in dirnames
            if d not in HIDDEN_DIRS
            and not d.startswith(".")
            and not (Path(dirpath) / d / ".preview-hide").exists()
        ]
        for fname in filenames:
            if fname.startswith("."):
                continue
            f = Path(dirpath) / fname
            if f.suffix.lower() in PREVIEWABLE_EXTS:
                out.append(_rel_posix(f, root))
    return sorted(out)


def _folder_media(project_dir: Path, current_rel: str, claimed: set[str] | None = None) -> list[str]:
    root = Path(project_dir)
    current = root / current_rel
    claimed = claimed or set()
    out: list[str] = []
    seen: set[str] = set(claimed)
    for dirname in ("media", "visuals"):
        for rel in _walk_previewable(current.parent / dirname, root):
            if rel not in seen:
                seen.add(rel)
                out.append(rel)
    return out


def _type_for(rel: str) -> str:
    return TYPE_BY_EXT.get(Path(rel).suffix.lower(), "unsupported")


def _type_tags(project_dir: Path, current_rel: str | None) -> list[str]:
    if not current_rel:
        return []
    paths = [current_rel]
    paths += _exports(project_dir, current_rel)
    paths += _manifest_media(project_dir, current_rel)
    paths += _folder_media(project_dir, current_rel, claimed=set(paths))
    exts: list[str] = []
    for rel in paths:
        ext = Path(rel).suffix.lower().lstrip(".")
        if ext and ext not in exts:
            exts.append(ext)
    return exts


def _label(slug: str) -> str:
    return slug.replace("-", " ").replace("_", " ").title()


def _row_group(project_dir: Path, slug: str, row: dict, index: int, archived: bool) -> dict:
    file_rel = row.get("file")
    current = str(file_rel) if file_rel else None
    if current and not (Path(project_dir) / current).is_file():
        versions: list[str] = []
    else:
        versions = _sibling_versions(project_dir, current) if current else []
    return {
        "kind": "deliverable",
        "slug": slug,
        "label": _label(slug),
        "status": row.get("status"),
        "current": current,
        "last_updated": state._iso(row.get("last_updated")),
        "version_count": len(versions),
        "has_exports": bool(current and _exports(project_dir, current)),
        "types": _type_tags(project_dir, current),
        "archived": archived,
        "_row_index": index,
    }


def artifact_groups(project_dir: Path, rows: dict) -> list[dict]:
    """Group summaries from tracker rows + archived rows, newest first.

    Sorted by ``last_updated`` descending; date-only ties break by tracker row
    order with the later row winning. Version lists stay lazy — summaries carry
    counts only (see ``group_detail``).
    """
    project_dir = Path(project_dir)
    groups = []
    index = 0
    for slug, row in (rows or {}).items():
        if isinstance(row, dict):
            groups.append(_row_group(project_dir, slug, row, index, archived=False))
            index += 1
    for slug, row in load_archived_rows(project_dir).items():
        if isinstance(row, dict) and slug not in (rows or {}):
            groups.append(_row_group(project_dir, slug, row, index, archived=True))
            index += 1
    groups.sort(key=lambda g: (g["last_updated"] or "", g["_row_index"]), reverse=True)
    for g in groups:
        del g["_row_index"]
    dg = design_group(project_dir)
    if dg is not None:
        groups.insert(0, dg)
    return groups


def design_detail(project_dir: Path, folder_rel: str) -> dict:
    """Whole design-language folder listing for the pinned Design group.

    Under the ``all`` filter the rail expands the Design group to every
    previewable file in the folder (md, tokens.yaml/.css, storybook, preview
    subfiles). ``versions`` carries the flat file list; media buckets stay
    empty since this group is not tracker-backed.
    """
    root = Path(project_dir)
    folder = root / folder_rel
    files: list[str] = []
    if folder.is_dir():
        for f in sorted(folder.rglob("*")):
            if f.is_file() and f.suffix.lower() in PREVIEWABLE_EXTS:
                files.append(_rel_posix(f, root))
    # No ``current`` key here: the design group summary already carries the
    # storybook (or md fallback) as ``current``; a None here would clobber it
    # when merged as ``{**group, **detail}``.
    return {
        "versions": files,
        "manifest_media": [],
        "exports": [],
        "folder_media": [],
    }


def group_detail(project_dir: Path, current_rel: str) -> dict:
    """Versions (ascending) and exports for one group; loaded on expand only."""
    exports = _exports(project_dir, current_rel)
    manifest_media = _manifest_media(project_dir, current_rel)
    return {
        "current": current_rel,
        "versions": _sibling_versions(project_dir, current_rel),
        "manifest_media": manifest_media,
        "exports": [rel for rel in exports if rel not in manifest_media],
        "folder_media": _folder_media(project_dir, current_rel, claimed=set(manifest_media + exports)),
    }


def file_record(project_dir: Path, rel: str, group: str) -> dict | None:
    root = Path(project_dir)
    path = root / rel
    if not path.is_file():
        return None
    st = path.stat()
    return {
        "path": rel,
        "group": group,
        "mtime": int(st.st_mtime),
        "size": st.st_size,
        "type": _type_for(rel),
    }


def _include_file(rel: str, file_class: str, narrow_type: str | None = None) -> bool:
    ext = Path(rel).suffix.lower()
    if file_class == "text":
        if ext not in TEXT_EXTS:
            return False
    elif file_class == "media":
        if ext in TEXT_EXTS:
            return False
        if narrow_type and _type_for(rel) != narrow_type:
            return False
    else:
        return False
    return True


def project_files(project_dir: Path, rows: dict, file_class: str, narrow_type: str | None = None) -> list[dict]:
    """Flat text/media list from grouped versions, exports, manifest media, and untracked files."""
    root = Path(project_dir)
    records: dict[str, dict] = {}
    groups = artifact_groups(root, rows)
    claimed: set[str] = set()
    for g in groups:
        if not g["current"]:
            continue
        detail = group_detail(root, g["current"])
        paths = [g["current"]] + detail["versions"] + detail["manifest_media"] + detail["exports"] + detail["folder_media"]
        for rel in paths:
            claimed.add(rel)
            if rel not in records and _include_file(rel, file_class, narrow_type):
                rec = file_record(root, rel, g["slug"])
                if rec:
                    records[rel] = rec
    for rel in untracked_files(root, claimed):
        if rel not in records and _include_file(rel, file_class, narrow_type):
            rec = file_record(root, rel, "untracked")
            if rec:
                records[rel] = rec
    return sorted(records.values(), key=lambda rec: (rec["mtime"], rec["path"]), reverse=True)


def untracked_files(project_dir: Path, claimed: set[str]) -> list[str]:
    """Previewable files not claimed by any group; hidden paths pruned."""
    root = Path(project_dir)
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            d
            for d in dirnames
            if d not in HIDDEN_DIRS
            and not d.startswith(".")
            and not (Path(dirpath) / d / ".preview-hide").exists()
        ]
        for fname in filenames:
            if fname.startswith("."):
                continue
            f = Path(dirpath) / fname
            if f.suffix.lower() not in PREVIEWABLE_EXTS:
                continue
            rel = _rel_posix(f, root)
            if rel not in claimed and rel != "project.md" and rel != "activity.md":
                found.append(rel)
    return sorted(found)
