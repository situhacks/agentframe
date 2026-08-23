#!/usr/bin/env python3
"""Cross-project retrieval substrate — the index behind `af index` / `af search`.

Constitutional position: DERIVED CACHE. Files stay the memory; this database is
rebuildable at any moment and is never cited as truth. Search results are
pointers — consumers open the cited files before relying on them.

Corpus (inclusion, not enforcement — whatever is indexed is the whole world
`af search` sees): workspace/, library/, .claude/plans/, and system/ machine
docs. Skipped everywhere: generated harness projections, vendor-source and
runtime subtrees, archives' noise stays ranked down rather than excluded.

Ranking v2 (hybrid): FTS5 BM25 over trigram tokens fused with local
nomic-embed-text cosine similarity through reciprocal-rank fusion — the
lexical half catches exact identifiers and filenames, the semantic half
catches vocabulary mismatch — then domain boosts. Precedence is
most-specific-path-wins. Tokens under 3 characters bypass trigram's floor
via a direct LIKE scan. When Ollama is unreachable the index degrades to
keyword-only and says so.

Embeddings run locally through Ollama's /api/embed (privacy: corpus never
leaves the machine). Stdlib only.

Cold start (fresh machine, no Ollama yet): install Ollama from ollama.com,
`ollama pull <model>` for EMBED_MODEL below, then `af index update --rebuild`.
Without Ollama the index still works — keyword-only — and every command says
so. The model is a swap-in constant: edit EMBED_MODEL, rebuild, compare via
`af index eval`. Dimension changes are detected and refused rather than
silently mixed. Measured model comparisons live in system/index/EVALS.md;
when picking a new model, benchmark on this instance's golden set before
adopting — leaderboard rank does not predict corpus fit.
"""

import array
import datetime
import hashlib
import json
import math
import os
import re
import sqlite3
import sys
import time
import urllib.request

INDEX_DIR_NAME = os.path.join("system", "index")
DB_NAME = "vault.db"
GOLDEN_NAME = "golden-set.yaml"

CORPUS_ROOTS = ["workspace", "library", os.path.join(".claude", "plans"), "system"]
SKIP_DIRS_ANYWHERE = {
    "_archive", "_scratch", "__pycache__", "node_modules", "snapshots",
    "runs", "cache", "local", "dist", "out", "source", "references",
}
# Extra skips applied only under system/ (machine internals; docs still indexed)
SYSTEM_SKIP_DIRS = {"tools", "browser", "server", "daemon", "research", "logs", "audit", "index"}
EXTENSIONS = {".md", ".txt"}

MAX_CHUNK_CHARS = 9000
POOL_SIZE = 60  # candidates fetched per retriever before fusion

# Local embedding backend (Ollama). Swap the model here, then run
# `af index update --rebuild` — dimension changes are detected and refused.
OLLAMA_URL = "http://127.0.0.1:11434/api/embed"
EMBED_MODEL = "qwen3-embedding:0.6b"  # challenger vs nomic-embed-text; see EVALS.md comparison
EMBED_BATCH = 32
EMBED_MAX_CHARS = 6000  # breadcrumb + body fed to the model per chunk
RRF_K = 60  # reciprocal-rank-fusion damping constant
LEXICAL_FUSION_WEIGHT = 1.0   # retriever weights in the fusion sum;
SEMANTIC_FUSION_WEIGHT = 1.0  # swept 0.5–1.4 on the golden set: recall flat
                              # below 1.0, degrades above — kept equal (n=29,
                              # do not fine-tune further without more queries)

FM_RE = re.compile(r"\A---\s*\n(.*?)\n---\s*\n?", re.DOTALL)
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")
FENCE_RE = re.compile(r"^\s*(```|~~~)")
VERSION_RE = re.compile(r"-v(\d+)\.[a-z0-9]+$", re.IGNORECASE)


