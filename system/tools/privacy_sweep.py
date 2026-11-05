"""Check a branch's diff for private material before it can merge.

Both repositories are public, so a tracked file carries no credential, client name, or private
fact. Compares <base>...HEAD plus anything uncommitted and reports added lines that hold a
credential shape, a workspace path, or a private term built from the vault itself: a project
slug, a pipeline application slug, or a person or place page's name. Exit 0 clean, 1 with hits.

Checks the folder it runs in, so a reviewer runs the base checkout's copy from inside the branch's
worktree, not the copy the branch can edit. A real name that a public file must carry goes in
--allow on that one run, never in this file.

    python <base checkout>/system/tools/privacy_sweep.py [--base master] [--vault <path>] [--allow "<term>"]...
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

CREDENTIALS = (
    ("api key", re.compile(r"\b(?:sk-[A-Za-z0-9_-]{20,}|ghp_[A-Za-z0-9]{20,}|xox[bap]-[A-Za-z0-9-]{20,}|AKIA[0-9A-Z]{16})\b")),
    ("secret assignment", re.compile(r"(?i)\b(?:api[_-]?key|secret|password|token)\b\s*[:=]\s*['\"][^'\"\s<>{}$]{12,}['\"]")),
    ("account number", re.compile(r"\b\d{4}[ -]\d{4}[ -]\d{4}[ -]\d{1,4}\b")),
)
FORBIDDEN_PATHS = re.compile(r"^(?:workspace/|system/sessions/|system/logs/|system/backlog|\.env$)")
# Words too common to flag even when a page or project happens to carry them.
COMMON = {
    "completed", "readme", "template", "groups", "research", "pipeline", "projects", "studio", "main",
    "park", "central", "mark",
}
MIN_TERM = 5


def added_lines(diff: str) -> list[tuple[str, str]]:
    """(path, line) for each added line of a unified diff."""
    out = []
    path = ""
    for line in diff.splitlines():
        if line.startswith("+++ "):
            path = line[4:].removeprefix("b/").strip()
        elif line.startswith("+") and not line.startswith("+++"):
            out.append((path, line[1:]))
    return out


def vault_terms(vault: Path) -> set[str]:
    """Private terms from the vault's own folders; none when there is no workspace here."""
    work = vault / "workspace"
    terms: set[str] = set()
    for rel in ("projects", "projects/completed", "pipeline/applications", "pipeline/applications/completed"):
        folder = work / rel
        if folder.is_dir():
            terms.update(p.name.lower() for p in folder.iterdir() if p.is_dir())
    for rel in ("life/people", "life/people/groups", "life/places"):
        folder = work / rel
        if folder.is_dir():
            for p in folder.glob("*.md"):
                terms.add(p.stem.lower())
                terms.update(page_names(p))
    return {t for t in terms if len(t) >= MIN_TERM and t not in COMMON}


def page_names(page: Path) -> set[str]:
    """A page's `name:` field and its title, when they are more than one word."""
    names: set[str] = set()
    try:
        head = page.read_text(encoding="utf-8", errors="replace")[:2000]
    except OSError:
        return names
    for pat in (r"(?m)^name:\s*(.+)$", r"(?m)^#\s+(.+)$"):
        m = re.search(pat, head)
        if m:
            value = m.group(1).strip().strip("\"'").lower()
            if len(value.split()) > 1:
                names.add(value)
    return names


def sweep(lines: list[tuple[str, str]], terms: set[str], allow: set[str]) -> list[str]:
    hits = []
    seen_paths = set()
    # A term may start a longer slug, which still names the same thing; it may not end one.
    patterns = [(t, re.compile(rf"(?<![A-Za-z0-9-]){re.escape(t)}(?![A-Za-z0-9])", re.I)) for t in sorted(terms - allow)]
    for path, line in lines:
        if path not in seen_paths:
            seen_paths.add(path)
            if FORBIDDEN_PATHS.match(path):
                hits.append(f"{path}: a private path in the diff")
        for label, rx in CREDENTIALS:
            if rx.search(line):
                hits.append(f"{path}: {label}: {line.strip()[:120]}")
        for term, rx in patterns:
            if rx.search(line):
                hits.append(f"{path}: private term '{term}': {line.strip()[:120]}")
    return hits


def diff_text(base: str, cwd: Path) -> str:
    def run(*args: str) -> str:
        r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        if r.returncode != 0:
            raise SystemExit(f"privacy_sweep: git {' '.join(args)} failed: {r.stderr.strip()}")
        return r.stdout

    parts = [run("diff", f"{base}...HEAD"), run("diff", "HEAD")]
    for rel in run("ls-files", "--others", "--exclude-standard").splitlines():
        # Untracked files count: they are what the next commit would add.
        text = (cwd / rel).read_text(encoding="utf-8", errors="replace")
        parts.append(f"+++ b/{rel}\n" + "".join(f"+{line}\n" for line in text.splitlines()))
    return "\n".join(parts)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--base", default="master")
    ap.add_argument("--vault", default=None, help="where workspace/ lives; defaults to this checkout")
    ap.add_argument("--allow", action="append", default=[], help="a term to skip, lower case")
    args = ap.parse_args(argv)
    vault = Path(args.vault) if args.vault else ROOT
    terms = vault_terms(vault)
    hits = sweep(added_lines(diff_text(args.base, Path.cwd())), terms, {a.lower() for a in args.allow})
    if not (vault / "workspace").is_dir():
        print(f"privacy_sweep: no workspace under {vault}; credential and path checks only; pass --vault for the private terms")
    for hit in hits:
        print(hit)
    print(f"privacy_sweep: {len(hits)} hit(s) against {len(terms)} private term(s)")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
