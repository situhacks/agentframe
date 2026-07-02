# deep-research — prompt provenance & refresh

`SKILL.md` is AgentFrame-owned, but its research loop is lifted from the DeepResearch-Bench-winning open architecture rather than written from scratch. This file pins each lifted element to its upstream source so the skill can be refreshed by diffing upstream against the pinned SHA.

## Upstream pins

| Element in SKILL.md | Upstream | File | Pinned SHA |
|---|---|---|---|
| Five specialist role prompts (evidence_gatherer, mechanism_explorer, comparator, critic, horizon_scanner, generalist fallback) | `LunonAI/lunon-deep-research` (MIT) | `deep_research/pipeline/specialists.py` (`_ROLE` dict) | `4c1d0014` |
| Finding-atom schema (statement / source_name / url / quote ≤125 chars / optional causal chain) + "reconcile conflicts in the statement" + snippet-vs-full-fetch rule | `LunonAI/lunon-deep-research` | `deep_research/pipeline/specialists.py` (`_EXTRACT_SYSTEM`) | `4c1d0014` |
| Architect pattern: typed queries (`factual/causal/comparative/critical/trend`) routed 1:1 to roles, TOC-first planning, query→section mapping, acceptance criteria with categories + verification, enumerated-deliverable decomposition | `LunonAI/lunon-deep-research` | `deep_research/pipeline/architect.py` (`_SYSTEM`, archetype dict) | `4c1d0014` |
| Role architecture lineage (Lunon adapted it from NVIDIA AI-Q `researcher_agent/prompts/*.j2`; AI-Q = DeepResearch Bench #1, 55.95 DRB-I / 54.50 DRB-II) | NVIDIA AI-Q blueprint | per Lunon module docstrings | — |
| Synthesis-gate checklist (angles / full fetches / concrete data / benefits+limitations / currency) | `bytedance/deer-flow` (MIT) | `skills/public/deep-research/SKILL.md` | `70d53da7` |
| Compression-at-specialist-boundary (atoms only cross the boundary; raw pages never persist) | `langchain-ai/open_deep_research` (MIT) | `src/open_deep_research/prompts.py` (researcher compression step) | pattern, not text |

Benchmark context at adoption (2026-07-01): Lunon self-reports 0.5351 RACE on DeepResearch Bench; AI-Q 55.95 (#1); Gemini 2.5 Pro DR ~49; OpenAI DR ~47; LangChain ODR 43.44. Lunon's benchmark exemplar reports live at `submission/reports/` upstream.

## Deliberately not lifted

- **RACE-judge tuning:** `readability_rewrite.py`, footnote/numbering normalizers, CJK post-processing, `w9_readability` cache — benchmark-judge optimization, not research quality.
- **Multi-model ensemble:** Lunon's Claude-Opus + GPT + Nemotron + DeepSeek stack and all API clients — the whole point of this skill is running on the session's own inference.
- **Benchmark-scale budgets:** 48–64 queries / 24–32 acceptance criteria / 14–24 atoms per specialist, entity-matrix and numeric-spine JSON contracts — scaled down to operator-sized tiers in SKILL.md.
- **Weizhena/Deep-Research-skills** (evaluated, not adopted): its per-harness forks and items×fields matrix shape were passed over in favor of one harness-agnostic SKILL.md; its resume-from-findings-files pattern IS reflected in the Run layout resume rule.

## Refresh procedure

1. Diff each pinned file upstream: pinned SHA → current HEAD (e.g. `gh api repos/LunonAI/lunon-deep-research/compare/4c1d0014...HEAD`).
2. Adopt deltas that change the research loop (role definitions, atom schema, planning structure). Ignore deltas in the not-lifted categories above.
3. Update the pinned SHAs here, smoke-test per the skill's tier table, and append a `system_changes` row via `system/audit/writer.py`.
