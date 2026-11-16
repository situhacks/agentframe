"""Keeps a gzip copy of this vault's Claude Code chat transcripts, which Claude Code deletes after its
clean-up period (30 days by default).

Every `*.jsonl` (recursively, so `subagents/` too) under the Claude Code project folders that belong to
this vault is copied to `workspace/_archive/transcripts/<project folder>/<path inside it>.jsonl.gz`, gzip
level 6. A folder belongs to the vault when its name is the one Claude Code derives from the vault's path
(every character that is not a letter or a digit becomes `-`), or the one it derives from a worktree under
`.claude/worktrees/` (that name plus `--claude-worktrees-<name>`). A name that merely starts with the vault's
(`...AgentFrame-Marketing`) is another project and is left alone. On Windows the match ignores case.

State is `system/logs/transcript-archive-state.json`: each source's size and modified time as last copied.
Only a new or changed file is copied; a changed file replaces its copy (transcripts only grow). With no
record of a file whose copy already exists, the copy is refreshed when the source is newer than it (each copy
takes its source's modified time) or larger than it was (gzip's own length field), and never deleted. This
never deletes an archive copy and never writes to the Claude Code folders. `workspace/_archive/transcripts/
.gitignore` holds `*`, so `af save` never commits the archive.

Prints one line: `transcript-archive: N new, M updated, K unchanged · <MB> archived` (MB is the whole
archive's size on disk). A file that could not be read or written adds ` · F failed` and exit 1.

    python system/tools/transcript_archive.py

The vault is AF_ROOT or this file's own vault; Claude Code's folder is CLAUDE_CONFIG_DIR, else the user's
`.claude` (the rule `af` uses, reach.rs `config_dir`).
"""

from __future__ import annotations

import gzip
import json
import os
import re
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

VAULT = Path(__file__).resolve().parent.parent.parent
ARCHIVE = Path("workspace/_archive/transcripts")
STATE = Path("system/logs/transcript-archive-state.json")
WORKTREES = "--claude-worktrees-"   # `/.claude/worktrees/` after the same replacement
LEVEL = 6
CHUNK = 1 << 20


def config_dir() -> Path | None:
    """Claude Code's folder: CLAUDE_CONFIG_DIR, else USERPROFILE or HOME with `.claude` (reach.rs)."""
    if d := os.environ.get("CLAUDE_CONFIG_DIR"):
        return Path(d)
    home = os.environ.get("USERPROFILE") or os.environ.get("HOME")
    return Path(home) / ".claude" if home else None


