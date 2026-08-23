# Retrieval Eval Numbers

> Owner of record for `af index eval` results. This file is tracked; the
> golden-set data behind it is per-instance and gitignored (`golden-set.yaml`
> beside this file), because each instance's corpus differs.
> Method: recall@k over voice-register question→expected-path pairs, MRR on the
> first expected hit. A pass means ANY expected path lands in the top-k.

## Baseline — keyword-only (FTS5 trigram + domain boosts)

| Measurement | Result | Date |
|---|---|---|
| Golden set harvested | 30 pairs | 2026-08-23 |
| Initial run (head-detection bug present) | recall@5 = 67% · MRR = 0.352 | 2026-08-23 |
| After head-flag fix (`is_head` defaulted unversioned files to non-head) | recall@5 = 73% · MRR = 0.553 | 2026-08-23 |
| Final keyword baseline (corrected multi-answer expectations) | recall@5 = 83% · MRR = 0.563 | 2026-08-23 |
| Voice corpus excluded from index (operator decision); one golden query retired | recall@5 = 83% · MRR = 0.563 on 29 queries | 2026-08-23 |

Excluded by operator decision: `library/context/*/voice/` — routed context,
not searched content; a curated snapshot whose sources live in projects.
Route loading still reaches it; only `af search` skips it.

## Hybrid bake-off (2026-08-23, all runs on this instance's golden set)

| Configuration | recall@5 | MRR | Notes |
|---|---|---|---|
| Keyword only (no embeddings) | 83% | 0.563 | reference |
| nomic-embed-text · bare queries · RRF 1:1 | 83% | 0.579 | fixed ranking-competition misses (master-cv, interview-playbook, bounded-autonomy) but regressed two others; did **not** fix the vocabulary-mismatch pair |
| qwen3-embedding:0.6b · bare queries | 62% | 0.457 | leaderboard favorite looked 21 points worse |
| qwen3-embedding:0.6b + documented query instruction | **86%** | **0.598** | Qwen's asymmetric usage (instruct prefix on queries only) flipped it — no rebuild needed, query-side fix |
| Fusion weight sweep (qwen+instruct): semantic 1.4 / 0.7 / 0.5 | 83% / 86% / 86% | 0.574 / 0.601 / 0.604 | recall plateaus at ≤1.0 and degrades above; kept equal weights — not fine-tuning further on n=29 |

**Decision:** adopt `qwen3-embedding:0.6b` with query-side instruction prefix,
equal RRF weights. Swap is one constant (`EMBED_MODEL`) + rebuild; dimension
mismatches are refused, never silently mixed. nomic-embed-text stays installed
as the fallback. Lesson recorded for every future retrieval build here:
**leaderboard rank did not predict corpus fit, and usage format outweighed
model choice entirely** (62→86 from using the model per its own docs).

## Error analysis — final 4 misses

All four sit in one application folder and share a shape: intent-shaped queries
("how do I pitch measurement literacy," "why Banyan specifically, what's my
angle," "my background maps to their JD") hunting documents that store facts,
theses, or requirement tables under different framing. The Duffy transcript miss
(q03) remains ranking competition against denser secondary discussion. Embeddings
did not fix the original two vocabulary-mismatch cases even after winning — the
similarity exists but competing surfaces outrank it. Next earned upgrade for
this family is query rewriting, not another embedding model.

## Corrections

- Earlier backend decision record said CPU-sufficient / GPU deliberately unused:
  wrong in mechanism, right in effort. Ollama used the RX 6900 XT at 100% GPU
  with zero configuration; full-corpus builds ran ~17 min (nomic) / ~29 min
  (qwen 0.6B). The lazy incremental cadence makes embedding cost near-idle
  day-to-day regardless of processor.

## Known corpus defects surfaced by the eval (Builder follow-up)

- Career bank exists twice: `library/context/operator/career/` and
  `library/context/operator-schema/career/` hold near-identical files; both
  rank and split relevance. Backlogged as BB-2026-08-23-01/-02 (local backlog).
