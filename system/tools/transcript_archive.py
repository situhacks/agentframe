"""Keeps a gzip copy of this vault's Claude Code chat files, which Claude Code deletes after its
clean-up period (30 days by default).

Every file (recursively: the transcripts, `subagents/`, the saved `tool-results/` the transcripts point at)
under the Claude Code project folders that belong to this vault is copied to
`workspace/_archive/transcripts/<project folder>/<path inside it>.gz`, gzip level 6; a transcript
`x.jsonl` becomes `x.jsonl.gz`. Left out: `*.wakatime` and lock or temporary files (`*.lock`, `*.lck`,
`*.tmp`, `*.temp`, `*.swp`). A folder belongs to the vault when its name is the one Claude Code derives from
the vault's path (every character that is not a letter or a digit becomes `-`), or the one it derives from a
worktree under `.claude/worktrees/` (that name plus `--claude-worktrees-<name>`). A name that merely starts
with the vault's (`...AgentFrame-Marketing`) is another project, and so is a temporary-folder run (the batch's
own `...Temp-agentframe-*`): they are left alone. On Windows the match ignores case.

State is `system/logs/transcript-archive-state.json`: each source's size and modified time as last copied,
and the days each failing file has failed. Only a new or changed file is copied; a changed file replaces its
copy (transcripts only grow). A source that is smaller than the copy already holds is not allowed to replace
it: the old copy stays and the new content is saved beside it as `<name>.<source mtime yyyymmddHHMMSS>.gz`,
counted as kept. With no record of a file whose copy already exists, the copy is refreshed when the source is
newer than it (each copy takes its source's modified time) or larger than it holds (gzip's own length field),
and never deleted. This never deletes an archive copy and never writes to the Claude Code folders.
`workspace/_archive/transcripts/.gitignore` holds `*`, so `af save` never commits the archive; if it cannot
be written, nothing is copied.

Prints one line: `transcript-archive: N new, M updated, K unchanged[, J kept] · <MB> archived` (MB is the whole
archive's size on disk). A file that could not be read or written adds ` · F failed`.

Exit codes: 0 all done; 3 some files failed and the rest were copied (the batch shows it as a note); 1 the whole
run failed (no Claude project folder readable, the archive folder not writable, or nothing at all could be
copied). A file that has failed on 3 or more days gets one loop for main (`af open add`: which file, since
when, and that Claude Code deletes transcripts after 30 days); its record clears when it copies.

    python system/tools/transcript_archive.py

The vault is AF_ROOT or this file's own vault; Claude Code's folder is CLAUDE_CONFIG_DIR, else the user's
`.claude` (the rule `af` uses, reach.rs `config_dir`).
"""

from __future__ import annotations

import datetime as dt
import gzip
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

VAULT = Path(__file__).resolve().parent.parent.parent
ARCHIVE = Path("workspace/_archive/transcripts")
STATE = Path("system/logs/transcript-archive-state.json")
WORKTREES = "--claude-worktrees-"   # `/.claude/worktrees/` after the same replacement
LEVEL = 6
EXIT_PARTIAL = 3          # some files failed, the rest were copied (loop_watch.EXIT_PARTIAL, watch_batch reads it)
FAIL_DAYS = 3             # a file that has failed on this many days gets a loop
SKIP_SUFFIXES = (".wakatime", ".lock", ".lck", ".tmp", ".temp", ".swp")
LOOP_SECTION = "Waiting on others"
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
    kids = sorted(p for p in projects.iterdir() if p.is_dir())   # OSError: no Claude project folder to read
    return [p for p in kids if belongs(p.name, derived, os.name == "nt")]


def skipped(name: str) -> bool:
    return name.endswith(SKIP_SUFFIXES) or name.startswith("~")


def sources(folder: Path, on_error=None):
    """Every file under a project folder: (path, path relative to it with forward slashes)."""
    for dirpath, dirnames, files in os.walk(folder, onerror=on_error):
        dirnames.sort()
        for f in sorted(files):
            p = Path(dirpath, f)
            if not skipped(f) and not p.is_symlink():
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


def load_state(path: Path) -> tuple[dict, dict]:
    """(files, failing): each source's [size, mtime_ns] as copied, and each failing file's record."""
    try:
        d = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}, {}
    if not isinstance(d, dict):
        return {}, {}
    files, failing = d.get("files"), d.get("failing")
    files = {k: v for k, v in files.items() if isinstance(v, list) and len(v) == 2} if isinstance(files, dict) else {}
    failing = {k: v for k, v in failing.items() if isinstance(v, dict) and isinstance(v.get("days"), list)
               } if isinstance(failing, dict) else {}
    return files, failing


