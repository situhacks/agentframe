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
| `pipe` | ported and parity PASS (R4, below) |
| `studio` | ported and parity PASS (R4, below) |
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
