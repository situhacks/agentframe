"""Preview hub: server-side workspace scan + HTML shell rendering.

The hub is the single page served at `/`. It enumerates every preview
artifact across the workspace (active projects, completed, demo) and
renders a self-contained dashboard with a persistent sidebar, a panzoom
canvas, and a right-rail copy pane.

Four artifact kinds are surfaced to the sidebar:
- `single`: one HTML file (slide, design board, comparison page, etc.)
- `image`:  PNG / JPG / WebP / GIF / SVG file. Rendered in an <img>.
- `pdf`:    PDF file. Rendered via PDF.js as a side-by-side page strip.
- `video`:  MP4 / MOV / WebM file. Rendered in a native <video controls>.
- `group`:  a `post-N/` directory containing `visuals/carousel-slide-*.html`
            (and an optional sibling `copy.md`). Renders as N slides
            side-by-side in the canvas, with copy.md text in the right rail.

Pure functions only — no Tornado coupling, so this can be unit-tested by
passing in a temp directory as `project_root`.
"""

from __future__ import annotations

import html as _html
import json
from fnmatch import fnmatchcase
from pathlib import Path, PurePosixPath
from typing import Any

# CDN-pinned panzoom (timmywil) — actively maintained, ~3.7 KB gzipped.
PANZOOM_CDN = "https://unpkg.com/@panzoom/panzoom@4.6.2/dist/panzoom.min.js"

# PDF.js v3.11.x is the last UMD release (exposes global `pdfjsLib`); v4+ is
# ESM-only and would require <script type="module"> plumbing. Loaded lazily
# on first PDF view to keep HTML-only sessions weightless.
PDFJS_CDN = "https://cdn.jsdelivr.net/npm/pdfjs-dist@3.11.174/legacy/build/pdf.min.js"
PDFJS_WORKER_CDN = "https://cdn.jsdelivr.net/npm/pdfjs-dist@3.11.174/legacy/build/pdf.worker.min.js"

# Map of file extension (lowercase, with dot) -> sidebar entry kind.
# .html -> single (renders in iframe)
# image formats -> image (renders in <img>)
# .pdf -> pdf (renders via PDF.js as canvas strip)
# video formats -> video (renders in <video controls>)
_KIND_BY_EXT: dict[str, str] = {
    ".html": "single",
    ".png":  "image",
    ".jpg":  "image",
    ".jpeg": "image",
    ".webp": "image",
    ".gif":  "image",
    ".svg":  "image",
    ".pdf":  "pdf",
    ".mp4":  "video",
    ".mov":  "video",
    ".webm": "video",
}
_SCANNED_EXTS = tuple(_KIND_BY_EXT.keys())


class _ScanFilters:
    """Config-driven exclusions + folder marker filtering for workspace scans."""

    def __init__(
        self,
        project_root: Path,
        *,
        exclude_globs: list[str] | None = None,
        include_intermediates: bool = False,
    ) -> None:
        self.project_root = project_root.resolve()
        self.exclude_globs = [g for g in (exclude_globs or []) if g]
        self.include_intermediates = include_intermediates
        self.filtered_count = 0
        self._hidden_dir_cache: dict[Path, bool] = {}

    def should_skip(self, path: Path) -> bool:
        if self.include_intermediates:
            return False
        rel = self._to_relative_posix(path)
        if rel is None:
            return False
        if any(self._matches_glob(rel, glob) for glob in self.exclude_globs):
            self.filtered_count += 1
            return True
        if self._ancestor_has_hide_marker(path.parent):
            self.filtered_count += 1
            return True
        return False

    def _to_relative_posix(self, path: Path) -> str | None:
        try:
            return path.resolve().relative_to(self.project_root).as_posix()
        except ValueError:
            return None

    @staticmethod
    def _matches_glob(rel_path: str, glob: str) -> bool:
        pure = PurePosixPath(rel_path)
        return pure.match(glob) or fnmatchcase(rel_path, glob)

    def _ancestor_has_hide_marker(self, directory: Path) -> bool:
        directory = directory.resolve()
        if directory in self._hidden_dir_cache:
            return self._hidden_dir_cache[directory]

        if directory == self.project_root:
            hidden = (directory / ".preview-hide").exists()
            self._hidden_dir_cache[directory] = hidden
            return hidden

        if self.project_root not in directory.parents:
            self._hidden_dir_cache[directory] = False
            return False

        hidden = (directory / ".preview-hide").exists()
        if not hidden:
            hidden = self._ancestor_has_hide_marker(directory.parent)
        self._hidden_dir_cache[directory] = hidden
        return hidden


def _file_to_url(project_root: Path, file_path: Path) -> str:
    """Convert an absolute path under project_root to a server URL."""
    rel = file_path.resolve().relative_to(project_root.resolve())
    return "/" + rel.as_posix()