def die(msg):
    print(f"af index: ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------- corpus walk

def iter_corpus(root):
    """Yield repo-relative forward-slash paths inside the corpus."""
    for top in CORPUS_ROOTS:
        base = os.path.join(root, top)
        if not os.path.isdir(base):
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            rel_dir = os.path.relpath(dirpath, root).replace("\\", "/")
            parts = rel_dir.split("/")
            pruned = []
            for d in dirnames:
                if d in SKIP_DIRS_ANYWHERE:
                    continue
                if top == "system" and d in SYSTEM_SKIP_DIRS:
                    continue
                # Voice corpus: routed context, not searched content — it is a
                # curated snapshot whose sources live in projects (operator
                # decision 2026-08-23). Route loads it; the index skips it.
                if top == "library" and d == "voice" and rel_dir.startswith("library/context"):
                    continue
                pruned.append(d)
            dirnames[:] = pruned
            for name in sorted(filenames):
                ext = os.path.splitext(name)[1].lower()
                if ext not in EXTENSIONS:
                    continue
                rel = os.path.join(rel_dir, name).replace("\\", "/")
                yield rel


# ------------------------------------------------------------- chunk building

def split_frontmatter(text):
    """Return (flat frontmatter dict or {}, body-without-frontmatter)."""
    m = FM_RE.match(text)
    if not m:
        return {}, text
    fm = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            key, val = line.split(":", 1)
            fm[key.strip()] = val.strip().strip('"').strip("'")
    return fm, text[m.end():]


def chunk_markdown(body):
    """Fence-aware heading split. Returns [(breadcrumb_titles_tuple, text)]."""
    sections, stack, cur = [], [], []

    def flush():
        text = "\n".join(cur).strip()
        if text:
            sections.append((tuple(t for _, t in stack), text))
        cur.clear()

    fenced, fence_mark = False, ""
    for line in body.splitlines():
        fm = FENCE_RE.match(line)
        if fm:
            mark = fm.group(1)[:3]
            if not fenced:
                fenced, fence_mark = True, mark
            elif mark == fence_mark:
                fenced, fence_mark = False, ""
            cur.append(line)
            continue
        hm = HEADING_RE.match(line)
        if not fenced and hm:
            flush()
            level, title = len(hm.group(1)), hm.group(2).strip()
            while stack and stack[-1][0] >= level:
                stack.pop()
            stack.append((level, title))
            continue
        cur.append(line)
    flush()

    if not sections:
        stripped = body.strip()
        return [((), stripped)] if stripped else []

    out = []
    for crumbs, text in sections:
        if len(text) <= MAX_CHUNK_CHARS:
            out.append((crumbs, text))
            continue
        parts, buf = [], ""
        for para in text.split("\n\n"):
            if buf and len(buf) + len(para) + 2 > MAX_CHUNK_CHARS:
                parts.append(buf)
                buf = para
            else:
                buf = f"{buf}\n\n{para}" if buf else para
        if buf:
            parts.append(buf)
        for i, part in enumerate(parts):
            crumb = crumbs + (f"(part {i + 1})",) if len(parts) > 1 else crumbs
            out.append((crumb, part))
    return out


def file_signals(rel_paths):
    """Heads: within a folder, the highest `-vN` file; every unversioned file
    is its own head."""
    versions = {}
    versioned = set()
    for rel in rel_paths:
        m = VERSION_RE.search(rel)
        if m:
            folder = os.path.dirname(rel).replace("\\", "/")
            versions.setdefault(folder, []).append((int(m.group(1)), rel))
            versioned.add(rel)
    heads = {rel for lst in versions.values() for _, rel in lst if _is_max(lst, rel)}
    heads |= set(rel_paths) - versioned
    return versions, heads


def _is_max(lst, rel):
    top = max(v for v, _ in lst)
    return dict((r, v) for v, r in lst)[rel] == top


# ------------------------------------------------------------------ database

def db_path(root):
    return os.path.join(root, INDEX_DIR_NAME, DB_NAME)


SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files(
  path TEXT PRIMARY KEY, mtime REAL NOT NULL, size INTEGER NOT NULL,
  hash TEXT NOT NULL, status TEXT, version INTEGER, is_head INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS chunks(
  chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL, ord INTEGER NOT NULL,
  breadcrumb TEXT NOT NULL, body TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS chunks_path ON chunks(path);
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  body, breadcrumb, path UNINDEXED, tokenize='trigram');
CREATE TABLE IF NOT EXISTS embeddings(
  path TEXT NOT NULL, ord INTEGER NOT NULL,
  dim INTEGER NOT NULL, norm REAL NOT NULL,
  vec BLOB NOT NULL, PRIMARY KEY(path, ord));
"""


def connect(root):
    os.makedirs(os.path.join(root, INDEX_DIR_NAME), exist_ok=True)
    con = sqlite3.connect(db_path(root))
    con.execute("PRAGMA journal_mode=WAL")
    con.executescript(SCHEMA_SQL)
    return con


def read_text(path):
    with open(path, "r", encoding="utf-8-sig", errors="replace") as fh:
        return fh.read()


def drop_file_rows(con, rel):
    con.execute("DELETE FROM chunks_fts WHERE path = ?", (rel,))
    con.execute("DELETE FROM chunks WHERE path = ?", (rel,))
    con.execute("DELETE FROM embeddings WHERE path = ?", (rel,))
    con.execute("DELETE FROM files WHERE path = ?", (rel,))


def insert_file(con, root, rel, mtime, size, file_hash, heads):
    """Index one file's chunks; returns [(path, ord, embed_text)] for the
    caller to queue for embedding."""
    fm, body = split_frontmatter(read_text(os.path.join(root, rel)))
    version = None
    vm = VERSION_RE.search(rel)
    if vm:
        version = int(vm.group(1))
    drop_file_rows(con, rel)
    stem = os.path.splitext(os.path.basename(rel))[0]
    items = []
    for i, (crumbs, text) in enumerate(chunk_markdown(body)):
        crumb = " › ".join((stem,) + crumbs)
        con.execute(
            "INSERT INTO chunks(path, ord, breadcrumb, body) VALUES(?,?,?,?)",
            (rel, i, crumb, text),
        )
        con.execute(
            "INSERT INTO chunks_fts(body, breadcrumb, path) VALUES(?,?,?)",
            (text, crumb, rel),
        )
        # breadcrumb rides along so vectors carry section context
        items.append((rel, i, f"{crumb}\n{text}"[:EMBED_MAX_CHARS]))
    con.execute(
        "INSERT INTO files(path, mtime, size, hash, status, version, is_head)"
        " VALUES(?,?,?,?,?,?,?)",
        (rel, mtime, size, file_hash, fm.get("status"), version,
         1 if rel in heads else 0),
    )
    return items


def cmd_update(root, rebuild=False, quiet=False):
    started = time.time()
    con = connect(root)
    if rebuild:
        for table in ("chunks_fts", "chunks", "files", "embeddings"):
            con.execute(f"DROP TABLE IF EXISTS {table}")
        con.executescript(SCHEMA_SQL)

    disk = {}
    n_md = n_txt = 0
    for rel in iter_corpus(root):
        full = os.path.join(root, rel)
        st = os.stat(full)
        disk[rel] = (st.st_mtime, st.st_size)
        if rel.lower().endswith(".md"):
            n_md += 1
        else:
            n_txt += 1

    versions, heads = file_signals(disk.keys())
    known = {row[0]: (row[1], row[2], row[3]) for row in
             con.execute("SELECT path, mtime, size, hash FROM files")}

    added = updated = unchanged = removed = rehashed_same = 0
    pending_embeds = []
    embed_state = {"failed": False, "embedded": 0}

    def flush_embeddings():
        """Embed one batch; on backend failure degrade to keyword-only and
        say so once — the keyword index still lands."""
        if not pending_embeds or embed_state["failed"]:
            pending_embeds.clear()
            return
        batch = pending_embeds[:EMBED_BATCH]
        try:
            vecs = embed_texts([text for _, _, text in batch])
            dim = len(vecs[0])
            row = con.execute("SELECT dim FROM embeddings LIMIT 1").fetchone()
            if row and row[0] != dim:
                raise RuntimeError(
                    f"embedding dimension changed ({row[0]} -> {dim});"
                    " run `af index update --rebuild` after switching models")
            for (pth, ordn, _), vec in zip(batch, vecs):
                norm = math.sqrt(math.fsum(x * x for x in vec))
                packed = array.array("f", vec).tobytes()
                con.execute(
                    "INSERT OR REPLACE INTO embeddings(path, ord, dim, norm, vec)"
                    " VALUES(?,?,?,?,?)", (pth, ordn, dim, norm, packed))
            del pending_embeds[:len(batch)]
            embed_state["embedded"] += len(batch)
        except Exception as exc:
            embed_state["failed"] = True
            pending_embeds.clear()
            print(
                f"af index: WARNING — embeddings unavailable"
                f" ({exc.__class__.__name__}: {exc}); indexed keyword-only."
                " Start Ollama and re-run `af index update` to fill vectors.",
                file=sys.stderr)

    def queue(items):
        pending_embeds.extend(items)
        while len(pending_embeds) >= EMBED_BATCH:
            flush_embeddings()

    for rel in sorted(set(disk) - set(known)):
        mtime, size = disk[rel]
        queue(insert_file(con, root, rel, mtime, size, _hash_of(root, rel), heads))
        added += 1
    for rel in sorted(set(known) & set(disk)):
        mtime, size = disk[rel]
        old_mtime, old_size, old_hash = known[rel]
        if old_mtime == mtime and old_size == size:
            unchanged += 1
            continue
        new_hash = _hash_of(root, rel)
        if new_hash == old_hash:
            con.execute("UPDATE files SET mtime=? WHERE path=?", (mtime, rel))
            rehashed_same += 1
            continue
        queue(insert_file(con, root, rel, mtime, size, new_hash, heads))
        updated += 1
    for rel in sorted(set(known) - set(disk)):
        drop_file_rows(con, rel)
        removed += 1
    flush_embeddings()

    total_chunks = con.execute("SELECT COUNT(*) FROM chunks").fetchone()[0]
    total_files = con.execute("SELECT COUNT(*) FROM files").fetchone()[0]
    embedded_total = con.execute("SELECT COUNT(*) FROM embeddings").fetchone()[0]
    con.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('updated_at', ?)",
                (datetime.datetime.now().isoformat(timespec="seconds"),))
    con.execute("INSERT OR REPLACE INTO meta(key, value) VALUES('embed_model', ?)",
                (EMBED_MODEL,))
    con.commit()
    con.close()
    summary = {
        "added": added, "updated": updated, "removed": removed,
        "files": total_files, "chunks": total_chunks,
        "embedded": embedded_total,
    }
    if not quiet:
        print(
            f"af index: scanned {len(disk)} files ({n_md} md, {n_txt} txt) · "
            f"+{added} new · ~{updated} updated · ={unchanged} unchanged "
            f"({rehashed_same} touch-only) · -{removed} gone · "
            f"{total_files} tracked / {total_chunks} chunks · "
            f"E{embedded_total}/{total_chunks} embedded · "
            f"{time.time() - started:.1f}s"
        )
    return summary


# Lazy incremental cadence (plan W2 decision record): every answering surface
# diffs the corpus manifest and re-indexes only what changed BEFORE answering,
# so there is no scheduler and no stale answers. Throttled per process.
_REFRESH_SECONDS = 60.0
_last_refresh = {"at": 0.0}


def refresh_before_answer(root):
    now = time.time()
    if now - _last_refresh["at"] < _REFRESH_SECONDS:
        return None
    _last_refresh["at"] = now
    return cmd_update(root, quiet=True)


def _hash_of(root, rel):
    with open(os.path.join(root, rel), "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


def cmd_status(root):
    con = connect(root)
    files_n = con.execute("SELECT COUNT(*) FROM files").fetchone()[0]
    chunks_n = con.execute("SELECT COUNT(*) FROM chunks").fetchone()[0]
    row = con.execute("SELECT value FROM meta WHERE key='updated_at'").fetchone()
    updated_at = row[0] if row else "never"
    stale = 0
    for rel, in con.execute("SELECT path FROM files"):
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            stale += 1
            continue
        st = os.stat(full)
        k = con.execute("SELECT mtime, size FROM files WHERE path=?", (rel,)).fetchone()
        if abs(k[0] - st.st_mtime) > 1e-6 or k[1] != st.st_size:
            stale += 1
    superseded = con.execute(
        "SELECT COUNT(*) FROM files WHERE is_head = 0").fetchone()[0]
    embedded = con.execute("SELECT COUNT(*) FROM embeddings").fetchone()[0]
    model_row = con.execute("SELECT value FROM meta WHERE key='embed_model'").fetchone()
    con.close()
    print(f"af index: db {os.path.relpath(db_path(root), root)}")
    print(f"  last update : {updated_at}")
    print(f"  tracked     : {files_n} files / {chunks_n} chunks")
    print(f"  embeddings  : {embedded}/{chunks_n} chunks via"
          f" {model_row[0] if model_row else 'n/a'}"
          f"{'' if embedded else ' — keyword-only until you run af index update'}")
    print(f"  stale       : {stale} (run `af index update`)")
    print(f"  superseded  : {superseded} prior versions ranked down")


# ------------------------------------------------------------------- ranking

def path_boost(rel):
    """Most-specific-path-wins class boosts -> (multiplier, tag)."""
    p = rel.replace("\\", "/").lower()
    if "_archive/" in p:
        return 0.45, "archive"
    if "/knowledge/" in p or p.startswith("knowledge/"):
        return 1.5, "knowledge"
    if "/sources/" in p or "/_local/" in p:
        return 1.15, "primary-material"
    if p.startswith(".claude/plans/"):
        return 1.05, "plan"
    if p.startswith("agents"):
        return 1.15, "router"
    return 1.0, ""


def tracker_pointers(root):
    """Repo-relative deliverable paths named by project/application trackers."""
    pointers = set()
    patterns = [
        os.path.join("workspace", "projects", "*", "project.md"),
        os.path.join("workspace", "pipeline", "applications", "*", "application.md"),
    ]
    for pattern in patterns:
        for full in __import__("glob").glob(os.path.join(root, pattern)):
            proj_dir = os.path.dirname(full)
            try:
                text = read_text(full)
            except OSError:
                continue
            for m in re.finditer(r"file:\s*[\"'`]?([^\s\"'`]+\.(?:md|txt))", text):
                cand = m.group(1).replace("\\", "/")
                for resolved in (cand, os.path.relpath(
                        os.path.normpath(os.path.join(proj_dir, cand)), root)):
                    resolved = resolved.replace("\\", "/")
                    if os.path.isfile(os.path.join(root, resolved)):
                        pointers.add(resolved)
                        break
    return pointers


def registered_sources(root):
    """Paths listed in any project's sources/INDEX.md."""
    reg = set()
    for dirpath, dirnames, filenames in os.walk(os.path.join(root, "workspace")):
        if os.path.basename(dirpath) != "sources":
            continue
        idx = os.path.join(dirpath, "INDEX.md")
        if not os.path.isfile(idx):
            continue
        proj_dir = os.path.dirname(dirpath)
        for m in re.finditer(r"[^\s()]*sources/[^\s()]*\.(?:md|txt)", read_text(idx)):
            cand = m.group(0).replace("\\", "/")
            for resolved in (cand, os.path.relpath(
                    os.path.normpath(os.path.join(proj_dir, cand)), root)):
                resolved = resolved.replace("\\", "/")
                if os.path.isfile(os.path.join(root, resolved)):
                    reg.add(resolved)
                    break
    return reg


def _tokenize(query):
    return [t for t in re.findall(r"[a-z0-9_']+", query.lower()) if t]


def _snippet(body, tokens, width=90):
    low = body.lower()
    pos = min((low.find(t) for t in tokens
               if len(t) >= 3 and low.find(t) >= 0), default=-1)
    if pos < 0:
        pos = 0
    start = max(0, pos - width // 3)
    frag = body[start:start + width * 2].strip()
    return re.sub(r"\s+", " ", frag)


def search(root, query, limit=8):
    """Hybrid ranked search: [{path, breadcrumb, snippet, score, why}].

    Two retrievers — lexical (BM25 + short-token LIKE) and semantic (cosine
    over local embeddings) — each produce a ranked file list; reciprocal-rank
    fusion merges them; domain boosts then multiply the fused score.
    """
    con = connect(root)
    tokens = _tokenize(query)
    good = ['"%s"' % t.replace('"', '""') for t in tokens if len(t) >= 3]
    shorts = [t for t in tokens if 0 < len(t) < 3]

    # --- lexical retriever ---
    best = {}  # path -> (bm25_rank_score, breadcrumb, body); smaller = better
    if good:
        match = " OR ".join(good)
        for path, crumb, body, r in con.execute(
                "SELECT path, breadcrumb, body, bm25(chunks_fts) FROM chunks_fts"
                " WHERE chunks_fts MATCH ? ORDER BY bm25(chunks_fts) LIMIT ?",
                (match, POOL_SIZE)):
            if path not in best or r < best[path][0]:
                best[path] = (r, crumb, body)
    lexical_order = sorted(best, key=lambda p: best[p][0])
    for short in shorts:
        for path, crumb, body in con.execute(
                "SELECT path, breadcrumb, body FROM chunks WHERE instr(lower(body), ?) > 0"
                " LIMIT 25", (short,)):
            if path not in best:
                best[path] = (0.0, crumb, body)
                lexical_order.append(path)

    # --- semantic retriever (degrades to keyword-only without Ollama) ---
    semantic_order, semantic_set = [], set()
    emb_count = con.execute("SELECT COUNT(*) FROM embeddings").fetchone()[0]
    if emb_count and any(len(t) >= 3 for t in tokens):
        try:
            qtext = query[:EMBED_MAX_CHARS]
            if "qwen3" in EMBED_MODEL:
                # Qwen3-embedding's documented asymmetric usage: instruct queries
                qtext = ("Instruct: Retrieve the user's own notes and documents"
                         f" relevant to their request\nQuery: {qtext}")
            qvec = embed_texts([qtext])[0]
            qarr = array.array("f", qvec)
            qnorm = math.sqrt(math.fsum(x * x for x in qarr))
            scored = []
            for path, blob, norm in con.execute(
                    "SELECT path, vec, norm FROM embeddings"):
                carr = array.array("f")
                carr.frombytes(blob)
                denom = qnorm * norm
                cos = math.sumprod(qarr, carr) / denom if denom else 0.0
                scored.append((cos, path))
            scored.sort(reverse=True)
            top = scored[:POOL_SIZE]
            semantic_order = [p for _, p in top]
            semantic_set = set(semantic_order)
        except Exception as exc:
            print(f"af search: note — semantic scoring unavailable"
                  f" ({exc.__class__.__name__}); keyword-only this run",
                  file=sys.stderr)

    # --- fuse, then boost ---
    fused = {}
    for rank, path in enumerate(lexical_order, 1):
        fused[path] = fused.get(path, 0.0) + LEXICAL_FUSION_WEIGHT / (RRF_K + rank)
    for rank, path in enumerate(semantic_order, 1):
        fused[path] = fused.get(path, 0.0) + SEMANTIC_FUSION_WEIGHT / (RRF_K + rank)

    trackers = tracker_pointers(root)
    registered = registered_sources(root)

    results = []
    for path, base in fused.items():
        info = best.get(path)
        if info is None:  # semantic-only hit: borrow its first chunk
            rowc = con.execute(
                "SELECT breadcrumb, body FROM chunks WHERE path=? ORDER BY ord LIMIT 1",
                (path,)).fetchone()
            info = (None, rowc[0], rowc[1]) if rowc else (None, "", "")
        _, crumb, body = info
        row = con.execute(
            "SELECT is_head FROM files WHERE path=?", (path,)).fetchone()
        mult, why = 1.0, []
        cls, cls_tag = path_boost(path)
        if cls != 1.0:
            mult *= cls
            why.append(cls_tag)
        if row and not row[0]:
            mult *= 0.55
            why.append("superseded-version")
        if path in trackers:
            mult *= 1.9
            why.append("tracker-head")
        if path in registered:
            mult *= 1.25
            why.append("registered-source")
        if path in semantic_set:
            why.append("semantic")
        if path in best:
            why.append("bm25")
        results.append({
            "path": path,
            "breadcrumb": crumb,
            "snippet": _snippet(body, tokens),
            "score": round(base * mult, 6),
            "why": why or ["fused"],
        })
    con.close()
    results.sort(key=lambda d: d["score"], reverse=True)
    return results[:limit]


def cmd_search_cli(root, query, limit=8, as_json=False):
    refreshed = refresh_before_answer(root)
    if refreshed and (refreshed["added"] or refreshed["updated"] or refreshed["removed"]):
        print(f"af index: refreshed first "
              f"(+{refreshed['added']} new · ~{refreshed['updated']} updated · "
              f"-{refreshed['removed']} gone)\n")
    results = search(root, query, limit=limit)
    if as_json:
        print(json.dumps(results, ensure_ascii=False, indent=2))
        return
    if not results:
        print(f"af search: no hits for {query!r} — try fewer or different words,"
              " or `af index update` if files changed recently")
        return
    print(f"af search: {len(results)} hits for {query!r}"
          "  (open cited files before relying on snippets)\n")
    for i, hit in enumerate(results, 1):
        tags = ",".join(hit["why"])
        print(f"{i}. [{hit['score']:7.3f}] {hit['path']}\n   {hit['breadcrumb']}")
        print(f"   …{hit['snippet']}…")
        print(f"   why: {tags}\n")


# --------------------------------------------------------------- eval (W2)

def load_golden(root):
    path = os.path.join(root, INDEX_DIR_NAME, GOLDEN_NAME)
    if not os.path.isfile(path):
        die(f"no golden set at {os.path.relpath(path, root)} — harvest one first"
            " (question lines '- q:' with 'expect:' path lines beneath each)")
    records, cur = [], None
    for raw in read_text(path).splitlines():
        line = raw.rstrip()
        if line.lstrip().startswith("#") or not line.strip():
            continue
        if line.startswith("- q:"):
            cur = {"q": line[len("- q:"):].strip(), "expect": []}
            records.append(cur)
        elif line.strip().startswith("expect:") and cur is not None:
            cur["expect"].append(line.strip()[len("expect:"):].strip())
    if not records:
        die("golden set parsed empty — check its '- q:' / 'expect:' shape")
    return records, path


def cmd_eval(root, k=5):
    refresh_before_answer(root)
    records, golden_path = load_golden(root)
    hits, mrr_total, rows = 0, 0.0, []
    for i, rec in enumerate(records, 1):
        ranked = [h["path"] for h in search(root, rec["q"], limit=k)]
        seen, uniq = set(), []
        for pth in ranked:
            if pth not in seen:
                seen.add(pth)
                uniq.append(pth)
        rank = next((uniq.index(exp) + 1 for exp in rec["expect"] if exp in uniq), None)
        ok = rank is not None
        hits += int(ok)
        mrr_total += (1.0 / rank) if ok else 0.0
        shown = uniq[0] if uniq else "(none)"
        rows.append((i, rec["q"], ok, rank, rec["expect"][0], shown))
    n = len(records)
    print(f"af index eval: {n} queries · recall@{k} = {hits}/{n} = {hits / n:.0%} · "
          f"MRR = {mrr_total / n:.3f}")
    print("(record numbers to system/index/EVALS.md)\n")
    for i, q, ok, rank, expected, shown in rows:
        mark = "PASS" if ok else "MISS"
        got = f"rank {rank}" if ok else f"top: {shown}"
        print(f"  [{mark}] q{i:>02} {q[:52]!r}")
        if not ok:
            print(f"         wanted: {expected}")
    misses = n - hits
    print(f"\nerror analysis input: {misses} miss(es)")


# -------------------------------------------------------- embedding backend

def embed_texts(texts, model=EMBED_MODEL):
    """Local-only embeddings via Ollama's /api/embed. Raises on failure so
    callers can degrade to keyword-only."""
    payload = json.dumps({"model": model, "input": list(texts)}).encode("utf-8")
    req = urllib.request.Request(
        OLLAMA_URL, data=payload, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    embs = data.get("embeddings") or []
    if len(embs) != len(texts):
        raise RuntimeError(
            f"embedding backend returned {len(embs)} vectors for {len(texts)} inputs")
    return embs


if __name__ == "__main__":
    print(__doc__)
