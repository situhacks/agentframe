# Deep Research provenance

`SKILL.md` is AgentFrame-owned. These sources inform its research-loop structure; no upstream runtime is vendored.

| Adopted element | Source | Source path | Ref |
|---|---|---|---|
| Specialist roles, finding atoms, typed-query architecture | `LunonAI/lunon-deep-research` | `deep_research/pipeline/specialists.py`; `deep_research/pipeline/architect.py` | `4c1d00149087fd2b7703e84fbb707f08724aeaca` |
| Synthesis gate: coverage, full-source reads, concrete evidence, limitations, currency | `bytedance/deer-flow` | `skills/public/deep-research/SKILL.md` | `736c412b4c89740fe342261b07a04898fde302e4` |
| Finding-atom compaction before synthesis | `langchain-ai/open_deep_research` | `src/open_deep_research/prompts.py` | `408da442a661ea5e40a6163329f82e3f22628949` |
| Writer craft: bottom line first, signpost headers, argument over catalog, interpreted data, positions with steelmanned alternatives, stated unknowns, new-insight-only conclusion, length fitted to the question | Anthropic `deep-research` skill | `references/report-writer.md` | synced copy, 2026-10-25 |
| Source triage for every specialist: speculation vs confirmed, original vs aggregator, boilerplate authority | Anthropic `deep-research` skill | `references/researcher.md` | synced copy, 2026-10-25 |
| Labeled `inference:` lines kept apart from finding atoms | Anthropic `deep-research` skill | `references/researcher.md` | synced copy, 2026-10-25 |

AgentFrame keeps only the patterns represented in its own `SKILL.md`; its local skill is authoritative. Not taken from the Anthropic skill: its coordinator-never-reads rule, its per-researcher tool-call cap, and its ban on bullets and numbered lists, since project templates own report shape.