def _campaign_label(file_path: Path, campaign_root: Path) -> str:
    """Build a sidebar label from the path inside a project.

    `<project>/phase-3-planning/design-language/preview/direction-compare.html`
        -> `phase-3 / design-language / direction-compare`

    HTML files have their .html extension stripped (it's noise — every
    HTML entry would have it). Image and PDF entries keep their extension
    so the user can tell at a glance what they're clicking.
    """
    rel = file_path.resolve().relative_to(campaign_root.resolve())
    parts = list(rel.parts)
    if parts and parts[-1].lower().endswith(".html"):
        parts[-1] = parts[-1][: -len(".html")]
    cleaned: list[str] = []
    for part in parts:
        if part == "preview":
            continue
        if part.startswith("phase-"):
            cleaned.append(part.split("-planning")[0].split("-production")[0]
                           .split("-research")[0].split("-strategy")[0]
                           .split("-launch")[0])
            continue
        if part == "posts":
            continue
        cleaned.append(part)
    return " / ".join(cleaned)


def _detect_post_groups(
    scan_root: Path,
    project_root: Path,
    *,
    filters: _ScanFilters | None = None,
) -> list[dict[str, Any]]:
    """Find post-N directories with visuals/carousel-slide-*.html.

    Convention (uniform across demo and projects):
      <scan_root>/.../post-{n}/visuals/carousel-slide-*.html
      <scan_root>/.../post-{n}/copy.md            (optional)

    Returns one entry per post directory, kind="group". Slides are listed
    in lexicographic order by filename (carousel-slide-1, -2, ...).
    """
    if not scan_root.exists():
        return []
    groups: list[dict[str, Any]] = []
    for post_dir in sorted(scan_root.rglob("post-*")):
        if not post_dir.is_dir():
            continue
        if filters is not None and filters.should_skip(post_dir):
            continue
        visuals = post_dir / "visuals"
        if not visuals.is_dir():
            continue
        slides = sorted(visuals.glob("carousel-slide-*.html"))
        if filters is not None:
            slides = [s for s in slides if not filters.should_skip(s)]
        if not slides:
            continue
        rel_post_dir = post_dir.relative_to(scan_root)
        parent_parts = list(rel_post_dir.parent.parts)
        slide_rel_paths = [
            slide_path.relative_to(scan_root).as_posix()
            for slide_path in slides
        ]
        slide_urls = [_file_to_url(project_root, s) for s in slides]
        copy_path = post_dir / "copy.md"
        copy_text = ""
        if copy_path.exists():
            try:
                copy_text = copy_path.read_text(encoding="utf-8")
            except OSError:
                copy_text = ""
        try:
            mtime = max(s.stat().st_mtime for s in slides)
        except OSError:
            mtime = 0.0
        dir_url = _file_to_url(project_root, post_dir)
        groups.append({
            "kind": "group",
            "id": "group:" + dir_url,
            "label": post_dir.name,
            "slide_urls": slide_urls,
            "copy_text": copy_text,
            "mtime": mtime,
            "_parent_parts": parent_parts,
            "_slide_rel_paths": slide_rel_paths,
        })
    groups.sort(key=lambda g: g["label"])
    return groups


def _iter_supported_files(scan_root: Path, *, filters: _ScanFilters | None = None):
    """Yield (path, kind) for every file under scan_root with a known extension.

    Case-insensitive on extension (so .PNG works as well as .png).
    """
    for path in scan_root.rglob("*"):
        if not path.is_file():
            continue
        if filters is not None and filters.should_skip(path):
            continue
        kind = _KIND_BY_EXT.get(path.suffix.lower())
        if kind is None:
            continue
        yield path, kind


def _entry_label(file_path: Path) -> str:
    """Leaf label for sidebar entries within a folder tree."""
    if file_path.suffix.lower() == ".html":
        return file_path.stem
    return file_path.name


def _new_tree_node(name: str, path: str) -> dict[str, Any]:
    return {
        "name": name,
        "path": path,
        "dirs": [],
        "entries": [],
    }


def _insert_entry_in_tree(
    tree: dict[str, Any],
    *,
    parent_parts: list[str],
    entry: dict[str, Any],
) -> None:
    node = tree
    walked: list[str] = []
    for part in parent_parts:
        walked.append(part)
        found = next((child for child in node["dirs"] if child["name"] == part), None)
        if found is None:
            found = _new_tree_node(part, "/".join(walked))
            node["dirs"].append(found)
        node = found
    node["entries"].append(entry)


def _sort_tree(node: dict[str, Any]) -> None:
    node["dirs"].sort(key=lambda d: d["name"].lower())
    node["entries"].sort(key=lambda e: e["label"].lower())
    for child in node["dirs"]:
        _sort_tree(child)


def _collect_entries_from_tree(node: dict[str, Any]) -> list[dict[str, Any]]:
    entries = list(node["entries"])
    for child in node["dirs"]:
        entries.extend(_collect_entries_from_tree(child))
    return entries