def write_atomic(path: Path, text: str) -> None:
    """A file is whole or absent: written to a temporary name and renamed over."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    try:
        tmp.write_text(text, encoding="utf-8", newline="\n")
        os.replace(tmp, path)
    finally:
        try:
            tmp.unlink()
        except OSError:
            pass


def save_state(path: Path, files: dict, failing: dict) -> None:
    write_atomic(path, json.dumps({"files": files, "failing": failing}, sort_keys=True))


def raise_loop(root: Path, text: str, today: dt.date) -> int:
    """One loop for main through `af open add`; af's exit code."""
    try:
        r = subprocess.run(["af", "open", "add", text, "--nudge", today.isoformat(), "--from", "transcript-archive",
                            "--section", LOOP_SECTION], capture_output=True, text=True, encoding="utf-8",
                           errors="replace", env={**os.environ, "AF_ROOT": str(root)}, cwd=root, timeout=120)
    except (OSError, subprocess.SubprocessError):
        return 127
    return r.returncode


@dataclass
class Result:
    new: int = 0
    updated: int = 0
    unchanged: int = 0
    kept: int = 0
    failed: int = 0
    megabytes: float = 0.0
    fatal: str = ""           # why the whole run failed, when it did
    loops: int = 0

    def line(self) -> str:
        if self.fatal:
            return f"transcript-archive: failed: {self.fatal}"
        kept = f", {self.kept} kept" if self.kept else ""
        out = (f"transcript-archive: {self.new} new, {self.updated} updated, {self.unchanged} unchanged{kept}"
               f" · {self.megabytes:.1f} MB archived")
        return out + (f" · {self.failed} failed" if self.failed else "")

    def code(self) -> int:
        done = self.new + self.updated + self.unchanged + self.kept
        if self.fatal or (self.failed and not done):
            return 1
        return EXIT_PARTIAL if self.failed else 0


def run(root: Path, config: Path | None, today: dt.date | None = None, add_loop=raise_loop) -> Result:
    res = Result()
    today = today or dt.date.today()
    out = root / ARCHIVE
    try:
        out.mkdir(parents=True, exist_ok=True)
        ignore = out / ".gitignore"
        try:
            has_star = "*" in ignore.read_text(encoding="utf-8").split()
        except OSError:
            has_star = False
        if not has_star:
            write_atomic(ignore, "*\n")
    except OSError as e:
        res.fatal = f"the archive folder is not writable ({type(e).__name__})"   # nothing is copied without the ignore file
        return res
    try:
        if config is None:
            raise FileNotFoundError("no Claude folder")
        folders = vault_folders(config / "projects", root)
    except OSError as e:
        res.fatal = f"no Claude Code project folder to read ({type(e).__name__})"
        return res
    state_path = root / STATE
    old, failing_old = load_state(state_path)
    new_state: dict = dict(old)
    failing: dict = {}
    unreadable = 0
    for folder in folders:
        top_failed = []

        def on_error(e, folder=folder, top_failed=top_failed):
            if Path(e.filename or "") == folder:
                top_failed.append(1)
            else:
                res.failed += 1
        for src, rel in sources(folder, on_error):
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
                    held = gz_size(copy) if status == "updated" else None
                    if held is not None and st.st_size < held:
                        # a source smaller than its copy never replaces it: the new content goes beside it
                        stamp = dt.datetime.fromtimestamp(st.st_mtime).strftime("%Y%m%d%H%M%S")
                        write_copy(src, st, copy.with_name(f"{copy.name[:-3]}.{stamp}.gz"))
                        status = "kept"
                    else:
                        write_copy(src, st, copy)
                new_state[key] = [st.st_size, st.st_mtime_ns]
                setattr(res, status, getattr(res, status) + 1)
            except OSError:
                if not src.exists():
                    continue   # Claude Code deleted it meanwhile; its copy, if any, stays
                res.failed += 1
                rec = failing_old.get(key) or {"days": [], "raised": False}
                if today.isoformat() not in rec["days"]:
                    rec["days"] = [*rec["days"], today.isoformat()]
                failing[key] = rec
        unreadable += bool(top_failed)
    if folders and unreadable == len(folders):
        res.fatal = "no Claude Code project folder of this vault could be read"
        return res
    for key, rec in failing.items():
        if len(rec["days"]) >= FAIL_DAYS and not rec["raised"]:
            text = (f"Transcript archive cannot copy {key}: failing since {rec['days'][0]} ({len(rec['days'])} days); "
                    f"Claude Code deletes transcripts after 30 days")
            if add_loop(root, text, today) == 0:
                rec["raised"] = True
                res.loops += 1
    try:
        save_state(state_path, new_state, failing)
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
    return res.code()


if __name__ == "__main__":
    sys.exit(main())
