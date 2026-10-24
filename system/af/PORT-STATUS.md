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
| `session register / list / stop / end / bundle` (R6, new) | built to `.claude/plans/2026-10-24-af-life-verbs-SPEC.md`; `src/sessions.rs` |
| hook handlers `session-start / stop / session-end / notification` (R6, new) | `sessions::hook_*`; hidden `af session hook_*` until R2's `af hook` lands |
| `open add / close / due`, `brief`, `close` (R6, new) | `src/open.rs`, `src/brief.rs`, `src/close.rs`; shared helpers `src/life.rs` |
| doctor: life.md cap, open.md grammar, idle rows, workspace remote, stale handoffs (R6) | `life::doctor_checks`, called from `check_system` |

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
