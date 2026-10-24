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

## Cutover notes

- Add `system/af/target/` to `.gitignore` (this stage did not touch it).
- Replace the `rules.py` presence switch in `src/packs/mod.rs` when the packs drop their Python modules.
- Decide root discovery (keep `AF_ROOT`, or install next to the repo) before wiring `.claude/settings.json` and PATH.
- Wire `.claude/settings.json` to the binary: SessionStart `af hook session-start`; PreToolUse `Edit|Write`, `Bash|PowerShell` and `mcp__substack__update_draft` each `af hook pre-tool-use`; PostToolUse `Bash|PowerShell` `af hook post-tool-use`. One command per matcher: `af hook` runs every guard the old matcher ran (Bash runs the PPT guard, then the voice guard; the first deny wins, otherwise additional contexts join with a blank line). Update the wiring assertions in `test_version_guard.py`, `test_harness_hooks.py` and `tests/hooks_*.rs`, which look for the `.py` names.
- `af hook session-start` spawns `python -c <shim> <root>/system/af.py index update <lock>` exactly as index_refresh.py does (the shim waits for the update and removes the lock). When R3's `af index` lands, change `index_refresh::spawn_argv` to run the binary; the lock removal still needs a waiting parent (keep the shim or add a child mode to `af hook`).
- The guard messages still say `python system/af.py ...` (parity); they change with every other reference at cutover.
- Delete `system/hooks/autonomy_guard.py` and `board_guard.py` (no-op stubs, not ported).
