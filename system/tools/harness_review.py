"""The harness review: what in the harness looks unused or stale, written as one report.

Read-only. It changes nothing but its own report (system/logs/harness-review-YYYY-MM-DD.md) and
its own state (system/logs/harness-review-state.json). No model call, no network. The morning batch
run (watch_batch.py) starts it every day; its own state decides whether it does anything: a report
younger than 14 days means "not due" and exit 0. When a report is written and it holds items worth
a look, it raises one loop for main, unless a loop from `harness-review` is already open.

It finds and lists; a builder chat acts on the report. Eleven checks: the words each chat loads at
its start, pages with items to fix, git branches and worktrees, session rows and folders, skills,
tools, process files, the backlog, scratch folders, the age of the model facts, and what came back:
backlog rows in the window that touch the same paths as a row completed before it, and learnings
entries whose guard path is gone. The chat that reads the report writes the root cause into
system/learnings.md.

    python system/tools/harness_review.py [--root <repo>] [--out <file>] [--force]
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
REPORT_DIR = "system/logs"
STATE = "system/logs/harness-review-state.json"
REGISTRY = "system/sessions/registry.md"
SESSIONS = "system/sessions"
BACKLOG = "system/backlog.md"
BACKLOG_DONE = "system/backlog-completed.md"
LEARNINGS = "system/learnings.md"
WORKSPACE_LEARNINGS = "workspace/learnings.md"  # the private log of project and life-layer mistakes; absent on a fresh clone
MODELS = "library/schema/models.md"
SCRATCH_GLOB = r"C:\tmp\af-*"
LOOP_FROM = "harness-review"
DUE_DAYS = 14             # the least time between two reports
WORDS_FLAG = 3500         # a chat's words at start, over which it is flagged
MAX_ROWS = 15             # rows a list shows before "and N more"
ENDED_DAYS = 14           # an ended registry row older than this is listed
BACKLOG_DAYS = 30
SCRATCH_DAYS = 14
WORKTREE_DAYS = 7         # a worktree whose last commit is newer than this is still in use
MODELS_DAYS = 60
AF_TIMEOUT_S = 60
ROLE_DIRS = ("life", "project", "system", "worker")
ROUTERS = {"operator": "AGENTS.operator.md", "builder": "AGENTS.builder.md", "worker": "AGENTS.worker.md"}
TEXT_EXT = (".md", ".py", ".ps1", ".rs", ".json", ".toml", ".yml", ".yaml", ".txt")
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")


class ReviewError(RuntimeError):
    """The review could not be made."""


# --- The outside world: af and git ------------------------------------------------

def af(args: list[str], root: Path) -> tuple[int, str, str]:
    """One af call: exit code, stdout, stderr. No af on the path is exit 127."""
    try:
        r = subprocess.run(["af", *args], capture_output=True, text=True, encoding="utf-8", errors="replace",
                           env={**os.environ, "AF_ROOT": str(root)}, cwd=root, timeout=AF_TIMEOUT_S)
    except FileNotFoundError:
        return 127, "", "af is not on the path"
    except OSError as e:
        return 127, "", f"af could not start: {e}"
    except subprocess.TimeoutExpired:
        return 124, "", f"af timed out after {AF_TIMEOUT_S}s"
    return r.returncode, r.stdout, r.stderr


def git(root: Path, *args: str) -> str:
    try:
        r = subprocess.run(["git", "--no-optional-locks", "-C", str(root), *args], capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=120,
                           env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"})
    except (OSError, subprocess.SubprocessError) as e:
        raise ReviewError(f"git {args[0]} failed: {e}") from None
    if r.returncode != 0:
        raise ReviewError(f"git {' '.join(args)} failed: {r.stderr.strip() or r.returncode}")
    return r.stdout


# --- Small helpers ------------------------------------------------------------------

def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def words(path: Path) -> int:
    """Words in a file; a missing file counts 0."""
    return len(read(path).split())


def first_date(text: str) -> dt.date | None:
    m = DATE.search(text)
    try:
        return dt.date.fromisoformat(m.group(0)) if m else None
    except ValueError:
        return None


def capped(lines: list[str]) -> list[str]:
    return lines if len(lines) <= MAX_ROWS else lines[:MAX_ROWS] + [f"and {len(lines) - MAX_ROWS} more"]


def bullets(lines: list[str]) -> list[str]:
    return [x if x.startswith("and ") and x.endswith(" more") else f"- {x}" for x in capped(lines)]


def table(head: list[str], rows: list[list[str]]) -> list[str]:
    out = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    shown = capped(["\x00".join(r) for r in rows])
    for r in shown:
        out.append("| " + " | ".join(r.split("\x00")) + " |" if "\x00" in r else r)
    return out


def mentioned(name_patterns: list[str], text: str) -> bool:
    return any(re.search(rf"(?<![\w-]){re.escape(n)}(?![\w-])", text) for n in name_patterns)


def tracked_text(root: Path) -> dict[str, str]:
    """Every tracked text file: its path (forward slashes) and its text."""
    out = {}
    for rel in git(root, "ls-files", "-z").split("\0"):
        if rel.endswith(TEXT_EXT):
            out[rel] = read(root / rel)
    return out


# --- The registry ---------------------------------------------------------------------

def read_registry(root: Path) -> tuple[list[dict], bool]:
    """The registry's table rows (shape: library/schema/sessions-registry.md): eight cells a row.
    The flag is true when the file holds a table but no row could be read from it."""
    rows, odd = [], False
    for line in read(root / REGISTRY).splitlines():
        t = line.strip()
        if not (t.startswith("|") and t.endswith("|")):
            continue
        cells = [c.strip() for c in t[1:-1].split("|")]
        if cells[0] == "name" and len(cells) == 8 or set("".join(cells)) <= set("-: "):
            continue
        if len(cells) != 8 or not cells[0]:
            odd = True
            continue
        rows.append(dict(zip(("name", "role", "session_id", "model", "started", "last_seen", "status", "handoff"), cells)))
    return rows, odd and not rows


def ended_on(row: dict) -> dt.date | None:
    if not row["status"].startswith("ended"):
        return None
    return first_date(row["status"])


# --- The checks: each returns (items worth a look, lines) -------------------------------

def check_words(root: Path, rows: list[dict]) -> tuple[int, list[str]]:
    fixed = ["AGENTS.md", *ROUTERS.values(), "workspace/life/life.md"]
    count = {f: words(root / f) for f in fixed}
    out = table(["File every chat loads", "Words"], [[f, str(count[f])] for f in fixed])
    chats, flagged = [], 0
    for r in rows:
        if r["status"].startswith("ended"):
            continue
        name = r["name"]
        base = count["AGENTS.md"] + count[ROUTERS[r["role"]]] if r["role"] in ROUTERS else count["AGENTS.md"]
        if r["role"] == "main":
            base += count["workspace/life/life.md"]
        brief, page = words(root / SESSIONS / name / "brief.md"), words(root / SESSIONS / name / "handoff.md")
        total = base + brief + page
        flagged += total > WORDS_FLAG
        chats.append([name, r["role"], str(base), str(brief), str(page), str(total),
                      f"over {WORDS_FLAG}" if total > WORDS_FLAG else ""])
    if chats:
        out += ["", *table(["Chat", "Role", "Routers", "Brief", "Page", "Total", "Flag"], chats)]
    return flagged, out


def check_pages(root: Path, run_af) -> tuple[int, list[str]]:
    try:
        code, out, err = run_af(["session", "tidy", "--all", "--check"], root)
    except OSError:
        code, out, err = 127, "", ""
    if code == 127:
        return 0, ["af is not on the path; the pages were not checked."]
    if code not in (0, 1):
        return 0, [f"af session tidy exited {code}: {(err or out).strip()[:160]}"]
    blocks, cur = [], None
    for line in out.splitlines():
        if not line.strip():
            continue
        if line[0] not in " \t":
            cur = [line.strip()]
            blocks.append(cur)
        elif cur is not None and re.match(r"\s*\[(fix|review)\]", line):
            cur.append(line.strip())
    lines = [f"{b[0]} {x}" for b in blocks for x in b[1:]]
    return len(lines), bullets(lines) or ["nothing"]


def branch_lines(root: Path, *args: str) -> list[str]:
    """Local branch lines; a detached HEAD, which git prints as "(HEAD detached at ...)", is no branch."""
    return [x.strip() for x in git(root, "branch", *args).splitlines() if x.strip() and not x.strip().startswith("(")]


def check_git(root: Path, today: dt.date) -> tuple[int, list[str]]:
    merged = [b for b in branch_lines(root, "--merged", "master", "--format=%(refname:short)") if b != "master"]
    unmerged = []
    for line in branch_lines(root, "--no-merged", "master", "--format=%(refname:short)|%(committerdate:short)"):
        name, _, date = line.partition("|")
        unmerged.append(f"{name} (last commit {date})")
    trees, block = [], {}
    for line in (git(root, "worktree", "list", "--porcelain") + "\n").splitlines():
        if line.strip():
            k, _, v = line.partition(" ")
            block[k] = v
        elif block:
            trees.append(block)
            block = {}
    in_use = {t["branch"].removeprefix("refs/heads/"): t["worktree"] for t in trees if t.get("branch")}
    free = [b for b in merged if b not in in_use]
    busy = [f"{b} (in use by worktree {in_use[b]})" for b in merged if b in in_use]
    removable = []
    for t in trees[1:]:
        branch = t.get("branch", "").removeprefix("refs/heads/")
        if not branch or branch not in merged:
            continue
        try:
            dirty = git(Path(t["worktree"]), "status", "--porcelain").strip()
            stamp = int(git(Path(t["worktree"]), "log", "-1", "--format=%ct").strip())
        except (ReviewError, ValueError):
            continue
        last = dt.datetime.fromtimestamp(stamp).date()
        if not dirty and (today - last).days > WORKTREE_DAYS:
            removable.append(f"{t['worktree']} ({branch}, last commit {last.isoformat()})")
    out = [f"**Merged into master (no commits of its own) ({len(free)})**", *(bullets(free) or ["nothing"]), "",
           f"**Merged, but checked out in a worktree, listed for information, not counted ({len(busy)})**",
           *(bullets(busy) or ["nothing"]), "",
           f"**Worktrees on a merged branch, no changes, last commit over {WORKTREE_DAYS} days ago ({len(removable)})**",
           *(bullets(removable) or ["nothing"]), "",
           f"**Not merged, listed for information, not counted ({len(unmerged)})**", *(bullets(unmerged) or ["nothing"])]
    return len(free) + len(removable), out


def session_folders(root: Path) -> list[str]:
    """Chat names with a folder holding a brief or a page: <role>/<name>, or a top-level name."""
    base = root / SESSIONS
    names = []
    if not base.is_dir():
        return names
    for d in sorted(p for p in base.iterdir() if p.is_dir()):
        candidates = [(f"{d.name}/{c.name}", c) for c in sorted(d.iterdir()) if c.is_dir()] if d.name in ROLE_DIRS else [(d.name, d)]
        names += [n for n, p in candidates if (p / "brief.md").is_file() or (p / "handoff.md").is_file()]
    return names


def check_sessions(root: Path, rows: list[dict], today: dt.date, unreadable: bool = False) -> tuple[int, list[str]]:
    if unreadable:
        return 0, [f"{REGISTRY} holds a table but no row could be read from it (unexpected columns); "
                   "rows and folders were not compared."]
    old = []
    for r in rows:
        d = ended_on(r)
        if d and (today - d).days > ENDED_DAYS:
            old.append(f"{r['name']} (ended {d.isoformat()})")
    known = {r["name"] for r in rows}
    orphans = [n for n in session_folders(root) if n not in known]
    missing = [r["name"] for r in rows if not (root / SESSIONS / r["name"]).is_dir()]
    out = [f"**Ended more than {ENDED_DAYS} days ago ({len(old)})**", *(bullets(old) or ["nothing"]), "",
           f"**A folder with a brief or a page and no registry row ({len(orphans)})**", *(bullets(orphans) or ["nothing"]), "",
           f"**A registry row whose folder is missing ({len(missing)})**", *(bullets(missing) or ["nothing"])]
    return len(old) + len(orphans) + len(missing), out


def check_skills(root: Path, files: dict[str, str]) -> tuple[int, list[str]]:
    unused, listing = [], []
    for skill in sorted((root / ".claude/skills").glob("*/SKILL.md")):
        name = skill.parent.name
        text = read(skill)
        body = re.sub(r"\A---\r?\n.*?\r?\n---\r?\n", "", text, count=1, flags=re.S)
        listing.append([name, str(len(body.split()))])
        own = f".claude/skills/{name}/"
        if not any(mentioned([name], t) for p, t in files.items()
                   if not p.startswith(own) and p not in (".claude/skills/README.md", "README.md")):
            unused.append(name)
    out = [f"**Not referenced by a router, a process file or an agent ({len(unused)})**", *(bullets(unused) or ["nothing"]), "",
           *table(["Skill", "Body words"], listing)] if listing else ["nothing"]
    return len(unused), out


def check_tools(root: Path, files: dict[str, str]) -> tuple[int, list[str]]:
    unused = []
    tools = sorted(p for p in (root / "system/tools").glob("*") if p.suffix in (".py", ".ps1"))
    for p in tools:
        rel = f"system/tools/{p.name}"
        stem = p.stem
        norm = stem.replace("-", "_")
        own_test = f"system/tests/test_{norm}.py"
        pats = list(dict.fromkeys([p.name, stem, norm, stem.replace("_", "-")]))
        if not any(mentioned(pats, t) for q, t in files.items() if q not in (rel, own_test)):
            unused.append(p.name)
    return len(unused), bullets(unused) or ["nothing"]


def check_process(root: Path, files: dict[str, str]) -> tuple[int, list[str]]:
    only_catalog = []
    for p in sorted((root / "library/process").glob("*.md")):
        if p.name == "README.md":
            continue
        rel = f"library/process/{p.name}"
        if not any(mentioned([p.name], t) for q, t in files.items() if q not in (rel, "library/process/README.md")):
            only_catalog.append(p.name)
    return len(only_catalog), bullets(only_catalog) or ["nothing"]


def check_backlog(root: Path, today: dt.date) -> tuple[int, list[str]]:
    path = root / BACKLOG
    if not path.is_file():
        return 0, ["There is no backlog file."]
    heads = [(m.start(), m.group(1), m.group(2)) for m in re.finditer(r"(?m)^## (BB-\d{4}-\d{2}-\d{2}-\d{2})\s*·?\s*(.*)$", read(path))]
    text = read(path)
    stale = []
    for i, (pos, bid, title) in enumerate(heads):
        block = text[pos:heads[i + 1][0] if i + 1 < len(heads) else len(text)]
        m = re.search(r"(?m)^Surfaced:(.*)$", block)
        d = (first_date(m.group(1)) if m else None) or first_date(bid)
        if d and (today - d).days > BACKLOG_DAYS:
            stale.append(f"{bid} {title.strip()} (surfaced {d.isoformat()})")
    out = [f"{len(heads)} rows; {len(stale)} surfaced more than {BACKLOG_DAYS} days ago.", *bullets(stale)]
    return len(stale), out


def check_scratch(today: dt.date, scratch_glob: str) -> tuple[int, list[str]] | None:
    if not Path(scratch_glob).parent.is_dir():
        return None
    found = glob.glob(scratch_glob)
    old = []
    for f in sorted(found):
        try:
            age = (today - dt.date.fromtimestamp(os.path.getmtime(f))).days
        except OSError:
            continue
        if age >= SCRATCH_DAYS:
            old.append(f"{f} ({age} days)")
    out = [f"{len(found)} entries; {len(old)} not changed for {SCRATCH_DAYS} days.", *bullets(old)]
    return (1 if old else 0), out


def check_models(root: Path, today: dt.date) -> tuple[int, list[str]] | None:
    m = re.search(r"(?m)^checked:\s*[\"']?(\d{4}-\d{2}-\d{2})", read(root / MODELS))
    if not m:
        return None
    d = first_date(m.group(1))
    if d is None:
        return None
    age = (today - d).days
    if age <= MODELS_DAYS:
        return 0, [f"The model facts were checked {d.isoformat()}, {age} days ago: current."]
    return 1, [f"The model facts were checked {d.isoformat()}, {age} days ago. A builder chat reads Anthropic's model pages again."]


# --- What came back ---------------------------------------------------------------------

ROW_HEAD = re.compile(r"(?m)^## (BB-\d{4}-\d{2}-\d{2}-\d{2})\s*·?\s*(.*)$")
LEARNING_HEAD = re.compile(r"(?m)^## (L-\d{4}-\d{2}-\d{2}-\d{2})\s*·?\s*(.*)$")


def norm_path(p: str) -> str:
    p = p.strip().strip("`").replace("\\", "/")
    while p.startswith("./"):
        p = p[2:]
    return p


PATH_ROOTS = ("system", "library", "workspace", ".claude", ".github", "docs")
PATH_TOKEN = re.compile(r"^[\w.@\-/\\]+$")
FILE_EXT = re.compile(r"[\w\-]\.[A-Za-z0-9]{1,5}$")


def is_path_like(tok: str) -> bool:
    """A word that names a file or folder: it ends in a file extension or a slash, or it starts in a repo root."""
    if not PATH_TOKEN.match(tok):
        return False
    tok = tok.replace("\\", "/")
    return tok not in NOT_PATHS and (bool(FILE_EXT.search(tok)) or tok.endswith("/") or ("/" in tok and tok.split("/")[0] in PATH_ROOTS))


def surface_paths(line: str) -> list[str]:
    """The paths on a `Surfaces:` line. Parenthetical prose goes first, backticks are only unwrapped, and a word
    counts only when it looks like a path, so "(lifted, the old one)" and "or a new places.rs" invent none."""
    line = re.sub(r"\([^)]*\)", " ", line).replace("`", " ")
    found = []
    for word in re.split(r"[\s,;]+", line):
        word = word.lstrip("'\"").rstrip("'\".:")
        if not is_path_like(word):
            continue
        n = norm_path(word)
        if n and n not in found:
            found.append(n)
    return found


def backlog_rows(text: str, done: bool) -> list[dict]:
    """Rows of one backlog file: id, title, the date it surfaced (else the id's), `Done:` date, `Surfaces:` paths."""
    heads = list(ROW_HEAD.finditer(text))
    rows = []
    for i, h in enumerate(heads):
        block = text[h.end():heads[i + 1].start() if i + 1 < len(heads) else len(text)]
        sm = re.search(r"(?m)^Surfaced:(.*)$", block)
        dm = re.search(r"(?m)^Done:(.*)$", block)
        pm = re.search(r"(?m)^Surfaces:(.*)$", block)
        surfaced = (first_date(sm.group(1)) if sm else None) or first_date(h.group(1))
        rows.append({"id": h.group(1), "title": h.group(2).strip(), "surfaced": surfaced, "completed": done,
                     "done": (first_date(dm.group(1)) if dm else None) or (surfaced if done else None),
                     "paths": surface_paths(pm.group(1)) if pm else []})
    return rows


# Touched by almost every fix, so a shared one says nothing. The report's header line names them.
COMMON_FOLDERS = ("system/tests/", "system/af/tests/")
COMMON_FILES = ("README.md", "CHANGELOG.md", "library/schema/README.md", "system/af/src/cli.rs", "system/af/src/lib.rs")
NOT_PATHS = ("e.g", "i.e", "etc")


def common(path: str) -> bool:
    return path.startswith(COMMON_FOLDERS) or path in COMMON_FILES


def is_folder(path: str) -> bool:
    """A path with a trailing slash, or whose last part has no extension (`system/af`), is a folder."""
    last = path.rstrip("/").rsplit("/", 1)[-1]
    return path.endswith("/") or "." not in last


def covers(folder: str, path: str) -> bool:
    return path == folder or path.startswith(folder.rstrip("/") + "/")


def shared_paths(a: list[str], b: list[str]) -> list[str]:
    """Paths both lists name, the common ones left out. A folder covers what is under it, with or without its
    trailing slash; the longer path is the one shown."""
    out = []
    for x in a:
        for y in b:
            if common(x) or common(y):
                continue
            hit = x == y or (is_folder(y) and covers(y, x)) or (is_folder(x) and covers(x, y))
            shown = x if len(x) >= len(y) else y
            if hit and shown not in out:
                out.append(shown)
    return out


def parse_learnings(text: str) -> list[dict]:
    """Entries of system/learnings.md (shape: library/schema/learnings.md): id, title, the Guard line and its paths.
    A guard path is a backticked word with a folder in it or a known file extension, minus a trailing `::name` or `#anchor`."""
    heads = list(LEARNING_HEAD.finditer(text))
    out = []
    for i, h in enumerate(heads):
        block = text[h.end():heads[i + 1].start() if i + 1 < len(heads) else len(text)]
        gm = re.search(r"(?m)^Guard:(.*)$", block)
        guard = gm.group(1).strip() if gm else ""
        paths = []
        for tok in re.findall(r"`([^`]+)`", guard):
            tok = norm_path(re.split(r"::|#", tok)[0])
            if tok and " " not in tok and ("/" in tok or tok.endswith(TEXT_EXT)) and tok not in paths:
                paths.append(tok)
        out.append({"id": h.group(1), "title": h.group(2).strip(), "guard": guard, "paths": paths})
    return out


def check_came_back(root: Path, today: dt.date) -> tuple[int, list[str]]:
    start = today - dt.timedelta(days=DUE_DAYS)
    done_rows = backlog_rows(read(root / BACKLOG_DONE), True)
    seen = {r["id"] for r in done_rows}
    open_rows = [r for r in backlog_rows(read(root / BACKLOG), False) if r["id"] not in seen]
    window = [r for r in [*open_rows, *done_rows] if r["surfaced"] and r["surfaced"] >= start]
    before = [r for r in done_rows if r["done"] and r["done"] < start]
    pairs = []
    for new in window:
        for old in before:
            if new["id"] == old["id"]:
                continue
            common = shared_paths(new["paths"], old["paths"])
            if common:
                pairs.append(f"{new['id']} ({'completed' if new['completed'] else 'open'}, surfaced {new['surfaced'].isoformat()}) "
                             f"touches what {old['id']} (done {old['done'].isoformat()}) fixed: "
                             + ", ".join(f"`{c}`" for c in common))
    gone, logs = [], []
    for log in (LEARNINGS, WORKSPACE_LEARNINGS):
        for e in parse_learnings(read(root / log)):
            missing = [p for p in e["paths"] if not (root / p).exists()]
            if missing:
                if log not in logs:
                    logs.append(log)
                gone.append(f"`{log}` {e['id']} {e['title']}: guard path gone: " + ", ".join(f"`{m}`" for m in missing))
    out = ["Paths left out of the match, since almost every fix touches them: "
           + ", ".join(f"`{p}`" for p in (*COMMON_FOLDERS, *COMMON_FILES)) + ".", "",
           f"**Rows since {start.isoformat()} that touch the same paths as a row completed before it ({len(pairs)})**",
           *(bullets(pairs) or ["nothing"]), "",
           f"**Learnings entries whose guard path no longer exists ({len(gone)})**", *(bullets(gone) or ["nothing"])]
    if pairs or gone:
        # Rows that came back belong in the tracked log; a listed entry belongs in its own file.
        files = [LEARNINGS] if pairs else []
        files += [f for f in logs if f not in files]
        out += ["", "Write each root cause into " + " and ".join(f"`{f}`" for f in files) + ", Guard first."]
    return len(pairs) + len(gone), out


# --- The report -------------------------------------------------------------------------

def build(root: Path, today: dt.date, run_af, scratch_glob: str) -> tuple[int, str]:
    rows, unreadable = read_registry(root)
    files = tracked_text(root)
    sections = [
        ("Words loaded at session start", check_words(root, rows)),
        ("Pages with items to fix or review", check_pages(root, run_af)),
        ("Git", check_git(root, today)),
        ("Sessions", check_sessions(root, rows, today, unreadable)),
        ("Skills", check_skills(root, files)),
        ("Tools no file mentions", check_tools(root, files)),
        ("Process files named only by the catalog", check_process(root, files)),
        ("Backlog", check_backlog(root, today)),
        ("Scratch folders", check_scratch(today, scratch_glob)),
        ("Model facts", check_models(root, today)),
        ("Came back", check_came_back(root, today)),
    ]
    total = sum(s[1][0] for s in sections if s[1])
    out = [f"# Harness review {today.isoformat()}: {total} item{'' if total == 1 else 's'} worth a look", "",
           "Written by `system/tools/harness_review.py`. It lists; a builder chat decides what goes.", ""]
    for n, (title, res) in enumerate(sections, 1):
        if res is None:
            continue
        count, lines = res
        out += [f"## {n}. {title} ({count})", "", *lines, ""]
    return total, "\n".join(out).rstrip() + "\n"


def loop_open(root: Path) -> bool:
    return any(line.lstrip().startswith("- [ ]") and f"from: {LOOP_FROM}" in line
               for line in read(root / "workspace/life/open.md").splitlines())


def loop_text(items: int, report: str) -> str:
    return (f"Harness review: {items} items to look at ({report}); dispatch a builder chat for what should go")


def load_state(root: Path) -> dict | None:
    try:
        d = json.loads(read(root / STATE))
        dt.date.fromisoformat(d["date"])
        return d
    except (ValueError, KeyError, TypeError):
        return None


def save_state(root: Path, today: dt.date, report: str, items: int, pending: str = "") -> None:
    """`loop_pending` holds the loop text when the loop could not be raised, else it is false."""
    path = root / STATE
    path.parent.mkdir(parents=True, exist_ok=True)
    d = {"date": today.isoformat(), "report": report, "items": items, "loop_pending": pending or False}
    path.write_text(json.dumps(d), encoding="utf-8", newline="\n")


def raise_loop(root: Path, run_af, text: str, today: dt.date) -> tuple[bool, str]:
    """`af open add` for main; an af that is missing or cannot start is a failure to retry, never a crash."""
    try:
        code, _, err = run_af(["open", "add", text, "--nudge", today.isoformat(), "--from", LOOP_FROM], root)
    except OSError as e:
        code, err = 127, str(e)
    return code == 0, f"({code}) {err.strip()[:160]}"


def main(argv: list[str] | None = None, *, today: dt.date | None = None, run_af=af, scratch_glob: str | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", help="the repository (default: AF_ROOT, else the one this file sits in)")
    ap.add_argument("--out", help="the report file (default: system/logs/harness-review-<date>.md)")
    ap.add_argument("--force", action="store_true", help="write a report even when the last one is younger than 14 days")
    args = ap.parse_args(argv)
    root = Path(args.root or os.environ.get("AF_ROOT") or ROOT).resolve()
    today = today or dt.date.today()
    state = load_state(root)
    last = dt.date.fromisoformat(state["date"]) if state else None
    if state and isinstance(state.get("loop_pending"), str) and not args.force:
        # The report is written; only its loop is owed.
        if loop_open(root):
            ok, why = True, ""
        else:
            ok, why = raise_loop(root, run_af, state["loop_pending"], today)
        if not ok:
            print(f"harness_review: the loop for the report of {state['date']} is still not raised, af open add failed {why}",
                  file=sys.stderr)
            return 1
        save_state(root, last, state.get("report", ""), state.get("items", 0))
        print(f"harness_review: raised the loop for the report of {state['date']}")
        return 0
    if last and 0 <= (today - last).days < DUE_DAYS and not args.force:
        print(f"harness_review: not due (last report {last.isoformat()}, next from {(last + dt.timedelta(days=DUE_DAYS)).isoformat()})")
        return 0
    try:
        items, text = build(root, today, run_af, scratch_glob or SCRATCH_GLOB)
    except ReviewError as e:
        print(f"harness_review: {e}", file=sys.stderr)
        return 2
    rel = f"{REPORT_DIR}/harness-review-{today.isoformat()}.md"
    out = Path(args.out) if args.out else root / rel
    if not out.is_absolute():
        out = root / out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="\n")
    if items and not loop_open(root):
        loop = loop_text(items, rel)
        ok, why = raise_loop(root, run_af, loop, today)
        if not ok:
            # The report stands as this period's; the next runs retry the loop and write no new report.
            save_state(root, today, str(out), items, pending=loop)
            print(f"harness_review: wrote {out}, but af open add failed {why}", file=sys.stderr)
            return 1
    save_state(root, today, str(out), items)
    print(f"harness_review: wrote {out}; {items} item(s) worth a look")
    return 0


if __name__ == "__main__":
    sys.exit(main())
