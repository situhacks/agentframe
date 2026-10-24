"""Parity cases for `af index` and `af search` (stage R3), run by parity.py.

These need their own comparison: the database file differs byte for byte between two
writers (page layout), so instead of the tree diff each case snapshots the tables row by
row at chosen steps — embeddings within 1e-6 — and compares every step's exit code,
stdout, and the `af index:` / `af search:` / `af: ERROR:` lines on stderr.

Step kinds:
  [argv...]            run on each side's own engine (Python / Rust binary)
  ("py", [argv...])    run the Python on both sides: on the Rust side it reads the
                       database the binary wrote
  ("dead", [argv...])  run with Ollama unreachable (Python: HTTP_PROXY to a dead port;
                       Rust: AF_OLLAMA_URL to the same port)
  ("db",)              snapshot the tables
  callable(root)       mutate the fixture (identically on both sides; mtimes pinned)

Cases needing Ollama print SKIP when it is not serving; the live case needs the live
index (system/index/vault.db in the main checkout) and is read-only against it.
"""
import array
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(CRATE))
BIN = os.path.join(CRATE, "target", "release", "af.exe" if os.name == "nt" else "af")
DEAD = "127.0.0.1:9"

T0 = 1790000000.0  # pinned fixture mtimes


def live_root():
    """The main checkout holding the live index (this file may sit in a worktree)."""
    env = os.environ.get("AF_LIVE_ROOT")
    if env:
        return env
    parts = os.path.normpath(REPO).split(os.sep)
    if ".claude" in parts and "worktrees" in parts:
        return os.sep.join(parts[:parts.index(".claude")])
    return REPO


def ollama_up():
    try:
        with urllib.request.urlopen("http://127.0.0.1:11434/api/tags", timeout=3) as r:
            return r.status == 200
    except Exception:
        return False


def w(root, rel, text, mtime=T0):
    path = os.path.join(root, *rel.split("/"))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    os.utime(path, (mtime, mtime))
    return path


def engine_files(root):
    for rel in ("system/af.py", "system/indexer.py"):
        dst = os.path.join(root, *rel.split("/"))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(os.path.join(REPO, *rel.split("/")), dst)
    if not os.path.isfile(os.path.join(root, "AGENTS.md")):
        w(root, "AGENTS.md", "# Fixture root\n")


# ------------------------------------------------------------------ snapshots

TABLES = {
    "files": "SELECT path, mtime, size, hash, status, version, is_head FROM files ORDER BY path",
    "chunks": "SELECT chunk_id, path, ord, breadcrumb, body FROM chunks ORDER BY chunk_id",
    "chunks_fts": "SELECT rowid, body, breadcrumb, path FROM chunks_fts ORDER BY rowid",
    "meta": "SELECT key, value FROM meta WHERE key != 'updated_at' ORDER BY key",
    "sqlite_sequence": "SELECT name, seq FROM sqlite_sequence ORDER BY name",
    "embeddings": "SELECT path, ord, dim, norm, vec FROM embeddings ORDER BY path, ord",
}


def snapshot(root):
    con = sqlite3.connect(os.path.join(root, "system", "index", "vault.db"))
    try:
        return {t: con.execute(q).fetchall() for t, q in TABLES.items()}
    finally:
        con.close()


def diff_snapshots(a, b):
    for t in TABLES:
        ra, rb = a[t], b[t]
        if len(ra) != len(rb):
            return f"table {t}: {len(ra)} rows py, {len(rb)} rs"
        for x, y in zip(ra, rb):
            if t != "embeddings":
                if x != y:
                    return f"table {t}: py={repr(x)[:160]} rs={repr(y)[:160]}"
                continue
            if x[:3] != y[:3]:
                return f"table embeddings key: py={x[:3]} rs={y[:3]}"
            va, vb = array.array("f", x[4]), array.array("f", y[4])
            if len(va) != len(vb) or abs(x[3] - y[3]) > 1e-6 or any(abs(p - q) > 1e-6 for p, q in zip(va, vb)):
                return f"table embeddings {x[:2]}: vectors differ beyond 1e-6"
    return None


# ------------------------------------------------------------------ running

UPDATE_SECS = re.compile(r"· \d+\.\ds")
NOW_T = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}")
TS_FIELD = re.compile(r'"ts": "[^"]*"')


def mask(text, roots, stamps):
    for r in roots:
        for form in (r.replace("\\", "\\\\"), r, r.replace("\\", "/")):
            text = text.replace(form, "<ROOT>")
    text = UPDATE_SECS.sub("· <SECS>", text)
    text = TS_FIELD.sub('"ts": "<TS>"', text)
    return NOW_T.sub(lambda m: m.group(0) if m.group(0) in stamps else "<NOW>", text)


