# Retrieval Eval Record

> Two layers, deliberately split:
>
> - **Tracked (this file):** the repeatable eval method, the bake-off lessons,
>   and one instance's results as a worked example.
> - **Per-instance (gitignored):** `golden-set.yaml` beside this file holds the
>   actual question→path pairs. Every AgentFrame instance harvests its own set
>   from its own history — another instance will have different projects, a
>   different corpus, and different numbers. The results below are a record of
>   *this* corpus, not a benchmark anyone should expect to reproduce.

## Method — repeatable on any instance

1. **Harvest** ~30 question→expected-path pairs phrased the way you actually ask
   (voice register, not filenames), pulled from real history: material you went
   looking for again, projects you revisited, facts you re-checked. Store them
   in `golden-set.yaml` (gitignored — your queries describe your life).
2. **Score** with `af index eval --k 5`: recall@k over the pairs (a pass = any
   expected path lands in the top-k), MRR on the first expected hit.
3. **Classify every miss** into exactly one bucket, because each has a different
   fix: vocabulary mismatch (right document, wrong words) → embeddings
   territory; ranking competition (document found, outranked) → fusion/boost
   territory; intent-shaped query hunting fact-shaped documents → query
   rewriting territory, not another model.
4. **Gate every ranking, chunking, or model change** against the same set. A
   change ships only if the number holds or improves.

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

## Hybrid bake-off (all runs on this instance's golden set)

| Configuration | recall@5 | MRR | Notes |
|---|---|---|---|
| Keyword only (no embeddings) | 83% | 0.563 | reference |
| nomic-embed-text · bare queries · RRF 1:1 | 83% | 0.579 | fixed ranking-competition misses but regressed two others; did **not** fix the vocabulary-mismatch pair |
| qwen3-embedding:0.6b · bare queries | 62% | 0.457 | leaderboard favorite looked 21 points worse |
| qwen3-embedding:0.6b + documented query instruction | **86%** | **0.598** | Qwen's asymmetric usage (instruct prefix on queries only) flipped it — no rebuild needed, query-side fix |
| Fusion weight sweep (qwen+instruct): semantic 1.4 / 0.7 / 0.5 | 83% / 86% / 86% | 0.574 / 0.601 / 0.604 | recall plateaus at ≤1.0 and degrades above; kept equal weights — not fine-tuning further on n=29 |

**Decision:** adopt `qwen3-embedding:0.6b` with query-side instruction prefix,
equal RRF weights. Swap is one constant (`EMBED_MODEL`) + rebuild; dimension
mismatches are refused, never silently mixed. nomic-embed-text stays installed
as the fallback.

**Lessons that transfer to any corpus:**

- Leaderboard rank did not predict corpus fit — the ranked-favorite embedding
  scored 21 points *below* plain keyword search on this corpus, bare.
- Usage format outweighed model choice entirely: 62→86 came from using the
  model per its own model card (query-side instruction prefix), not from
  shopping for a different model. Read the card before the bake-off, not after.
- Recognize plateaus: the fusion-weight sweep was flat within noise at n=29;
  further tuning would have been fitting the golden set.

## Error analysis — final 4 misses (this instance)

All four sit in one application-prep folder and share a shape: intent-shaped
queries ("how do I pitch measurement literacy," "why this company specifically,
what's my angle," "my background maps to their JD") hunting documents that store
facts, theses, or requirement tables under different framing. One recruiter-
screen transcript miss remains ranking competition against denser secondary
discussion. Embeddings did not fix the original two vocabulary-mismatch cases
even after winning — the similarity exists but competing surfaces outrank it.
Next earned upgrade for this family is query rewriting, not another embedding
model.

## Corrections

- Earlier backend decision record said CPU-sufficient / GPU deliberately unused:
  wrong in mechanism, right in effort. Ollama used the operator's RX 6900 XT at
  100% GPU with zero configuration; full-corpus builds ran ~17 min (nomic) /
  ~29 min (qwen 0.6B). The lazy incremental cadence makes embedding cost
  near-idle day-to-day regardless of processor.

## Known corpus defects surfaced by the eval (Builder follow-up)

- Career bank exists twice: `library/context/operator/career/` and
  `library/context/operator-schema/career/` hold near-identical files; both
  rank and split relevance. Backlogged locally (BB-2026-08-23-01/-02).
