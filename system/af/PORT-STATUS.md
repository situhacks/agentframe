# af Rust port: status (work file, deleted at cutover)

Stage R1 of the port planned in `.claude/plans/2026-10-23-life-os-PLAN.md` section 7. Baseline: `system/af.py` at `a6fdb3a8` (unchanged through `decd5fda`).

## Verbs

| Verb | Status |
|---|---|
| `new-project` | ported and parity PASS |
| `draft` | ported and parity PASS |
| `adopt` | ported and parity PASS |
| `version` | ported and parity PASS |
| `ready` (incl. marketing post-FINAL assembly, careers gates) | ported and parity PASS |
| `publish` | ported and parity PASS |
| `doctor` (all checks, incl. media shelf, pipeline, studio, buttons (14d)) | ported and parity PASS |
| run log (`system/logs/af-runs.jsonl`) | ported and parity PASS |
| `index`, `search` | not started (R2+); binary prints "not ported yet" and exits 1 |
| `voice` | not started (R2+) |
| `pipe` | not started (R2+); doctor's pipeline checks are ported |
| `studio` | not started (R2+); doctor's studio checks are ported |
| `system/hooks/*` | not started (R2+) |

## Crate layout

- `Cargo.toml`: package `af`, library `afcore` (`src/lib.rs`), binary `af` (`src/main.rs`). Deps: clap, regex, chrono, walkdir, serde_json, yaml-rust2. `Cargo.lock` belongs in git.
- `src/cli.rs`: clap parser (argparse abbreviations, last-wins options, draft's required group), dispatch, root discovery, the run-log wrapper that mirrors af.py `main()`.
- `src/error.rs`: `die()` refusals, usage errors, crashes, the recorded last error.
- `src/out.rs`: stdout/stderr with Python's Windows `\n` to `\r\n` translation.
- `src/pyos.rs`, `src/pyglob.rs`: ntpath/os.path, file I/O (`utf-8-sig` read, `\n` write), `repr(str)`, glob/fnmatch as CPython 3.12.
- `src/fm.rs`, `src/core.rs`: frontmatter-as-text helpers, `re.sub` templates, `Ctx` and shared row/version logic.
- `src/cmds.rs`: the six transition verbs. `src/packs/`: compiled-in marketing and careers rules.
- `src/runlog.rs`: run-log write/read and the buttons note. `src/doctor.rs`: every doctor check.
- `src/media_shelf.rs`: port of `media_intake.notes()`. `src/pyyaml.rs`: PyYAML `safe_load` semantics over yaml-rust2.
- `tests/*.rs`: ported pytest cases (`tests/common/mod.rs` rebuilds the conftest fixtures) plus `media_shelf.rs`. `tests/parity.py`: Python-vs-binary harness (`-k`, `--keep`, `-v`).

## Verification (last run)

- `cargo test`: 155 passed (3 unit, 152 integration). Not ported: the two `LazyIndexer` cases in `test_af_buttons.py` (index verb) and the `pipe stage` half of one `test_af_pipe.py` case.
- `python system/af/tests/parity.py`: 53/53 PASS (19 verb and doctor cases, 34 media-shelf cases).
- `python -m pytest system/tests -q -p no:cacheprovider`: 390 passed, 1 skipped.
- `cargo build --release`: no warnings; `target/release/af.exe` 3,331,072 bytes.
- `af doctor` on this worktree: 0.150 s median vs 0.361 s for `python system/af.py doctor` (7 runs each).

## Known divergences

- Help and usage text come from clap, not argparse. Exit codes and the run-log `refusal: usage` match.
- A crash prints `af: CRASH: Type: msg` (or Rust's panic text), not a traceback. Exit 1 and the run-log row (exit 2, `crash`) match.
- Not-ported verbs exit 1 with a pointer to `python system/af.py` and write no run-log row.
- Root: `AF_ROOT`, else the nearest ancestor of the binary, then of the cwd, holding `AGENTS.md` and `system/`.
- Pack rules are compiled in and active only while the pack ships `rules.py`; a pack shipping a `rules.py` with no Rust counterpart is refused.
- Media-shelf YAML: values, types, constructor errors, tab and duplicate-anchor errors match PyYAML. Still different: scanner and parser error text for malformed YAML (yaml-rust2 wording), NEL/LS/PS as line breaks outside quoted scalars, `!!set` print order (hash order in Python), multi-line quoted keys in flow mappings (yaml-rust2 accepts).
- Messages follow CPython 3.12 (`KeyError: slice(None, 12, None)`, `datetime.date` type names). A Python upgrade can move them.

## Cutover notes

- Add `system/af/target/` to `.gitignore` (this stage did not touch it).
- Replace the `rules.py` presence switch in `src/packs/mod.rs` when the packs drop their Python modules.
- Decide root discovery (keep `AF_ROOT`, or install next to the repo) before wiring `.claude/settings.json` and PATH.
- Port order suggestion for R2: `pipe` and `studio` (doctor already covers their state), then `index`/`search`, `voice`, hooks.

## Stage R3: `index`, `search`

Baseline: `system/indexer.py` and af.py's `cmd_index`/`cmd_search` at `b030c418`. This section supersedes the `index`, `search` row in the R1 table above.

| Verb | Status |
|---|---|
| `index update [--rebuild]` | ported and parity PASS (tables row for row; embeddings bit-exact on the live copy) |
| `index status` | ported and parity PASS |
| `index eval [--k N]` | ported and parity PASS (live golden set: 23 queries, recall@5 21/23, MRR 0.591 on both) |
| `search "<q>" [-n N] [--json]` | ported and parity PASS (all 23 golden queries plus 5 `--json -n 12`, ranked output identical) |

### Layout

- `src/index/`: `chunk.rs` (frontmatter, heading chunks, breadcrumbs, version heads), `store.rs` (schema, corpus walk, update, status), `rank.rs` (hybrid search, boosts, snippets), `eval.rs`, `embed.rs` (Ollama over `ureq`, a small JSON parser), `pynum.rs` (CPython `math.fsum`, 3.12 `math.sumprod`, `round`, float `repr`, `st_mtime`), `mod.rs` (CLI glue, JSON output).
- Deps added: `rusqlite =0.35.0` with `bundled` (SQLite 3.49.1, the same version Python 3.12.10's `sqlite3` carries, so FTS5 BM25 ties order the same), `ureq 2` without TLS, `sha2`.
- Same schema SQL, same `meta` keys (`updated_at`, `embed_model`), same little-endian f32 vector blobs: the binary opens the Python's `vault.db` as is, and the Python reads a binary `--rebuild` (parity case `index-fixture` runs the Python on the binary's database).

### Verification (last run)

- `cargo test` (debug and release): 178 passed (18 unit, 160 integration); new: 15 unit in `src/index/`, 8 in `tests/index_search.rs` (keyword-only through the binary against a dead port, fusion through the library with a fake embedder).
- `python system/af/tests/parity.py`: 57/57 PASS. New cases live in `tests/parity_index.py` (hooked from `parity.py`): `index-usage`, `index-fixture` (Ollama live; update, mutate, update, eval, `--rebuild`, Python reading the binary's database), `index-keyword-only` (Ollama unreachable on both sides; warnings, partial vectors, backfill), `index-live` (a copy of the live corpus and `vault.db`, read-only against the main checkout). Ollama-dependent cases print SKIP when it is not serving.
- `python -m pytest system/tests -q -p no:cacheprovider`: 390 passed, 1 skipped (untouched).
- `cargo build --release`: no warnings; `af.exe` 5,762,048 bytes (R1: 3,331,072; the bundled SQLite is most of the growth).
- Wall time on the live copy, median of 7: `search` 2.126 s Python vs 0.746 s Rust; `search --json -n 12` 1.619 vs 0.724; `index update` with nothing stale 0.556 vs 0.272; `index status` 0.562 vs 0.304.

### Choices where the Python was ambiguous or platform-bound

- `AF_OLLAMA_URL` overrides the endpoint (tests drive the keyword-only path with it); `indexer.py:67` has no override. Parity reaches the same state in the Python with `HTTP_PROXY` to a dead port.
- `st_mtime` is `sec + nsec * 1e-9` as CPython builds it (R1's `pyos::getmtime` divides by 1e9, which can differ in the last bit; the index needs exact equality at `indexer.py:367`).
- Vector norms use an exact `fsum` port and cosine uses CPython 3.12's triple-length `sumprod` (`indexer.py:339`, `:614`); a naive loop lands a few ulps off and can flip near-tie ranks.
- Ollama floats are parsed with a correctly rounded parser (serde_json's default float path can be an ulp off).
- `VERSION_RE` (`indexer.py:80`) uses ASCII digits; Python's `\d` also takes non-ASCII decimal digits in a filename.
- Exception text in the degraded-mode warning follows urllib (`URLError: <urlopen error [WinError 10061] ...>`, `HTTPError: HTTP Error N: reason`); rarer failures (malformed JSON, timeouts) are named the same class with shorter text.
- The indexer's own `die()` (`indexer.py:83`, golden set missing or empty) prints `af index: ERROR:` and exits 1 with no `refusal` in the run log, as af.py records it.
- argparse `type=int` for `-n` and `--k` accepts negative values and slices like Python (`-n -1` drops the last hit).

### Cutover notes (R3)

- The SessionStart hook `system/hooks/index_refresh.py` still spawns `python system/af.py index update` (R2 owns hooks); point it at the binary at cutover.
- No embedding backfill is pending: the live index is fully embedded, and either engine backfills orphan vectors on its next `index update` or search refresh. A model swap still needs one `af index update --rebuild` by the operator (about 13k chunks through Ollama).
- `NOT_PORTED` in `src/cli.rs` lost `index` and `search`; the R2 and R4 merges will touch the same line.