def tagged_stderr(err):
    keep = ("af index:", "af search:", "af: ERROR:", "af: CRASH:")
    usage = re.compile(r"^af( \w+)*: error: ")  # argparse's usage errors (clap words them its own way)
    return [ln for ln in err.splitlines() if ln.startswith(keep) and not usage.match(ln)]


def run_step(root, step, rust):
    """-> (label, observation) or None for a mutation."""
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    for k in ("AF_ROOT", "AF_OLLAMA_URL", "HTTP_PROXY", "http_proxy", "NO_PROXY", "no_proxy"):
        env.pop(k, None)
    if callable(step):
        step(root)
        return None
    if isinstance(step, tuple) and step[0] == "db":
        return ("db snapshot", snapshot(root))
    engine_rust, argv = rust, step
    if isinstance(step, tuple):
        kind, argv = step
        if kind == "py":
            engine_rust = False
        elif kind == "dead":
            if rust:
                env["AF_OLLAMA_URL"] = f"http://{DEAD}/api/embed"
            else:
                env["HTTP_PROXY"] = env["http_proxy"] = f"http://{DEAD}"
    if engine_rust:
        env["AF_ROOT"] = root
        cmd = [BIN] + argv
    else:
        cmd = [sys.executable, os.path.join(root, "system", "af.py")] + argv
    p = subprocess.run(cmd, cwd=root, env=env, capture_output=True)
    out = p.stdout.decode("utf-8", "replace")
    err = p.stderr.decode("utf-8", "replace")
    return (f"`af {' '.join(argv)}`", (p.returncode, out, tagged_stderr(err)))


def first_line_diff(a, b):
    al, bl = a.splitlines(), b.splitlines()
    for i, (x, y) in enumerate(zip(al, bl)):
        if x != y:
            return f"line {i + 1}: py={x[:160]!r} rs={y[:160]!r}"
    if len(al) != len(bl):
        i = min(len(al), len(bl))
        return f"line {i + 1}: {'py' if len(al) > len(bl) else 'rs'} has extra {(al + bl)[i][:160]!r}"
    return "line endings or trailing bytes differ"


def run_case(name, build, keep=False, verbose=False):
    """build(base_root) -> steps, or a string reason to SKIP."""
    work = tempfile.mkdtemp(prefix=f"afparity-{name}-")
    py_root, rs_root = os.path.join(work, "py"), os.path.join(work, "rs")
    try:
        os.makedirs(py_root)
        steps = build(py_root)
        if isinstance(steps, str):
            return "SKIP", steps
        engine_files(py_root)
        shutil.copytree(py_root, rs_root, copy_function=shutil.copy2)
        roots = [py_root, rs_root]
        stamps = set()
        db = os.path.join(py_root, "system", "index", "vault.db")
        if os.path.isfile(db):
            con = sqlite3.connect(db)
            row = con.execute("SELECT value FROM meta WHERE key='updated_at'").fetchone()
            con.close()
            if row:
                stamps.add(row[0])
        for i, step in enumerate(steps):
            a = run_step(py_root, step, rust=False)
            b = run_step(rs_root, step, rust=True)
            if a is None:
                continue
            label = f"step {i + 1} {a[0]}"
            if a[0] == "db snapshot":
                d = diff_snapshots(a[1], b[1])
                if d:
                    return "DIFF", f"{label}: {d}"
                continue
            (ca, oa, ea), (cb, ob, eb) = a[1], b[1]
            if verbose:
                print(f"--- {label}: exit {ca}")
                print(mask(oa, roots, stamps).rstrip())
                for ln in ea:
                    print("    stderr:", mask(ln, roots, stamps))
            if ca != cb:
                return "DIFF", f"{label}: exit py={ca} rs={cb} (rs stderr: {eb[:2]})"
            ma, mb = mask(oa, roots, stamps), mask(ob, roots, stamps)
            if ma != mb:
                return "DIFF", f"{label} stdout {first_line_diff(ma, mb)}"
            if oa.count("\r") != ob.count("\r"):
                return "DIFF", f"{label} stdout line endings differ"
            ea = [mask(x, roots, stamps) for x in ea]
            eb = [mask(x, roots, stamps) for x in eb]
            if ea != eb:
                return "DIFF", f"{label} stderr py={ea[:2]} rs={eb[:2]}"
        logs = []
        for r in roots:
            p = os.path.join(r, "system", "logs", "af-runs.jsonl")
            logs.append(mask(open(p, encoding="utf-8").read(), roots, stamps) if os.path.isfile(p) else "")
        if logs[0] != logs[1]:
            return "DIFF", f"run log {first_line_diff(logs[0], logs[1])}"
        return "PASS", None
    finally:
        if keep:
            print(f"  kept {work}")
        else:
            shutil.rmtree(work, ignore_errors=True)


