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

- RESOLVED 2026-08-27. Career-bank template layer competed with the content
  layer: `library/context/operator-schema/career/` holds 0.5-3KB skeletons of
  the same documents `library/context/operator/career/` holds populated at
  4-35KB, so a template could outrank the content it templates. These are NOT
  duplicates and were never to be consolidated - operator-schema is the tracked
  public skeleton downstream copies need. Fixed as an indexer scope exclusion
  matching the voice exclusion: route loads the schema, search skips it.
  q19-q22 (proof-points, master-cv, interview-playbook, search-profile) all
  pass now; they were the queries the defect was reported from.

- RESOLVED 2026-08-27. Golden set carried pre-archive paths after the careers
  rebuild moved magical / rbc / eliseai under `applications/completed/`.
  Five expectations repointed; q10-q13 recovered.

## Standing, 2026-08-27

**2026-08-27: recall@5 = 24/29 = 83%, MRR 0.623** (index excludes
operator-schema; golden paths current; 10454/10730 chunks embedded).

Against the recorded 86% baseline this is one query down, and it is q16 rather
than a ranking regression: `applications/completed/rbc-agentic-ai-portfolio-lead/
application.md` is a 1.3KB bare scaffold with no body - RBC was dropped right
after `af pipe start`, so there is nothing beyond frontmatter to match. Corpus
thinness, not retrieval. Either accept it as a standing miss or retire the
expectation; do not tune ranking against it.

The four genuine misses are unchanged and remain the real work: q02
(measurement literacy to Kazmier), q03 (Mike Duffy recruiter screen), q05
(background-to-JD mapping), q06 (Banyan angle). All four are synthesis queries
whose answer is spread across several documents rather than sitting in one.

## Boundary change and re-harvest, 2026-09-09

Two operator decisions, taken together because each one moved the golden set.

**The corpus is the personal layer.** `CORPUS_ROOTS` dropped `system/` and the
library machinery (`process/`, `deliverables/`, `domains/`) and now walks
`workspace/`, `library/context/` (still minus the voice corpus and the schema
skeletons), `library/lenses/`, `library/assets/`, and `.claude/plans/`. The
operator's framing: search exists to find what he made or lived, faster than
grep; how the machine works is routed by the AGENTS routers and their catalogs,
never searched. The rebuild dropped 646 files (1511 -> 1097 files, 13546 ->
9829 chunks), all of them skill bodies, process files, templates, and system
docs. Plans stay in: they are his decision history, not machinery.

**Archived applications leave the golden set.** The search is closed and every
application sits under `applications/completed/`; the operator retired those
queries rather than keep repointing them. Sixteen application queries and the
two system-shaped queries (`upstream-sync`, `bounded-autonomy`) went with them.
Twelve replacements were harvested from real `af search` calls found in the
session transcripts (2026-08-24 to 2026-09-09), each expected path verified on
disk; the file marks them `[real]`. Twenty-three queries now, so the set is
thinner than the 30-pair target and every number below carries that caveat.

**Refresh without remembering.** `system/hooks/index_refresh.py` runs at session
start on all three harnesses and spawns `af index update` detached when the
index is more than a day old. It never builds from cold and never blocks.

| Measurement | Result | Date |
|---|---|---|
| Golden set after retirement + re-harvest, first run | recall@5 = 18/23 = 78% · MRR 0.549 | 2026-09-09 |
| After repointing 3 stale paths and 1 wrong expectation | **recall@5 = 22/23 = 96% · MRR 0.679** | 2026-09-09 |

Error analysis on the first run, one bucket each:

- q01, q02, q11 (joyce-hair-pilot, vancouver-ai-consultancy): **stale paths**,
  the same defect as 2026-08-27. Both projects closed since the harvest and
  moved under `workspace/projects/completed/`; the search ranked the moved files
  second. Repointed. Rule, now stated in the golden file's header: closed
  projects stay in the set at their moved path, and a project close is a
  golden-set edit.
- q19 (Aeroplan / Cobalt): **wrong expectation**, not a miss. The harvest
  guessed `life/knowledge/financial-accounts.md`; the index put
  `life/_local/travel-credit-card-pov/travel-credit-card-pov-v8.md` first at
  0.259, which is the deliverable the query was actually after. Added as the
  primary expectation.
- q18 ("career as a project agent architecture POV post" ->
  `post-12-career-is-a-project/post-12-plan.md`): **ranking competition**, the
  one genuine miss. The campaign's messaging architecture, project.md, and
  campaign brief all carry the same vocabulary more densely than the plan file.
  Same family as the 2026-08-27 synthesis misses; do not tune fusion weights
  against one query on n=23.

The 83% -> 76% drift BB-2026-09-08-01 reported was corpus growth competing with
a set whose targets had moved, not a ranking regression: nothing in ranking
changed between the two readings, and the recovered number above came from
correcting targets and boundary alone.

## Current standing

**2026-09-09: recall@5 = 22/23 = 96%, MRR 0.679** (personal-layer corpus,
1097 files / 9829 chunks, all embedded via qwen3-embedding:0.6b). One
competition miss (q18). Re-measure when the set is back to ~30 real queries;
harvest them from `af search` calls in transcripts, not from guesses.
