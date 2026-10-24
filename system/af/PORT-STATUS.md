# af Rust port: status (work file, deleted at cutover)

Stages R1 and R2 of the port planned in `.claude/plans/2026-10-23-life-os-PLAN.md` section 7. Baseline: `system/af.py` at `a6fdb3a8` (unchanged through `decd5fda`); R2's baseline for `system/voice_bundle.py`, `system/voice_lint.py` and `system/hooks/*.py` is `b030c418`.

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
| `pipe` | ported and parity PASS (R4, below) |
| `studio` | ported and parity PASS (R4, below) |
| `system/hooks/*` | not started (R2+) |
| `session register / list / stop / end / bundle` (R6, new) | built to `.claude/plans/2026-10-24-af-life-verbs-SPEC.md`; `src/sessions.rs` |
| hook handlers `session-start / stop / session-end / notification` (R6, new) | `sessions::hook_*`; hidden `af session hook_*` until R2's `af hook` lands |
| `open add / close / due`, `brief`, `close` (R6, new) | `src/open.rs`, `src/brief.rs`, `src/close.rs`; shared helpers `src/life.rs` |
| doctor: life.md cap, open.md grammar, idle rows, workspace remote, stale handoffs (R6) | `life::doctor_checks`, called from `check_system` |
| `voice bundle` | ported and parity PASS (R2) |
| `voice lint` (system/voice_lint.py's CLI: `<head>`, `--project/--deliverable`, `--prev`, `--json`) | ported and parity PASS (R2) |
| `pipe` | not started (R2+); doctor's pipeline checks are ported |
| `studio` | not started (R2+); doctor's studio checks are ported |
| `af hook pre-tool-use` (version, PPT staging, voice, publish guards) | ported and parity PASS (R2) |
| `af hook post-tool-use` (PPT follow-through, voice report after `af version`) | ported and parity PASS (R2) |
| `af hook session-start` (index refresh) | ported and parity PASS (R2); spawns `python system/af.py index update` until `af index` exists |

## Crate layout

- `Cargo.toml`: package `af`, library `afcore` (`src/lib.rs`), binary `af` (`src/main.rs`). Deps: clap, regex, chrono, walkdir, serde_json, yaml-rust2; R2 added fancy-regex (the lint's look-around patterns) and rusqlite with bundled SQLite (the index age check; R3 should reuse the same line). `Cargo.lock` belongs in git.
- `src/cli.rs`: clap parser (argparse abbreviations, last-wins options, draft's required group), dispatch, root discovery, the run-log wrapper that mirrors af.py `main()`.
- `src/error.rs`: `die()` refusals, usage errors, crashes, the recorded last error.
- `src/out.rs`: stdout/stderr with Python's Windows `\n` to `\r\n` translation.
- `src/pyos.rs`, `src/pyglob.rs`: ntpath/os.path, file I/O (`utf-8-sig` read, `\n` write), `repr(str)`, glob/fnmatch as CPython 3.12.
- `src/fm.rs`, `src/core.rs`: frontmatter-as-text helpers, `re.sub` templates, `Ctx` and shared row/version logic.
- `src/cmds.rs`: the six transition verbs. `src/packs/`: compiled-in marketing and careers rules.
- `src/runlog.rs`: run-log write/read and the buttons note. `src/doctor.rs`: every doctor check.
- `src/media_shelf.rs`: port of `media_intake.notes()`. `src/pyyaml.rs`: PyYAML `safe_load` semantics over yaml-rust2.
- `src/voice.rs` (+ `src/voice/bundle.rs`, `src/voice/lint.rs`): `af voice bundle` (through the button wrapper, so it writes a run-log row) and `af voice lint` (intercepted before it, like voice_lint.py: no run-log row). `src/pyjson.rs`: `json.dumps` (compact ASCII and `indent=1`), float `repr`, `str()`/`repr()` of decoded JSON.
- `src/hooks.rs` (+ `src/hooks/*.rs`): `af hook <event>`, intercepted before the button wrapper (no run log, flags ignored). One file per Python hook script, plus `shlex.rs` (CPython's posix `shlex.split`) and `pathlib.rs` (`str(Path)`, `/`, `.name`, `.resolve()`, case-insensitive containment).
- `tests/*.rs`: ported pytest cases (`tests/common/mod.rs` rebuilds the conftest fixtures) plus `media_shelf.rs`; R2's are `voice_*.rs` and `hooks_*.rs` (`hooks_shlex.rs` checks the tokenizer against `hooks_shlex_cases.json`, produced by CPython 3.12). `tests/parity.py`: Python-vs-binary harness (`-k`, `--keep`, `-v`); `compare()` runs af.py verbs, `compare_script()` runs `voice_lint.py` and the hook scripts against `af voice lint` and `af hook <event>`.

## Verification (last run, R2)

- `cargo test`: 260 passed (3 unit, 257 integration); R2 added 105: every case of `test_voice_bundle.py` (9), `test_voice_lint.py` (23), `test_voice_guard.py` (11), `test_version_guard.py` (14), `test_ppt_master_guard.py` (17 hook cases; its 5 doctor stray-deck cases were ported in R1), `test_publish_guard.py` (11), `test_index_refresh.py` (5) and `test_harness_hooks.py` (2), plus 13 beyond them (the verb surface, fail-open shapes, the tokenizer table). Not ported (R1): the two `LazyIndexer` cases in `test_af_buttons.py` and the `pipe stage` half of one `test_af_pipe.py` case.
- `python system/af/tests/parity.py`: 71/71 PASS (19 verb and doctor cases, 34 media-shelf cases, 2 `voice bundle` cases, 1 `voice lint` case of 15 steps, 15 hook cases: version 22 payloads, voice 21, PPT 14, publish 12 plus a broken tracker, empty and garbage stdin 26, session start 9 fixtures). Nothing is masked beyond R1's clock and root masks: the session-start steps wait for the detached refresh to remove its lock before the tree (refresh log and the child's run-log row included) is compared. A mutated message in either a guard or the lint shows as DIFF.
- Lint differential fuzz (scratch, not committed): 1,000 random documents (two seeds) plus 300 quote-edge documents over the patterns' trigger words, Unicode whitespace, BOM/CRLF frontmatter and quotes; `--json` and the report identical for all.
- `python -m pytest system/tests -q -p no:cacheprovider`: 390 passed, 1 skipped (unchanged).
- `cargo build --release`: no warnings; `target/release/af.exe` 5,841,920 bytes (3,331,072 at R1; the bundled SQLite is most of the growth).
- Hook latency, wall time per process, median of 7 runs, same payload, fixture root (outputs equal in every row):

| Python hook | `af hook` event | Python | af |
|---|---|---|---|
| version_guard.py (Edit of a drafting head) | pre-tool-use | 84 ms | 7.9 ms |
| voice_guard.py (Bash `af ready`, lint runs) | pre-tool-use | 207 ms | 17.3 ms |
| ppt_master_guard.py (Bash PPT init in the vendored folder) | pre-tool-use | 86 ms | 6.5 ms |
| publish_guard.py (Substack update_draft) | pre-tool-use | 202 ms | 10.3 ms |
| index_refresh.py (fresh index, no spawn) | session-start | 113 ms | 11.1 ms |

  The voice guard's floor is regex compilation (about 12 ms of CPU for the lint's patterns), spread over threads in `voice/lint.rs`.

## Known divergences

- Help and usage text come from clap, not argparse. Exit codes and the run-log `refusal: usage` match.
- A crash prints `af: CRASH: Type: msg` (or Rust's panic text), not a traceback. Exit 1 and the run-log row (exit 2, `crash`) match.
- Not-ported verbs exit 1 with a pointer to `python system/af.py` and write no run-log row.
- Root: `AF_ROOT`, else the nearest ancestor of the binary, then of the cwd, holding `AGENTS.md` and `system/`.
- Pack rules are compiled in and active only while the pack ships `rules.py`; a pack shipping a `rules.py` with no Rust counterpart is refused.
- Media-shelf YAML: values, types, constructor errors, tab and duplicate-anchor errors match PyYAML. Still different: scanner and parser error text for malformed YAML (yaml-rust2 wording), NEL/LS/PS as line breaks outside quoted scalars, `!!set` print order (hash order in Python), multi-line quoted keys in flow mappings (yaml-rust2 accepts).
- Messages follow CPython 3.12 (`KeyError: slice(None, 12, None)`, `datetime.date` type names). A Python upgrade can move them.
- Hooks read stdin as UTF-8. The Python scripts read it in the locale code page (cp1252 here), so a payload carrying non-ASCII text (a path or command with `é`) is mojibake in Python and read correctly by `af hook`.
- `json.loads` accepts `NaN`/`Infinity` literals, lone surrogates and integers past 64 bits; serde_json rejects the first two (the hook fails open with `{}`) and reads the third as a float.
- `voice_guard._tool_output` joins a dict response's string values in insertion order; serde_json iterates keys sorted. Only the `af version:` containment test reads the join.
- Regex classes: Python's `\s` (with U+001C..U+001F) is spelled out where the lint's patterns use it; `\w`/`\b` follow Unicode word characters in both engines but Python's `\w` excludes combining marks. `datetime.fromisoformat` is ported for the extended and basic ISO forms, not week or ordinal dates.
- `af voice lint` usage errors and crashes print clap text and `af: CRASH: ...`, not argparse usage or a traceback; exit codes (2, 1) match.
- Unknown flags: the Python guards ignore argv and index_refresh.py's argparse exits 2 on an unknown flag; `af hook` ignores every flag (exit 2 from a PreToolUse hook would block the tool). A missing or unknown event exits 1 with a message.

## Stage R4: `pipe` and `studio`

Baseline: `system/af.py` at `b030c418`.

### Subcommands

| Subcommand | Parity case(s) | Status |
|---|---|---|
| `pipe save` | `pipe-save`, `pipe-topology`, `pipe-close-search` (reopen) | PASS |
| `pipe start` | `pipe-start`, `pipe-lifecycle` | PASS |
| `pipe stage` | `pipe-stage`, `pipe-no-board` | PASS |
| `pipe close-search` | `pipe-close-search`, `pipe-close-search-empty` | PASS |
| `pipe round` | `pipe-round` (incl. the IndexError crash path) | PASS |
| `pipe archive`, `pipe unarchive` | `pipe-archive` | PASS |
| `pipe board` | every pipe case | PASS |
| `studio new` | `studio-new`, `studio-new-packs`, `pipe-topology` | PASS |
| `studio stage` | `studio-stage`, `studio-no-calendar` | PASS |
| `studio post` | `studio-post`, `studio-lifecycle` | PASS |

### Layout

- `src/pipe.rs`: the `PipeCmd` clap surface, `topology_pack`, board helpers (`pipe_row_add`, `app_dir`, `stamp_shipped`), every `cmd_pipe_*`.
- `src/studio.rs`: the `StudioCmd` clap surface, slot helpers (`mapping_row_set`, `mapping_row_add`), every `cmd_studio_*`.
- Shared-file edits: `src/lib.rs` (two `pub mod` lines), `src/cli.rs` (`NOT_PORTED` drops `pipe`/`studio`; two `Cmd` variants and two dispatch arms), `tests/parity.py` (one appended block, `CASES +=`).
- Tests: `tests/pipe_verbs.rs` (test_af_pipe.py), `tests/pipe_rounds.rs` (test_af_pipe_rounds.py), `tests/studio_verbs.rs` (test_af_studio.py). `af_pipe.rs` and `af_studio.rs` keep R1's fixture-written doctor cases; the verb-driven versions sit beside them.

### Verification (last run)

- `cargo test`: 217 passed (3 unit, 214 integration).
- `python system/af/tests/parity.py`: 69/69 PASS (R1's 53 plus 16 pipe/studio cases).
- `python -m pytest system/tests -q -p no:cacheprovider`: 390 passed, 1 skipped.
- `cargo build --release`: no warnings; `target/release/af.exe` 3,658,240 bytes.

### Divergences and choices

- `pipe round NUMBER` (af.py 2577, `type=int`): ASCII digits, sign, single underscores, whitespace, i128 range. Python's `int()` also takes Unicode decimal digits and unbounded values. `-1` is a positional for both (argparse has no negative-number options there).
- `shutil.move` (af.py 1102, 1296, 1313) is `std::fs::rename`: same-volume moves only. Every move here stays inside `workspace/pipeline/`; a cross-volume move would crash where Python copies.
- `shutil.copy2` (af.py 1535) copies bytes plus access and modification times, not Windows file attributes.
- `--with ", x"` (af.py 1249) reproduces Python's `IndexError: list index out of range` crash instead of refusing.
- Paths in `pipe round`, `pipe archive`/`unarchive`, `studio new`, and the skeleton refusals come from `os.path.relpath` without `/` normalisation, so they print with `\` on Windows; the port does the same.

### Left for cutover

- The careers and creator packs, `.claude/skills/job-scout/SKILL.md`, and two `library/process/` files say `af pipe ...` / `af studio ...`; they resolve to whatever `af` is on PATH once root discovery is decided (see Cutover notes).
- When R2 and R3 merge, `NOT_PORTED` in `src/cli.rs` is edited by all three stages; the merged line keeps only the verbs still unported.

## Cutover notes

- Add `system/af/target/` to `.gitignore` (this stage did not touch it).
- Replace the `rules.py` presence switch in `src/packs/mod.rs` when the packs drop their Python modules.
- Decide root discovery (keep `AF_ROOT`, or install next to the repo) before wiring `.claude/settings.json` and PATH.
- Port order suggestion for R2: `pipe` and `studio` (doctor already covers their state), then `index`/`search`, `voice`, hooks.

## R6: life and session verbs (choices where the spec is silent)

- Registry: `stale_after` is a constant 120 minutes. Name segments match `[A-Za-z0-9][A-Za-z0-9_.-]*`, one segment after the prefix. A missing `--model` writes `-`. `--brief` is vault-relative (absolute accepted). A session id resolves to its last row that has not ended, else its last row. `session list` rewrites each stored status to the effective one (live and idle both ways). Lines outside the table, CRLF, and BOM are preserved; new files are LF.
- Roster: `live_state` is the roster entry's `status` matched on `sessionId`, else `absent`. The `claude` call times out after 5 s. The "unavailable" note goes to stdout in table mode and stderr with `--json`.
- Handoff check: `written` has minute precision, so a handoff is required when the newest touched mtime, floored to the minute, is later than `written`. An unparseable `written` counts as required. The handoff never counts as its own touched file. With no `Files touched` and no `write_surface`, nothing is required. An ended row never requires a handoff.
- Stamps: the SessionStart handler stamps `last_seen` and revives an ended row to `live` (a resume). Stop and Notification stamp `last_seen` and set `live` only on rows that have not ended. `af session bundle` is read-only.
- Hooks: SessionEnd maps `prompt_input_exit` to `exit` and any unknown reason to `other`. Notification finds the row by payload `session_id`, then by `session_name`/`name`/`session_title` against registry names; the hooks docs do not say which id a background-session notification carries. `sessionTitle` goes out on every SessionStart source; the harness applies it on startup, resume, and fork. Handlers take `(ctx, payload)` and never fail: a bad payload yields empty stdout and exit 0.
- Bundle: the roster line lists rows that have not ended, with their effective status, and runs no `claude` call. An over-cap `life.md` prints `af: life.md is N lines over its cap` before the body (plan 4.2 rule 1).
- open.md: the grammar applies to every list item (`- `, `* `) under any heading; prose, headings, and HTML comments are free. A `[x]` line is a doctor issue. `n` is the 1-based ordinal of open loops in file order; `due` prints `(Nd, #n)` after each line. Prefix match is case-insensitive on the text, and an exact text match beats prefixes. `--from` defaults to `main`. A missing section is appended as `## <section>`. Text that would not parse back is refused. "Oldest first" sorts by the opened date.
- Month log: created with no heading. `af open close` appends `closed: <text> → <outcome>` before removing the line, so a failure leaves the loop open rather than lost.
- Brief: "handoff newer than last_seen" is implemented as the Stop check (`handoff current | handoff stale | no handoff`). The Stop hook stamps `last_seen` after the chat writes its handoff, so the literal comparison would read false on every normal stop. Studio "this week" means dated on or before today+6, passed slots included, state not posted or dropped. Empty sections are omitted except Due and Open. `--write` overwrites today's file.
- Index age (bundle, brief): the `system/index/vault.db` mtime is over 24 h. No SQLite dependency; R2's index port can swap in `meta.updated_at`.
- `af close`: `completed_at: <today>` goes on the line after `status:`, and `last_activity` is touched. The activity line is `project_closed: <slug>; folder moved to completed/`. A published row counts as unharvested for a skill when no activity line names both the skill (`deliverable-harvest` or `deliverable_harvest`, likewise voice) and the row slug or file. Refusals: unknown, already closed, target exists, not active, drafting rows; `--force` overrides the last two.
- Doctor: a handoff with no parseable `written` is a note. The 7-day idle note uses the effective status, so a stored `live` row 10 days old notes too. Remotes come from `workspace/.git/config` (a `gitdir:` file is followed) with no git process.
- Run log: rows are recorded as verb `session`, `open`, `brief`, `close`. A direct `af session stop` exit 3 is counted as a `session` refusal by `runlog::button_notes`. The merge should exclude it there (`runlog.rs` was outside this stage).
- Verification: `cargo test` 213 passed (58 new across `tests/sessions_*.rs`, `tests/life_*.rs`; helpers in `tests/life_fx/`). `cargo build --release` has no warnings. Release `af session hook_session_start` takes about 0.035 s and `af doctor` about 0.18 s.
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
- Wire `.claude/settings.json` to the binary: SessionStart `af hook session-start`; PreToolUse `Edit|Write`, `Bash|PowerShell` and `mcp__substack__update_draft` each `af hook pre-tool-use`; PostToolUse `Bash|PowerShell` `af hook post-tool-use`. One command per matcher: `af hook` runs every guard the old matcher ran (Bash runs the PPT guard, then the voice guard; the first deny wins, otherwise additional contexts join with a blank line). Update the wiring assertions in `test_version_guard.py`, `test_harness_hooks.py` and `tests/hooks_*.rs`, which look for the `.py` names.
- `af hook session-start` spawns `python -c <shim> <root>/system/af.py index update <lock>` exactly as index_refresh.py does (the shim waits for the update and removes the lock). When R3's `af index` lands, change `index_refresh::spawn_argv` to run the binary; the lock removal still needs a waiting parent (keep the shim or add a child mode to `af hook`).
- The guard messages still say `python system/af.py ...` (parity); they change with every other reference at cutover.
- Delete `system/hooks/autonomy_guard.py` and `board_guard.py` (no-op stubs, not ported).
