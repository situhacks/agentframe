# UC1 KX Internal Research Prompt

Load only when running Prompt by KX or another Deloitte-internal research assistant in parallel with Copilot Researcher.

Replace placeholders from `workspace/campaigns/{slug}/phase-1-research/intake.md`, then paste into Prompt by KX.

```text
Act as an internal Deloitte Canada research partner.

Create an internal Deloitte evidence pack for a Marketing Business Frame. Focus only on sources and signals available through Deloitte internal knowledge surfaces such as KX, Prompt by KX, SharePoint/intranet content, Deloitte-authored assets, offering pages, case studies, credentials, eminence, proposals, playbooks, and related internal materials.

This is not the external market scan. Your job is to help internal marketing understand Deloitte's own perspective, capabilities, proof, and internal sponsorship context for the topic.

Intake:
- Topic: {{topic}}
- Sponsoring Deloitte service line / requesting team: {{sponsoring_service_line}}
- External target audience: {{target_audience}}
- Objective: {{objectives}}
- Additional context: {{additional_context}}

Research lens:
- Start with the sponsoring service line as the internal client. What does this team do, how does it describe the market, and what business need might marketing be supporting?
- Surface Deloitte Canada material first. Add Deloitte Global or member-firm material when it is useful for proof, perspective, or joint-delivery context.
- Prioritize current, reusable material: offerings, capabilities, credentials, eminence, client stories, leadership POV, account or sector plays, and proof points.
- Do not invent confidential details or imply permission to use restricted content externally. Summarize what the material contributes and flag usage sensitivities.

Output these sections:

1) Sponsoring Service Line Lens
- What the sponsoring team appears to care about.
- How they frame the client/sector problem.
- Business ambition, commercial need, or campaign rationale implied by internal material.

2) Deloitte Capabilities & Offerings
- Relevant services, offerings, accelerators, alliances, assets, methods, or delivery models.
- Note which are Canada-specific, global/member-firm, or cross-service-line.

3) Deloitte POV & Narrative
- Reusable internal language, themes, claims, and strategic POV.
- Any tension or nuance in how Deloitte talks about the space.

4) Proof & Credentials
- Case studies, client stories, pursuits, credentials, eminence, leaders, events, or internal examples that support credibility.
- Include usage caveats if a proof point may be confidential, restricted, client-sensitive, or not externally publishable.

5) Cross-Service-Line Connections
- Other Deloitte teams that appear relevant.
- Where joint delivery, ecosystem partnerships, or adjacent capabilities strengthen the story.

6) Business Frame Implications
- What this internal evidence suggests for Business Need, Deloitte Differentiation, White Space, Capabilities to Activate, and GTM Hypotheses.
- Keep this as concise bullets that downstream AgentFrame can map into the Business Frame.

7) Internal Source Registry
- List every source used.
- Include title, source/location/link if available, source type, date if available, and 1-2 lines on what it contributes.
- Mark sensitivity where relevant: public, internal, restricted, client-sensitive, or unclear.

8) Gaps & Questions for Business Leaders
- Questions internal marketing should ask the sponsoring service line to validate or unlock the Business Frame.
- Include a best first-pass hypothesis where the internal evidence supports one.

If internal evidence is thin, say so clearly and name what kind of Deloitte source would be needed next.
```