# ------------------------------------------------------------------ fixture vault

LONG = "\n\n".join(f"Paragraph {i}: " + ("quarterly roadmap detail " * 40) for i in range(30))


def fixture_vault(root):
    p = "workspace/projects/acme"
    w(root, f"{p}/project.md",
      "---\nname: acme\nslug: acme\nstatus: active\ndeliverables:\n  brief:\n    status: drafting\n"
      "    file: brief/brief-v2.md\n  memo:\n    file: \"memo/memo.md\"\n---\n\n# Acme launch\n\nThe launch plan for Acme widgets.\n")
    w(root, f"{p}/brief/brief-v1.md", "---\nstatus: superseded\n---\n# Brief v1\n\n## Audience\nEarly adopters of the widget launch.\n")
    w(root, f"{p}/brief/brief-v2.md", "---\nstatus: \"drafting\"\n---\n# Brief v2\n\n## Audience\nEarly adopters of the widget launch, sharpened.\n\n## Channels\nLinkedIn and a Substack essay.\n")
    w(root, f"{p}/memo/memo.md", "# Memo\n\nPricing memo: the widget costs 40 dollars. AI pricing note.\n\n```\n# not a heading\nprice = 40\n```\n")
    w(root, f"{p}/sources/INDEX.md", "# Sources\n\n- [interview](sources/interview-notes.md) customer interview\n- (sources/missing.md)\n")
    w(root, f"{p}/sources/interview-notes.md", "# Interview\n\nThe customer said the widget launch felt rushed. Pricing was fine.\n")
    w(root, f"{p}/knowledge/glossary.md", "---\nstatus: ready\n---\n# Glossary\n\nWidget: the product. Launch: the go-to-market moment.\n")
    w(root, f"{p}/_local/scratch-notes.txt", "Loose notes on the launch timing and the widget price.\n")
    w(root, f"{p}/roadmap.md", "# Roadmap\n\n" + LONG + "\n")
    w(root, f"{p}/node_modules/pkg/README.md", "# skipped widget launch\n")
    w(root, "workspace/projects/acme/_archive/old-plan.md", "# Old plan\n\nAn archived widget launch plan.\n")
    w(root, "workspace/projects/beta/project.md", "---\nname: beta\n---\n# Beta\n\nCafé résumé naïve — unicode widget notes. ΣΊΣΥΦΟΣ.\n")
    w(root, "workspace/life/career/plan.md", "# Career plan\n\n## Next year\nMove toward forward-deployed engineering; keep writing about the widget launch.\n")
    w(root, "workspace/life/voice/register.md", "# Voice\n\nskipped by the corpus walk\n")
    w(root, "workspace/life/_history/2025.md", "# 2025\n\nThe year of the first widget.\n")
    w(root, "library/assets/brand/colors.md", "# Colors\n\nWidget blue is #1d4ed8.\n")
    w(root, ".claude/plans/2026-10-01-plan.md", "# Plan\n\nPort the widget indexer to Rust.\n")
    w(root, "workspace/pipeline/applications/acme-fde/application.md",
      "---\ncompany: Acme\n---\n# Application\n\nfile: `workspace/projects/acme/memo/memo.md`\n")


SEARCHES = [
    ["search", "widget launch plan"],
    ["search", "pricing memo", "--json"],
    ["search", "who are the early adopters", "-n", "3"],
    ["search", "launch", "-n", "-1"],
    ["search", "AI"],
    ["search", "zzqqxx nothing matches here"],
    ["search", "widget", "-n", "0"],
    ["search", "café résumé", "--json"],
]


def mutate(root):
    p = "workspace/projects/acme"
    w(root, f"{p}/memo/memo.md", "# Memo\n\nPricing memo, revised: the widget now costs 45 dollars.\n", mtime=T0 + 100)
    path = os.path.join(root, *f"{p}/knowledge/glossary.md".split("/"))
    os.utime(path, (T0 + 50, T0 + 50))  # touch-only: same bytes
    os.remove(os.path.join(root, *f"{p}/_local/scratch-notes.txt".split("/")))
    w(root, f"{p}/brief/brief-v3.md", "# Brief v3\n\n## Audience\nEnterprise buyers of the widget.\n", mtime=T0 + 200)


