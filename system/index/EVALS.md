# Retrieval Evals

Two layers:

- **Tracked (this file):** the eval method, the lessons that transfer, the corpus boundary, and the current standing.
- **Per-instance (gitignored):** `golden-set.yaml` beside this file holds the question-to-path pairs. Each instance harvests its own set from its own history; the queries describe the operator's life and never leave the machine.

## Method

1. **Harvest** about 30 pairs phrased the way the operator asks (voice register, not filenames), taken from real `af search` calls in session transcripts, never guessed. Verify every expected path on disk.
2. **Score** with `af index eval --k 5`: recall@5 over the pairs (a pass is any expected path in the top five) and MRR on the first expected hit.
3. **Classify every miss** into one bucket, because each has a different fix:
   - stale path (the file moved) or wrong expectation (the index found the better file): fix the golden set;
   - vocabulary mismatch (right document, other words): embeddings;
   - ranking competition (found, outranked): fusion or boosts;
   - intent-shaped query hunting fact-shaped documents: query rewriting, not another model.
4. **Gate** every ranking, chunking, model, or corpus-boundary change on the same set. A change ships only if the number holds or improves. Do not tune against one query on a set this small.

## Golden-Set Upkeep

- A closed project stays in the set at its moved path; closing a project or moving a file is a golden-set edit.
- A bare scaffold with no body is corpus thinness, not a retrieval miss. Accept it as a standing miss or retire the expectation.

## Corpus Boundary

`CORPUS_ROOTS` in `system/indexer.py`: `workspace/` (life included, minus the voice corpus at `workspace/life/voice/`), `library/assets/`, and `.claude/plans/`. Search finds what the operator made or lived. Machinery (`system/`, process files, templates, skills) is reached through the routers and catalogs, never searched. The schema skeletons in `library/schema/` sit outside the corpus, so a template never outranks the content it shapes.

## Model And Fusion

- `qwen3-embedding:0.6b` with the query-side instruction prefix its model card documents, and equal reciprocal-rank-fusion weights. `nomic-embed-text` stays installed as the fallback. A model swap is one constant (`EMBED_MODEL`) plus `af index update --rebuild`; dimension mismatches are refused, never mixed.
- Leaderboard rank does not predict corpus fit: used bare, the ranked favourite scored 21 points below keyword search on this corpus.
- Usage format outweighs model choice: the query prefix alone moved recall@5 from 62% to 86%. Read the model card before a bake-off.
- A fusion-weight sweep that stays flat within noise at this set size is a plateau. Stop tuning.

## Refresh

`system/hooks/index_refresh.py` runs on SessionStart and spawns `af index update` detached when the index is more than a day old. It never builds from cold and never blocks.

## Current Standing

Replace this section when `af index eval` produces a new number; do not append.

- recall@5 = 21/23 (91%), MRR = 0.591, gate 86%, corpus 1,423 files / 13,400 chunks, all embedded with qwen3-embedding:0.6b.
- Ranking classes: `_archive/` 0.45; `workspace/life/` outside `_history/` and `sources/` 1.5 (`life`); `knowledge/` 1.5; `sources/` and `_local/` 1.15; `.claude/plans/` 1.05; routers 1.15. Prior versions rank down 0.55; tracker heads 1.9; registered sources 1.25.
- The two open misses: a project post plan outranked by the campaign's own brief and messaging draft for a generic query, and a long voice-register query whose extra words pull toward FDE material; both are query-shape misses, not corpus or path misses. Nineteen golden paths moved with the life migration and the set follows the new paths.

