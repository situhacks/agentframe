# Research And Signals

Shared kickoff procedure for a project's research phase. Owns workspace-context definition, the live MCP scan, and the research-method offer. Loaded at kickoff when the plan opens with research ([`project-flow.md`](project-flow.md)). Deliverable shape and readiness criteria belong to [`library/deliverables/research-artifact/template.md`](../domains/marketing/deliverables/research-artifact/template.md).

## Workspace-Context Definition

Connected workspace context means forward-looking work signals, not every connected app. Use productivity/work surfaces such as email, calendar, drive/docs/slides/sheets, task systems, or similar operator workspaces. Exclude social/content platforms, public profile surfaces, current or completed project deliverables, and historic project performance unless the operator explicitly names one as a reference. Those are retrospective content signals, not forward-looking project inputs. Public web search is for evidence-checking or trend validation after a candidate direction exists; it is not a substitute for workspace signal.

Check the connected MCP servers for approved workspace-context tools; do not maintain a local connector list. If the live surface returns only social/content tools or no useful workspace context, say so before falling back to local positioning or operator-provided ideas.

## Live MCP Scan Procedure

When the operator starts a new project:

1. Check the connected MCP servers for approved workspace-context tools. If useful context is available, scan only approved lightweight metadata first (for example: message subjects, event titles, doc titles, timestamps, and folder names; not bodies or full document contents unless the operator approves). Ignore social/content tool matches for project idea sourcing. If the surface is unavailable or not useful, say so before falling back to local positioning or operator-provided ideas.
2. Build a short candidate list with the operator and cite provenance for each direction. Save it as `phase-1-research/idea-bank.md` unless the plan names another path: per candidate a title, a 1-3 sentence thesis, and one provenance line, then the selected pick in one line.
3. Let the operator pick the top direction for deeper research.

## Research Method Offer

After a direction is selected, offer the operator a choice:

- **Gemini web handoff prompt.** Produce a paste-ready prompt for the operator to run in a Gemini web session, then ingest the returned material.
- **Native deep research** via [`.claude/skills/deep-research/SKILL.md`](../../.claude/skills/deep-research/SKILL.md). Runs on the agent's own tools — no research API cost, but heavy token usage (a specialist wave of searches and fetches that counts against session/subscription limits). The skill states execution mode, tier, and specialist model for acknowledgement before launching; it injects the research deliverable template's Output Shape per its Output contract.
- **Manual sources.** Operator-provided files, transcripts, or pasted text when no external research path applies.

The selected method is recorded in `research-artifact-v{N}.md` frontmatter as `research_method`.

## Prompt Composition (Gemini web handoff)

When composing the Gemini web-handoff prompt, the agent loads [`library/deliverables/research-artifact/template.md`](../domains/marketing/deliverables/research-artifact/template.md) "Output Shape" and injects that section block into the prompt as the required return structure. The deliverable template is the single source of truth for shape; the prompt does not paraphrase, restate, or partially copy the section list. If the template's Output Shape changes, the next prompt automatically reflects the new shape.

Operator research goals and the selected project direction go in their own block above the injected return structure. Save the composed prompt at `phase-1-research/source-material/gemini-deep-research-prompt.md` so future runs are traceable.

## Fallback Rule

If no connected MCP server is available, the servers expose only social/content tools, or otherwise produces no useful workspace context, name the gap explicitly to the operator and fall back to local positioning, the operator profile, or operator-provided ideas. Do not silently skip the scan and do not invent provenance.
