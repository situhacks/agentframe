"""Deterministic project-state readers for the Workspace Dashboard.

Scanner contract (see the V2 plan): active projects live at root-level
``workspace/projects/*`` and historical projects live one level below
``workspace/projects/completed/*``. Callers choose whether to include history.
Folders without ``project.md`` are skipped silently and missing fields are
tolerated as ``None``. Frontmatter is parsed with PyYAML so inline comments
parse cleanly; a project whose frontmatter fails to parse is skipped, never
fatal.
"""

from __future__ import annotations

import datetime
import re
from pathlib import Path

import yaml

FRONTMATTER_RE = re.compile(r"\A---\s*\n(.*?)\n---", re.DOTALL)
ATTENTION_BULLET_RE = re.compile(r"^-\s*\[([ xX])\]\s*(.*)$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
TIMESTAMPED_LINE_RE = re.compile(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2})\s*[—–-]*\s*(.*)$")
EVENT_PREFIX_RE = re.compile(r"^([a-z][a-z0-9_]*):(?!//)\s*(.*)$")
MD_LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
PATH_TOKEN_RE = re.compile(r"[\w.\-]+(?:/[\w.\-]+)+\.\w{2,5}")

PROJECT_FIELDS = (
    "name",
    "status",
    "domain",
    "flow",
    "current_phase",
    "created_at",
    "last_activity",
    "shipped_at",
    "completed_at",
    "cancelled_at",
)


def _iso(value):
    """Normalize yaml date/datetime objects to ISO strings; pass through the rest."""
    if isinstance(value, (datetime.datetime, datetime.date)):
        return value.isoformat()
    return value


def parse_frontmatter(text: str) -> dict:
    """Parse the leading ``---`` frontmatter block; {} when absent or invalid."""
    m = FRONTMATTER_RE.match(text)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1))
    except yaml.YAMLError:
        return {}
    return data if isinstance(data, dict) else {}


def project_directories(root: Path, *, include_completed: bool = False) -> list[Path]:
    """Project directories in deterministic order, optionally including history."""
    projects_dir = Path(root) / "workspace" / "projects"
    if not projects_dir.is_dir():
        return []
    folders = [
        folder
        for folder in sorted(projects_dir.iterdir())
        if folder.is_dir() and folder.name != "completed"
    ]
    completed_dir = projects_dir / "completed"
    if include_completed and completed_dir.is_dir():
        folders.extend(folder for folder in sorted(completed_dir.iterdir()) if folder.is_dir())
    return folders


def scan_projects(root: Path, *, include_completed: bool = False) -> list[dict]:
    """Scan active projects, or active + completed/cancelled project history."""
    out = []
    for folder in project_directories(root, include_completed=include_completed):
        project_md = folder / "project.md"
        if not project_md.is_file():
            continue
        try:
            fm = parse_frontmatter(project_md.read_text(encoding="utf-8"))
        except OSError:
            continue
        if not fm or (not include_completed and fm.get("status") != "active"):
            continue
        if include_completed and fm.get("status") not in {"active", "complete", "cancelled"}:
            continue
        project = {field: _iso(fm.get(field)) for field in PROJECT_FIELDS}
        project["slug"] = fm.get("slug") or folder.name
        deliverables = fm.get("deliverables")
        project["deliverables"] = deliverables if isinstance(deliverables, dict) else {}
        automations = fm.get("automations")
        project["automations"] = automations if isinstance(automations, dict) else {}
        project["dir"] = str(folder)
        out.append(project)
    return out


def _attention_span(lines: list[str]) -> tuple[int, int]:
    """(start, end) line indexes of the ``## Attention`` block, or (-1, -1).

    The block runs from its heading until the next heading or the first line
    that is neither blank nor a checkbox bullet.
    """
    start = -1
    for i, line in enumerate(lines):
        if line.strip() == "## Attention":
            start = i
            break
    if start == -1:
        return (-1, -1)
    end = len(lines)
    for j in range(start + 1, len(lines)):
        stripped = lines[j].strip()
        if not stripped:
            continue
        if stripped.startswith("#") or not ATTENTION_BULLET_RE.match(stripped):
            end = j
            break
    return (start, end)


def parse_attention(text: str) -> list[dict]:
    """Parse ``- [ ] date | kind | text`` bullets from the ``## Attention`` block."""
    lines = text.splitlines()
    start, end = _attention_span(lines)
    if start == -1:
        return []
    items = []
    for line in lines[start + 1 : end]:
        m = ATTENTION_BULLET_RE.match(line.strip())
        if not m:
            continue
        checked = m.group(1).lower() == "x"
        rest = m.group(2).strip()
        parts = [p.strip() for p in rest.split("|")]
        if len(parts) >= 3 and DATE_RE.match(parts[0]):
            date, kind, body = parts[0], parts[1], " | ".join(parts[2:])
        else:
            date, kind, body = None, None, rest
        link = MD_LINK_RE.search(body)
        items.append(
            {
                "date": date,
                "kind": kind,
                "text": body,
                "checked": checked,
                "file": link.group(1) if link else None,
                "raw": line.strip(),
            }
        )
    return items


def parse_activity(text: str) -> list[dict]:
    """Parse activity lines (outside the Attention block), newest first.

    Timestamped entries sort by timestamp descending (later-in-file first on
    ties); untimestamped raw lines follow in file order.
    """
    lines = text.splitlines()
    att_start, att_end = _attention_span(lines)
    timestamped: list[dict] = []
    raw_lines: list[dict] = []
    for i, line in enumerate(lines):
        if att_start != -1 and att_start <= i < att_end:
            continue
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        m = TIMESTAMPED_LINE_RE.match(stripped)
        if m:
            rest = m.group(2)
            ev = EVENT_PREFIX_RE.match(rest)
            timestamped.append(
                {
                    "timestamp": m.group(1),
                    "event": ev.group(1) if ev else None,
                    "text": ev.group(2) if ev else rest,
                    "raw": stripped,
                }
            )
        else:
            raw_lines.append({"timestamp": None, "event": None, "text": stripped, "raw": stripped})
    timestamped.reverse()
    timestamped.sort(key=lambda e: e["timestamp"], reverse=True)
    return timestamped + raw_lines


def detect_file_ref(line: str, project_root: Path) -> str | None:
    """First path-like token in the line that resolves inside the project."""
    candidates = MD_LINK_RE.findall(line) + PATH_TOKEN_RE.findall(line)
    for cand in candidates:
        rel = cand.strip()
        while rel.startswith("./"):
            rel = rel[2:]
        if not rel or rel.startswith(("http:", "https:", "..", "/")):
            continue
        try:
            if (Path(project_root) / rel).is_file():
                return rel
        except OSError:
            continue
    return None