def _scan_campaign(
    campaign_dir: Path,
    project_root: Path,
    *,
    filters: _ScanFilters | None = None,
) -> dict[str, Any]:
    """Build a nested folder tree for one project under phase-* dirs.

    "Single" here is a misnomer for back-compat: the scanner emits entries of
    kind `single` (HTML), `image`, or `pdf` — all rendered as one artifact in
    the canvas. Only `group` is multi-artifact.
    """
    tree = _new_tree_node(campaign_dir.name, "")
    groups = _detect_post_groups(campaign_dir, project_root, filters=filters)
    grouped_slide_paths: set[str] = set()
    for group in groups:
        grouped_slide_paths.update(group.get("_slide_rel_paths", []))
        parent_parts = list(group.get("_parent_parts", []))
        _insert_entry_in_tree(tree, parent_parts=parent_parts, entry=group)

    for file_path, kind in _iter_supported_files(campaign_dir, filters=filters):
        rel = file_path.relative_to(campaign_dir)
        if not any(p.startswith("phase-") for p in rel.parts):
            continue
        if rel.as_posix() in grouped_slide_paths:
            continue
        try:
            mtime = file_path.stat().st_mtime
        except OSError:
            continue
        url = _file_to_url(project_root, file_path)
        _insert_entry_in_tree(tree, parent_parts=list(rel.parts[:-1]), entry={
            "kind": kind,
            "id": url,
            "label": _entry_label(file_path),
            "url": url,
            "mtime": mtime,
        })
    _sort_tree(tree)
    return tree


def _scan_campaign_group(
    group_dir: Path,
    project_root: Path,
    *,
    filters: _ScanFilters | None = None,
) -> list[dict[str, Any]]:
    """Scan a directory of projects. Returns one entry per project folder."""
    if not group_dir.exists():
        return []
    projects: list[dict[str, Any]] = []
    for child in sorted(group_dir.iterdir()):
        if not child.is_dir():
            continue
        if child.name == "completed":
            continue
        tree = _scan_campaign(child, project_root, filters=filters)
        projects.append({
            "slug": child.name,
            "tree": tree,
        })
    return projects


def _scan_demo(
    demo_dir: Path,
    project_root: Path,
    *,
    filters: _ScanFilters | None = None,
) -> dict[str, Any]:
    """Build a nested folder tree for demo artifacts."""
    tree = _new_tree_node("demo", "")
    if not demo_dir.exists():
        return tree
    groups = _detect_post_groups(demo_dir, project_root, filters=filters)
    grouped_slide_paths: set[str] = set()
    for group in groups:
        grouped_slide_paths.update(group.get("_slide_rel_paths", []))
        parent_parts = list(group.get("_parent_parts", []))
        _insert_entry_in_tree(tree, parent_parts=parent_parts, entry=group)

    for file_path, kind in _iter_supported_files(demo_dir, filters=filters):
        rel = file_path.relative_to(demo_dir)
        if rel.as_posix() in grouped_slide_paths:
            continue
        try:
            mtime = file_path.stat().st_mtime
        except OSError:
            continue
        url = _file_to_url(project_root, file_path)
        _insert_entry_in_tree(tree, parent_parts=list(rel.parts[:-1]), entry={
            "kind": kind,
            "id": url,
            "label": _entry_label(file_path),
            "url": url,
            "mtime": mtime,
        })
    _sort_tree(tree)
    return tree


def scan_workspace(
    project_root: str | Path,
    *,
    exclude_globs: list[str] | None = None,
    include_intermediates: bool = False,
) -> dict[str, Any]:
    """Discover every preview HTML and group it for the sidebar.

    Returns a dict with `active`, `completed`, `demo`, and `default_id`.
    `default_id` is the most-recently-modified entry across all groups
    so the hub auto-loads something useful on first paint.
    """
    root = Path(project_root)
    filters = _ScanFilters(
        root,
        exclude_globs=exclude_globs,
        include_intermediates=include_intermediates,
    )
    active = _scan_campaign_group(
        root / "workspace" / "projects",
        root,
        filters=filters,
    )
    completed = _scan_campaign_group(
        root / "workspace" / "projects" / "completed",
        root,
        filters=filters,
    )
    demo = _scan_demo(
        root / "system" / "server" / "static" / "demo",
        root,
        filters=filters,
    )

    pool: list[dict[str, Any]] = []
    for camp in active + completed:
        pool.extend(_collect_entries_from_tree(camp["tree"]))
    pool.extend(_collect_entries_from_tree(demo))
    default_id = ""
    if pool:
        default_id = max(pool, key=lambda e: e["mtime"])["id"]

    return {
        "active": active,
        "completed": completed,
        "demo": demo,
        "default_id": default_id,
        "hidden_count": filters.filtered_count if not include_intermediates else 0,
        "include_intermediates": include_intermediates,
    }


def _entry_payload(entry: dict[str, Any]) -> dict[str, Any]:
    """Strip mtime; serialize only what the client needs to render."""
    payload: dict[str, Any] = {
        "kind": entry["kind"],
        "id": entry["id"],
        "label": entry["label"],
    }
    if entry["kind"] in ("single", "image", "pdf", "video"):
        payload["url"] = entry["url"]
    elif entry["kind"] == "group":
        payload["slide_urls"] = entry["slide_urls"]
        payload["copy_text"] = entry["copy_text"]
    return payload


# Sidebar prefix per kind. HTML singles have no prefix (the default case);
# group uses a filled square; image/pdf/video use bracketed tags so they
# stand out in a long sidebar list of HTML labels.
_SIDEBAR_PREFIX: dict[str, str] = {
    "group": "\u25a3 ",
    "image": "[img] ",
    "pdf":   "[pdf] ",
    "video": "[video] ",
}


