# UC1 Research Artifact Prompt

Canonical current prompt template for the UC1 Copilot Researcher source artifact.

Load only after UC1 intake is confirmed and Copilot Chat has Researcher enabled. If the operator gives feedback on this prompt's structure, source coverage, or output quality, update this file.

Replace placeholders from `workspace/campaigns/{slug}/phase-1-research/intake.md`, then paste into Copilot Chat.

```text
Act as a strategic marketing research analyst for Deloitte Canada.

Create a research artifact that internal marketing can use to prepare for a Business Frame conversation with the sponsoring service line. The artifact should be useful for a human reviewer and for a downstream LLM: structured, source-grounded, and easy to parse.

Intake:
- Topic: {{topic}}
- Sponsoring Deloitte service line / requesting team: {{sponsoring_service_line}}
- External target audience: {{target_audience}}
- Objective: {{objectives}}
- Additional context: {{additional_context}}

Research lens:
- Canada-first. Use Canadian market, policy, procurement, regulatory, economic, and sector context wherever relevant.
- Treat the sponsoring service line as the internal client. Capture how Deloitte talks about the space, what capabilities/proof points matter, and where other Deloitte teams or member firms may be relevant.
- Separate neutral market facts from competitor messaging. Competitor content is evidence of market positioning, not neutral truth.
- Prefer sources from the last 5 years. Use older sources only when they are still essential context.

Source coverage:
- Deloitte sources: Deloitte.ca, Deloitte Insights, public Deloitte thought leadership, and uploaded Deloitte materials when available.
- External market sources: government, regulators, industry bodies, reputable press, conference or procurement signals.
- Competitor sources: Big 4, MBB, Accenture, IBM, CGI, relevant boutiques/specialists, and industry-adjacent advisory players when they compete for attention, budget, or bids.

Citations:
- Use numbered inline citations like [1], [2], [3] for substantive factual claims.
- Keep numbering stable across the artifact.
- Include every cited source in the Source Registry with title/link, contribution summary, URL if needed, and source type.

Output these sections in order:

1) Industry Snapshot
- Define the space in plain language.
- Explain why it matters now in Canada.
- Include 5-8 concise evidence-backed bullets.

2) Deloitte POV
- Summarize Deloitte's relevant capabilities, offerings, proof, and perspective.
- Lead with the sponsoring service line when evidence supports it.
- Add adjacent Deloitte Canada or member-firm perspectives where the topic likely requires joint delivery.

3) External
- Summarize the Canadian market landscape: conditions, trends, policy/regulatory signals, procurement/budget signals, buyer pressures, and sector implications.
- Segment the landscape if the audience spans distinct buyer groups.

4) Competitor
- Identify the most relevant competitors and adjacent players.
- For each material competitor: narrative, value proposition, proof points, overlap with Deloitte, and whitespace.

5) Theme List (Ranked)
- Produce a broad ranked list of possible campaign themes.
- For each theme: title, 2-3 sentence description, and evidence-based rationale.
- Cap at 30 themes; fewer is fine if the space is narrower.

6) Recommended Shortlist
- Pick the top 5 themes from the ranked list.
- For each: score or summarize market relevance, Deloitte Canada capability fit, competitor whitespace, and Deloitte International leverage.
- Include a short rationale for why it belongs in the shortlist.

7) Source Registry
- List every cited source in numeric order.
- Include source type: Deloitte Internal, External Market, or Competitor.
- Add 1-2 lines on what the source contributes.

8) Gaps & Open Questions
- List the questions internal marketing should take to business leaders.
- For each, include a best first-pass hypothesis where the evidence supports one.
- Clearly label hypotheses as hypotheses.

Default to depth and breadth. If evidence is thin, state the gap rather than filling it with generic marketing language.
```