def derive(path: str) -> str:
    """The project folder name Claude Code makes from an absolute path: each UTF-16 unit that is not
    an ASCII letter or digit becomes `-`."""
    return "".join(c if re.fullmatch(r"[A-Za-z0-9]", c) else "-" * (len(c.encode("utf-16-le")) // 2) for c in path)


def belongs(folder: str, derived: str, windows: bool) -> bool:
    """Whether a project folder is the vault's or one of its worktrees'."""
    marker = WORKTREES
    if windows:
        folder, derived, marker = folder.casefold(), derived.casefold(), marker.casefold()
    return folder == derived or folder.startswith(derived + marker)


def vault_folders(projects: Path, root: Path) -> list[Path]:
    derived = derive(os.path.abspath(root))
    try:
        kids = sorted(p for p in projects.iterdir() if p.is_dir())
    except OSError:
        return []
    return [p for p in kids if belongs(p.name, derived, os.name == "nt")]


def sources(folder: Path):
    """Every transcript under a project folder: (path, path relative to it with forward slashes)."""
    for dirpath, dirnames, files in os.walk(folder):
        dirnames.sort()
        for f in sorted(files):
            p = Path(dirpath, f)
            if f.endswith(".jsonl") and not p.is_symlink():
                yield p, p.relative_to(folder).as_posix()


def gz_size(path: Path) -> int | None:
    """The uncompressed length a gzip file records in its last four bytes, or None when unreadable."""
    try:
        with open(path, "rb") as f:
            f.seek(-4, os.SEEK_END)
            return int.from_bytes(f.read(4), "little")
    except OSError:
        return None


def stale_copy(src: Path, st: os.stat_result, copy: Path) -> bool:
    """A copy with no record: refresh it when the source is newer than the copy or larger than it holds."""
    try:
        c = copy.stat()
    except OSError:
        return True
    held = gz_size(copy)
    return st.st_mtime_ns > c.st_mtime_ns or held is None or st.st_size > held


def write_copy(src: Path, st: os.stat_result, copy: Path) -> None:
    """Gzip the source into place through a temporary file, so a copy is whole or absent, and stamp it with
    the source's modified time."""
    copy.parent.mkdir(parents=True, exist_ok=True)
    tmp = copy.with_name(copy.name + ".tmp")
    try:
        with open(src, "rb") as r, open(tmp, "wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw,
                                                                         compresslevel=LEVEL, mtime=0) as w:
            shutil.copyfileobj(r, w, CHUNK)
        os.utime(tmp, ns=(st.st_atime_ns, st.st_mtime_ns))
        os.replace(tmp, copy)
    finally:
        try:
            tmp.unlink()
        except OSError:
            pass


def load_state(path: Path) -> dict:
    try:
        d = json.loads(path.read_text(encoding="utf-8"))
        files = d.get("files") if isinstance(d, dict) else None
        return {k: v for k, v in files.items() if isinstance(v, list) and len(v) == 2} if isinstance(files, dict) else {}
    except (OSError, ValueError):
        return {}


def save_state(path: Path, files: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps({"files": files}, sort_keys=True), encoding="utf-8", newline="\n")
    os.replace(tmp, path)


@dataclass
class Result:
    new: int = 0
    updated: int = 0
    unchanged: int = 0
    failed: int = 0
    megabytes: float = 0.0

    def line(self) -> str:
        out = (f"transcript-archive: {self.new} new, {self.updated} updated, {self.unchanged} unchanged"
               f" · {self.megabytes:.1f} MB archived")
        return out + (f" · {self.failed} failed" if self.failed else "")


def run(root: Path, config: Path | None) -> Result:
    res = Result()
    out = root / ARCHIVE
    out.mkdir(parents=True, exist_ok=True)
    ignore = out / ".gitignore"
    try:
        has_star = "*" in ignore.read_text(encoding="utf-8").split()
    except OSError:
        has_star = False
    if not has_star:
        ignore.write_text("*\n", encoding="utf-8", newline="\n")
    state_path = root / STATE
    old = load_state(state_path)
    new_state: dict = dict(old)
    for folder in vault_folders(config / "projects", root) if config else []:
        for src, rel in sources(folder):
            key = f"{folder.name}/{rel}"
            copy = out / folder.name / (rel + ".gz")
            try:
                st = src.stat()
                seen = old.get(key)
                if seen is None:
                    if not copy.exists():
                        status = "new"
                    else:
                        status = "updated" if stale_copy(src, st, copy) else "unchanged"
                elif [st.st_size, st.st_mtime_ns] != seen:
                    status = "updated"
                else:
                    status = "unchanged" if copy.exists() else "new"
                if status != "unchanged":
                    write_copy(src, st, copy)
                new_state[key] = [st.st_size, st.st_mtime_ns]
                setattr(res, status, getattr(res, status) + 1)
            except OSError:
                res.failed += 1
    try:
        save_state(state_path, new_state)
    except OSError:
        res.failed += 1
    total = 0
    for dirpath, _, files in os.walk(out):
        for f in files:
            try:
                total += os.path.getsize(os.path.join(dirpath, f))
            except OSError:
                pass
    res.megabytes = total / 1e6
    return res


def main() -> int:
    root = Path(os.environ.get("AF_ROOT") or VAULT)
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
    except Exception:  # noqa: BLE001
        pass
    res = run(root, config_dir())
    print(res.line())
    return 1 if res.failed else 0


if __name__ == "__main__":
    sys.exit(main())