def _render_entry_li(entry: dict[str, Any]) -> str:
    """One sidebar list item. Embeds the artifact payload as data-artifact JSON."""
    payload_json = json.dumps(_entry_payload(entry))
    label = _html.escape(entry["label"])
    prefix = _SIDEBAR_PREFIX.get(entry["kind"], "")
    if prefix:
        label = f"{prefix}{label}"
    return (
        f'<li><a class="entry kind-{entry["kind"]}" '
        f'href="#{_html.escape(entry["id"])}" '
        f'data-id="{_html.escape(entry["id"])}" '
        f'data-artifact="{_html.escape(payload_json, quote=True)}">'
        f'{label}</a></li>'
    )


def _render_tree(node: dict[str, Any]) -> str:
    blocks: list[str] = []
    for child in node["dirs"]:
        child_html = _render_tree(child)
        if not child_html:
            continue
        blocks.append(
            f'<details class="folder"><summary>{_html.escape(child["name"])}</summary>'
            f"{child_html}</details>"
        )
    if node["entries"]:
        items = "\n".join(_render_entry_li(e) for e in node["entries"])
        blocks.append(f'<ul class="entries">{items}</ul>')
    return "".join(blocks)


def _render_campaign_block(camp: dict[str, Any]) -> str:
    slug = _html.escape(camp["slug"])
    tree_html = _render_tree(camp["tree"])
    if not tree_html:
        return (
            f'<details class="camp empty"><summary>{slug}</summary>'
            f'<div class="empty-note">no preview HTML yet</div></details>'
        )
    return (
        f'<details class="camp" open><summary>{slug}</summary>'
        f"{tree_html}</details>"
    )


def _render_demo_block(tree: dict[str, Any]) -> str:
    tree_html = _render_tree(tree)
    if not tree_html:
        return '<div class="empty-note">no demo files</div>'
    return tree_html