def golden(root):
    w(root, "system/index/golden-set.yaml",
      "# fixture golden set\n- q: widget pricing memo\n  expect: workspace/projects/acme/memo/memo.md\n"
      "- q: career next year\n  expect: workspace/life/career/plan.md\n"
      "- q: enterprise buyers audience\n  expect: workspace/projects/acme/brief/brief-v3.md\n"
      "  expect: workspace/projects/acme/brief/brief-v2.md\n- q: something absent entirely\n  expect: nowhere.md\n")


def case_fixture(root):
    if not ollama_up():
        return "Ollama not serving on 127.0.0.1:11434"
    fixture_vault(root)
    return ([["index", "status"], ["index", "update"], ("db",), ["index", "status"]] + SEARCHES
            + [mutate, ["index", "update"], ("db",), ["index", "status"]] + SEARCHES[:3]
            + [["index", "eval"], golden, ["index", "eval"], ["index", "eval", "--k", "1"],
               ["index", "update", "--rebuild"], ("db",),
               # the Python reading what each side wrote (on rs: the binary's --rebuild)
               ("py", ["index", "status"]), ("py", ["search", "widget launch plan"]),
               ("py", ["index", "update"]), ("db",)])


def case_keyword_only(root):
    if not ollama_up():
        return "Ollama not serving on 127.0.0.1:11434"
    fixture_vault(root)
    return [("dead", ["index", "update"]), ("db",), ["index", "status"],
            ("dead", ["search", "widget launch plan"]), ("dead", ["search", "pricing", "--json"]),
            ["index", "update"], ("db",), ["index", "status"],
            mutate, ("dead", ["index", "update"]), ["index", "status"],
            ("dead", ["search", "enterprise buyers"]),
            ["search", "enterprise buyers"], ("db",), ["index", "status"]]


def case_usage(root):
    w(root, "workspace/notes/a.md", "# A\n\nalpha\n")
    return [["search"], ["index"], ["index", "bogus"], ["search", "x", "-n", "abc"], ["index", "eval", "--k", "x"],
            ["index", "update", "--reb"], ["search", "alpha", "--js"]]


# ------------------------------------------------------------------ live copy

def golden_queries(live):
    path = os.path.join(live, "system", "index", "golden-set.yaml")
    qs = []
    with open(path, encoding="utf-8-sig", errors="replace") as f:
        for line in f.read().splitlines():
            if line.startswith("- q:"):
                qs.append(line[len("- q:"):].strip())
    return qs


def copy_live(live, dst):
    """The corpus (every .md/.txt under the corpus roots, skipped folders included, since
    the tracker and source-registry scans read them) and the index, mtimes preserved."""
    for top in ("workspace", os.path.join("library", "assets"), os.path.join(".claude", "plans")):
        for dp, dns, fns in os.walk(os.path.join(live, top)):
            for fn in fns:
                if os.path.splitext(fn)[1].lower() in (".md", ".txt"):
                    src = os.path.join(dp, fn)
                    out = os.path.join(dst, os.path.relpath(src, live))
                    os.makedirs(os.path.dirname(out), exist_ok=True)
                    shutil.copy2(src, out)
    os.makedirs(os.path.join(dst, "system", "index"), exist_ok=True)
    for name in ("vault.db", "vault.db-wal", "golden-set.yaml"):
        src = os.path.join(live, "system", "index", name)
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(dst, "system", "index", name))


def case_live(root):
    live = live_root()
    if not os.path.isfile(os.path.join(live, "system", "index", "vault.db")):
        return f"no live index under {live}"
    if not ollama_up():
        return "Ollama not serving on 127.0.0.1:11434"
    copy_live(live, root)
    engine_files(root)
    # Absorb files edited since the live index last ran, so both sides start current.
    subprocess.run([sys.executable, os.path.join(root, "system", "af.py"), "index", "update"],
                   cwd=root, capture_output=True, check=True)
    steps = [["index", "status"]]
    queries = golden_queries(live) if os.path.isfile(os.path.join(live, "system", "index", "golden-set.yaml")) else []
    steps += [["search", q] for q in queries]
    steps += [["search", q, "--json", "-n", "12"] for q in queries[:5]]
    steps += [["index", "eval"], ["index", "eval", "--k", "3"], ["index", "update"], ("db",), ["index", "status"]]
    return steps


CASES = [
    ("index-usage", case_usage),
    ("index-fixture", case_fixture),
    ("index-keyword-only", case_keyword_only),
    ("index-live", case_live),
]


def extra_cases():
    """(name, runner(keep, verbose) -> (status, detail)) for parity.py."""
    return [(name, (lambda b, n: lambda keep, verbose: run_case(n, b, keep, verbose))(build, name))
            for name, build in CASES]