def render_hub_html(model: dict[str, Any]) -> str:
    """Render the full hub HTML shell (head + sidebar + canvas + rail + JS)."""
    active_html = "\n".join(_render_campaign_block(c) for c in model["active"]) \
        or '<div class="empty-note">no active projects</div>'
    completed_html = "\n".join(_render_campaign_block(c) for c in model["completed"]) \
        or '<div class="empty-note">none yet</div>'
    demo_html = _render_demo_block(model["demo"])
    default_id_json = json.dumps(model.get("default_id") or "")
    hidden_count = int(model.get("hidden_count") or 0)
    include_intermediates = bool(model.get("include_intermediates"))
    intermediates_link_html = ""
    if hidden_count > 0 and not include_intermediates:
        intermediates_link_html = (
            f'<a class="show-intermediates" href="/?intermediates=1">'
            f"Show intermediates ({hidden_count} hidden)</a>"
        )
    elif include_intermediates:
        intermediates_link_html = '<a class="show-intermediates" href="/">Hide intermediates</a>'

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<title>AgentFrame Preview Hub</title>
<style>
  * {{ box-sizing: border-box; }}
  html, body {{ margin: 0; padding: 0; height: 100%; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; color: #e6e6e6; background: #1a1a1a; }}
  body {{ display: grid; grid-template-columns: 280px 1fr 0; height: 100vh; overflow: hidden; transition: grid-template-columns 0.15s ease; }}
  body.has-copy {{ grid-template-columns: 280px 1fr 320px; }}
  aside.sidebar {{ background: #111; border-right: 1px solid #2a2a2a; overflow-y: auto; padding: 16px 12px; }}
  aside.sidebar h1 {{ font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: #888; margin: 0 0 4px 8px; }}
  aside.sidebar h1.brand {{ font-size: 14px; color: #f0f0f0; margin-bottom: 16px; letter-spacing: 0.04em; }}
  aside.sidebar .group {{ margin-bottom: 18px; }}
  aside.sidebar .group > details {{ margin-bottom: 4px; }}
  aside.sidebar summary {{ cursor: pointer; padding: 6px 8px; font-size: 13px; color: #ccc; border-radius: 4px; user-select: none; }}
  aside.sidebar summary:hover {{ background: #1f1f1f; color: #fff; }}
  aside.sidebar details > summary::-webkit-details-marker {{ display: none; }}
  aside.sidebar details > summary::before {{ content: "\u25b8 "; color: #555; font-size: 10px; }}
  aside.sidebar details[open] > summary::before {{ content: "\u25be "; color: #888; }}
  aside.sidebar .camp summary {{ font-weight: 500; color: #ddd; }}
  aside.sidebar .folder {{ margin-left: 10px; }}
  aside.sidebar .folder > summary {{ font-size: 12px; color: #bbb; }}
  aside.sidebar ul.entries {{ list-style: none; padding: 2px 0 6px 20px; margin: 0; }}
  aside.sidebar ul.entries li {{ margin: 0; }}
  aside.sidebar a.entry {{ display: block; padding: 4px 8px; font-size: 12px; color: #aaa; text-decoration: none; border-radius: 3px; line-height: 1.3; }}
  aside.sidebar a.entry:hover {{ background: #1f1f1f; color: #fff; }}
  aside.sidebar a.entry.active {{ background: #2a3a4a; color: #fff; }}
  aside.sidebar a.entry.kind-group {{ color: #c9d6e3; font-weight: 500; }}
  aside.sidebar .empty-note {{ font-size: 11px; color: #666; padding: 4px 12px 4px 28px; font-style: italic; }}
  main {{ display: grid; grid-template-rows: auto 1fr; min-width: 0; min-height: 0; }}
  .toolbar {{ display: flex; align-items: center; gap: 12px; padding: 8px 14px; background: #161616; border-bottom: 1px solid #2a2a2a; font-size: 12px; }}
  .toolbar .breadcrumb {{ color: #888; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  .toolbar .zoom-group {{ display: flex; gap: 4px; }}
  .toolbar button {{ background: #222; color: #ccc; border: 1px solid #333; padding: 4px 10px; font-size: 12px; border-radius: 3px; cursor: pointer; font-family: inherit; }}
  .toolbar button:hover {{ background: #2a2a2a; color: #fff; }}
  .toolbar button.open-tab {{ margin-left: 6px; }}
  .toolbar a.show-intermediates {{ color: #8fb8ff; text-decoration: none; white-space: nowrap; }}
  .toolbar a.show-intermediates:hover {{ color: #b8d2ff; text-decoration: underline; }}
  .toolbar .hint {{ color: #555; font-size: 11px; }}
  .canvas {{ position: relative; overflow: hidden; background: #0d0d0d; background-image: linear-gradient(45deg, #151515 25%, transparent 25%), linear-gradient(-45deg, #151515 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #151515 75%), linear-gradient(-45deg, transparent 75%, #151515 75%); background-size: 20px 20px; background-position: 0 0, 0 10px, 10px -10px, -10px 0px; cursor: grab; }}
  .canvas:active {{ cursor: grabbing; }}
  #canvas-content {{ position: absolute; top: 0; left: 0; pointer-events: none; display: inline-block; }}
  /* Generalized: every direct child of #canvas-content is non-interactive (panzoom owns gestures). */
  #canvas-content > *,
  #canvas-content iframe,
  #canvas-content img,
  #canvas-content canvas {{ pointer-events: none; }}
  #canvas-content video {{ pointer-events: auto; }}
  #canvas-content iframe,
  #canvas-content img,
  #canvas-content canvas,
  #canvas-content video {{ display: block; border: 1px solid #2a2a2a; box-shadow: 0 8px 32px rgba(0,0,0,0.5); background: #000; }}
  #canvas-content .strip {{ display: flex; flex-direction: row; gap: 24px; align-items: flex-start; }}
  aside.rail {{ background: #111; border-left: 1px solid #2a2a2a; overflow-y: auto; overflow-x: hidden; }}
  aside.rail .rail-header {{ padding: 10px 14px; border-bottom: 1px solid #2a2a2a; font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: #888; }}
  aside.rail pre {{ margin: 0; padding: 16px; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 12px; line-height: 1.55; color: #d8d8d8; white-space: pre-wrap; word-wrap: break-word; }}
  .empty-state {{ display: grid; place-items: center; height: 100%; color: #555; font-size: 14px; padding: 40px; text-align: center; }}
</style>
</head>
<body>
<aside class="sidebar">
  <h1 class="brand">AgentFrame Preview</h1>

  <div class="group">
    <h1>Active</h1>
    {active_html}
  </div>

  <div class="group">
    <h1>Completed</h1>
    <details><summary>archived</summary>
      <div style="padding: 4px 8px;">{completed_html}</div>
    </details>
  </div>

  <div class="group">
    <h1>Demo</h1>
    <details open><summary>system/server/static/demo</summary>
      <div style="padding: 4px 8px;">{demo_html}</div>
    </details>
  </div>
</aside>

<main>
  <div class="toolbar">
    <div class="breadcrumb" id="breadcrumb">\u2014</div>
    <div class="zoom-group">
      <button data-zoom="0.25">25%</button>
      <button data-zoom="0.5">50%</button>
      <button data-zoom="1">100%</button>
      <button data-zoom="fit">Fit</button>
    </div>
    <button class="open-tab" id="open-tab">Open in new tab</button>
    {intermediates_link_html}
    <span class="hint">drag to pan \u00b7 ctrl+wheel to zoom \u00b7 \u2190/\u2192 walk \u00b7 F fit \u00b7 1 100% \u00b7 +/- step</span>
  </div>

  <div class="canvas" id="canvas">
    <div id="canvas-content"></div>
  </div>
</main>

<aside class="rail" id="copy-rail">
  <div class="rail-header">copy.md</div>
  <pre id="copy-text"></pre>
</aside>

<script src="{PANZOOM_CDN}"></script>
<script>
(function() {{
  const DEFAULT_ID = {default_id_json};
  const PDFJS_CDN = "{PDFJS_CDN}";
  const PDFJS_WORKER_CDN = "{PDFJS_WORKER_CDN}";
  const canvas = document.getElementById('canvas');
  const canvasContent = document.getElementById('canvas-content');
  const breadcrumb = document.getElementById('breadcrumb');
  const openTabBtn = document.getElementById('open-tab');
  const copyText = document.getElementById('copy-text');
  const entries = Array.from(document.querySelectorAll('a.entry'));

  let panzoom = null;
  let currentArtifact = null;
  let pdfjsLoadPromise = null;  // memoized lazy-load (one CDN fetch per session).

  // Ctrl+wheel = zoom; plain wheel falls through to native scroll.
  canvas.addEventListener('wheel', function(e) {{
    if (e.ctrlKey && panzoom) {{
      e.preventDefault();
      panzoom.zoomWithWheel(e);
    }}
  }}, {{ passive: false }});

  // Figma-style keybindings: walk sidebar with arrows, fit/zoom with F/0/1/+/-,
  // hold Space to enter pan mode (cursor changes to grab/grabbing).
  // Stay no-op when typing into form fields so we don't fight text inputs.
  function isTypingTarget(el) {{
    if (!el) return false;
    const tag = (el.tagName || '').toLowerCase();
    return tag === 'input' || tag === 'textarea' || tag === 'select' || el.isContentEditable;
  }}

  function zoomStep(direction) {{
    if (!panzoom) return;
    // Zoom about the canvas's visual center to keep behavior predictable.
    const scale = panzoom.getScale();
    const next  = direction > 0 ? scale * 1.2 : scale / 1.2;
    const cw = canvas.clientWidth, ch = canvas.clientHeight;
    panzoom.zoomToPoint(next, {{ clientX: cw / 2, clientY: ch / 2 }}, {{ animate: false }});
  }}

  function walkArtifact(direction) {{
    if (!entries.length) return;
    const idx = entries.findIndex(function(a) {{
      return currentArtifact && a.dataset.id === currentArtifact.id;
    }});
    const nextIdx = idx < 0
      ? 0
      : (idx + direction + entries.length) % entries.length;
    const next = artifactFromEntry(entries[nextIdx]);
    if (next) loadArtifact(next);
  }}

  let spaceHeld = false;
  document.addEventListener('keydown', function(e) {{
    if (isTypingTarget(e.target)) return;
    if (e.metaKey || e.altKey) return;  // leave shortcuts alone

    // Space: enter pan mode (grab cursor). Repeats are noisy — only act once.
    if (e.code === 'Space' && !e.repeat) {{
      spaceHeld = true;
      canvas.style.cursor = 'grab';
      e.preventDefault();
      return;
    }}

    switch (e.key) {{
      case 'ArrowRight': walkArtifact(+1); e.preventDefault(); break;
      case 'ArrowLeft':  walkArtifact(-1); e.preventDefault(); break;
      case 'f': case 'F': case '0':
        applyZoom('fit'); e.preventDefault(); break;
      case '1':
        if (panzoom) {{ panzoom.zoom(1, {{ animate: false }}); }}
        e.preventDefault(); break;
      case '+': case '=':
        zoomStep(+1); e.preventDefault(); break;
      case '-': case '_':
        zoomStep(-1); e.preventDefault(); break;
    }}
  }});

  document.addEventListener('keyup', function(e) {{
    if (e.code === 'Space') {{
      spaceHeld = false;
      canvas.style.cursor = '';
    }}
  }});

  // While space is held, force grabbing cursor on mousedown for the figma feel.
  canvas.addEventListener('mousedown', function() {{
    if (spaceHeld) canvas.style.cursor = 'grabbing';
  }});
  canvas.addEventListener('mouseup', function() {{
    if (spaceHeld) canvas.style.cursor = 'grab';
  }});

  function applyZoom(value) {{
    // Centering math assumes Panzoom's default transform-origin (50% 50%).
    // CSS transform pipeline: `translate(x, y) scale(s)` with origin 50/50 means
    // translation is applied AFTER scale, in canvas-pixel space. So to land the
    // element's center at the canvas's center, pan by (cw/2 - natW/2, ch/2 - natH/2).
    if (!panzoom) return;
    const cw   = canvas.clientWidth;
    const ch   = canvas.clientHeight;
    const natW = canvasContent.offsetWidth  || 1080;
    const natH = canvasContent.offsetHeight || 1350;
    let scale;
    if (value === 'fit') {{
      const FIT_MARGIN = 0.95;  // 5% breathing room on every side
      scale = Math.min(cw / natW, ch / natH) * FIT_MARGIN;
    }} else {{
      scale = parseFloat(value);
    }}
    panzoom.zoom(scale, {{ animate: false }});
    panzoom.pan((cw - natW) / 2, (ch - natH) / 2, {{ animate: false, force: true }});
  }}

  document.querySelectorAll('.zoom-group button').forEach(function(btn) {{
    btn.addEventListener('click', function() {{
      applyZoom(btn.dataset.zoom);
    }});
  }});

  function setActive(id) {{
    entries.forEach(function(a) {{
      a.classList.toggle('active', a.dataset.id === id);
    }});
    const found = entries.find(function(a) {{ return a.dataset.id === id; }});
    breadcrumb.textContent = found ? found.textContent + '  \u2014  ' + id : id;
    openTabBtn.disabled = false;
  }}

  function makeIframe(url) {{
    // Architectural rule: every hub-mounted iframe gets scrolling="no".
    // Inner scrollbars cannot appear regardless of slide author behavior.
    const ifr = document.createElement('iframe');
    ifr.setAttribute('scrolling', 'no');
    ifr.src = url;
    ifr.style.width  = '1080px';
    ifr.style.height = '1350px';
    return ifr;
  }}

  function autoSizeIframe(ifr) {{
    // After load, resize the iframe to its content's intrinsic dimensions
    // so canvas-content's bounding box reflects the true artwork size.
    try {{
      const doc = ifr.contentDocument;
      if (doc && doc.documentElement) {{
        const bw = doc.body ? doc.body.scrollWidth  : 0;
        const bh = doc.body ? doc.body.scrollHeight : 0;
        const w  = Math.max(doc.documentElement.scrollWidth,  bw, 1080);
        const h  = Math.max(doc.documentElement.scrollHeight, bh, 600);
        ifr.style.width  = w + 'px';
        ifr.style.height = h + 'px';
      }}
    }} catch (e) {{ /* cross-origin not expected on localhost */ }}
  }}

  function mountSingle(artifact) {{
    const ifr = makeIframe(artifact.url);
    canvasContent.appendChild(ifr);
    ifr.addEventListener('load', function onload() {{
      ifr.removeEventListener('load', onload);
      autoSizeIframe(ifr);
      requestAnimationFrame(function() {{ applyZoom('fit'); }});
    }});
  }}

  function mountGroup(artifact) {{
    const strip = document.createElement('div');
    strip.className = 'strip';
    const iframes = artifact.slide_urls.map(function(url) {{
      const ifr = makeIframe(url);
      strip.appendChild(ifr);
      return ifr;
    }});
    canvasContent.appendChild(strip);

    // Apply Fit only after every slide has loaded so the bounding box is stable.
    let pending = iframes.length;
    iframes.forEach(function(ifr) {{
      ifr.addEventListener('load', function onload() {{
        ifr.removeEventListener('load', onload);
        autoSizeIframe(ifr);
        pending -= 1;
        if (pending === 0) {{
          requestAnimationFrame(function() {{ applyZoom('fit'); }});
        }}
      }});
    }});
  }}

  function mountImage(artifact) {{
    // Native <img> renders at full intrinsic size; panzoom handles framing.
    const img = document.createElement('img');
    img.src = artifact.url;
    img.alt = artifact.label || '';
    canvasContent.appendChild(img);
    img.addEventListener('load', function onload() {{
      img.removeEventListener('load', onload);
      // Pin width/height so canvasContent's bounding box reflects intrinsic size
      // even when the image hasn't been laid out by the browser yet.
      img.style.width  = img.naturalWidth  + 'px';
      img.style.height = img.naturalHeight + 'px';
      requestAnimationFrame(function() {{ applyZoom('fit'); }});
    }});
    img.addEventListener('error', function() {{
      breadcrumb.textContent = 'failed to load image: ' + artifact.url;
    }});
  }}

  function mountVideo(artifact) {{
    // Native video playback for rendered MP4/WebM/MOV artifacts. Unlike static
    // previews, controls need pointer events so the operator can scrub/play.
    const video = document.createElement('video');
    video.src = artifact.url;
    video.controls = true;
    video.preload = 'metadata';
    video.style.maxWidth = '1280px';
    video.style.maxHeight = '720px';
    canvasContent.appendChild(video);
    video.addEventListener('loadedmetadata', function onload() {{
      video.removeEventListener('loadedmetadata', onload);
      const w = video.videoWidth || 1280;
      const h = video.videoHeight || 720;
      video.style.width = w + 'px';
      video.style.height = h + 'px';
      requestAnimationFrame(function() {{ applyZoom('fit'); }});
    }});
    video.addEventListener('error', function() {{
      breadcrumb.textContent = 'failed to load video: ' + artifact.url;
    }});
  }}

  function loadPdfJs() {{
    // Inject PDF.js v3 (UMD) on first use; memoize the promise so repeat calls
    // resolve immediately. Worker URL must be set before any getDocument() call.
    if (pdfjsLoadPromise) return pdfjsLoadPromise;
    pdfjsLoadPromise = new Promise(function(resolve, reject) {{
      if (window.pdfjsLib) {{
        window.pdfjsLib.GlobalWorkerOptions.workerSrc = PDFJS_WORKER_CDN;
        resolve(window.pdfjsLib);
        return;
      }}
      const s = document.createElement('script');
      s.src = PDFJS_CDN;
      s.onload = function() {{
        if (!window.pdfjsLib) {{
          reject(new Error('pdfjsLib missing after script load'));
          return;
        }}
        window.pdfjsLib.GlobalWorkerOptions.workerSrc = PDFJS_WORKER_CDN;
        resolve(window.pdfjsLib);
      }};
      s.onerror = function() {{ reject(new Error('PDF.js CDN load failed')); }};
      document.head.appendChild(s);
    }});
    return pdfjsLoadPromise;
  }}

  function mountPdf(artifact) {{
    // Render every page to its own <canvas> in a horizontal strip — same
    // mental model as the carousel post-group view. Iframing the PDF would
    // hand control to the browser's PDF viewer and break Fit + pan.
    const strip = document.createElement('div');
    strip.className = 'strip';
    canvasContent.appendChild(strip);
    breadcrumb.textContent = 'loading PDF\u2026';

    loadPdfJs().then(function(pdfjsLib) {{
      return pdfjsLib.getDocument(artifact.url).promise;
    }}).then(function(pdfDoc) {{
      const pageNumbers = [];
      for (let i = 1; i <= pdfDoc.numPages; i++) pageNumbers.push(i);
      // Render pages sequentially to keep memory predictable on long PDFs.
      // Standard high-DPI canvas pattern: bitmap at 2x for retina sharpness,
      // CSS size pinned to 1x logical PDF point dimensions so the strip's
      // bounding box matches HTML iframe equivalents. Without the CSS pin,
      // canvas defaults its display size to its bitmap size, doubling the
      // strip width and driving Fit to land at half the expected scale —
      // which then makes ctrl+wheel zoom steps feel disproportionate.
      const RENDER_SCALE = 2;
      return pageNumbers.reduce(function(chain, pageNum) {{
        return chain.then(function() {{
          return pdfDoc.getPage(pageNum).then(function(page) {{
            const renderViewport  = page.getViewport({{ scale: RENDER_SCALE }});
            const displayViewport = page.getViewport({{ scale: 1 }});
            const cv = document.createElement('canvas');
            cv.width  = renderViewport.width;
            cv.height = renderViewport.height;
            cv.style.width  = displayViewport.width  + 'px';
            cv.style.height = displayViewport.height + 'px';
            strip.appendChild(cv);
            return page.render({{
              canvasContext: cv.getContext('2d'),
              viewport: renderViewport,
            }}).promise;
          }});
        }});
      }}, Promise.resolve());
    }}).then(function() {{
      // Restore breadcrumb (loadArtifact's setActive already wrote one,
      // but we clobbered it with the 'loading' message above).
      if (currentArtifact) setActive(currentArtifact.id);
      requestAnimationFrame(function() {{ applyZoom('fit'); }});
    }}).catch(function(err) {{
      breadcrumb.textContent = 'PDF render failed: ' + (err && err.message || err);
    }});
  }}

  function loadArtifact(artifact, opts) {{
    if (!artifact) return;
    currentArtifact = artifact;

    // Tear down previous panzoom instance & content.
    if (panzoom) {{
      try {{ panzoom.destroy(); }} catch (e) {{}}
      panzoom = null;
    }}
    canvasContent.innerHTML = '';
    canvasContent.removeAttribute('style');
    canvasContent.style.position = 'absolute';
    canvasContent.style.top = '0';
    canvasContent.style.left = '0';
    canvasContent.style.pointerEvents = 'none';

    // Mount new content based on artifact kind.
    if (artifact.kind === 'group') {{
      mountGroup(artifact);
    }} else if (artifact.kind === 'image') {{
      mountImage(artifact);
    }} else if (artifact.kind === 'pdf') {{
      mountPdf(artifact);
    }} else if (artifact.kind === 'video') {{
      mountVideo(artifact);
    }} else {{
      mountSingle(artifact);
    }}

    // Init panzoom on the container. Works for iframe / image / canvas / strip.
    // We use Panzoom's default transform-origin (50% 50%) — it's required for
    // ctrl+wheel zoom-at-cursor to work correctly. applyZoom() does centering
    // math compatible with that origin. step is tightened for finer wheel zoom.
    panzoom = Panzoom(canvasContent, {{
      canvas: true,
      maxScale: 5,
      minScale: 0.05,
      step: 0.15,
      animate: false,
      cursor: 'grab',
    }});

    // Right-rail copy.
    if (artifact.kind === 'group' && artifact.copy_text) {{
      copyText.textContent = artifact.copy_text;
      document.body.classList.add('has-copy');
    }} else {{
      copyText.textContent = '';
      document.body.classList.remove('has-copy');
    }}

    setActive(artifact.id);

    if (location.hash !== '#' + artifact.id) {{
      if (opts && opts.replace) {{
        history.replaceState(null, '', '#' + artifact.id);
      }} else {{
        history.pushState(null, '', '#' + artifact.id);
      }}
    }}
  }}

  function artifactFromEntry(a) {{
    try {{
      return JSON.parse(a.dataset.artifact);
    }} catch (e) {{
      return null;
    }}
  }}

  function findArtifactById(id) {{
    const a = entries.find(function(el) {{ return el.dataset.id === id; }});
    return a ? artifactFromEntry(a) : null;
  }}

  entries.forEach(function(a) {{
    a.addEventListener('click', function(e) {{
      e.preventDefault();
      const artifact = artifactFromEntry(a);
      if (artifact) loadArtifact(artifact);
    }});
  }});

  openTabBtn.addEventListener('click', function() {{
    if (!currentArtifact) return;
    const k = currentArtifact.kind;
    if (k === 'single' || k === 'image' || k === 'pdf' || k === 'video') {{
      window.open(currentArtifact.url, '_blank');
    }} else if (k === 'group' && currentArtifact.slide_urls.length) {{
      // Group has no single canonical URL; open the first slide.
      window.open(currentArtifact.slide_urls[0], '_blank');
    }}
  }});

  window.addEventListener('hashchange', function() {{
    const id = location.hash.replace(/^#/, '');
    if (!id || (currentArtifact && currentArtifact.id === id)) return;
    const artifact = findArtifactById(id);
    if (artifact) loadArtifact(artifact, {{ replace: true }});
  }});

  // Initial selection: hash if present, else default.
  const initialId = location.hash.replace(/^#/, '') || DEFAULT_ID;
  const initialArtifact = findArtifactById(initialId);
  if (initialArtifact) {{
    loadArtifact(initialArtifact, {{ replace: true }});
  }} else {{
    breadcrumb.textContent = 'no preview HTML found in workspace';
  }}
}})();
</script>
</body>
</html>
"""


__all__ = ["scan_workspace", "render_hub_html"]
