# Research And Signals

Shared kickoff procedure for any campaign flow's research phase. Owns workspace-context definition, the live Composio/Rube MCP scan, and the research-method offer. Loaded from each flow's Phase 1 (`marketing-solo-flow.md` Phase `1-setup-research`, `marketing-standard-flow.md` Phase 1 - Research). Deliverable shape and lock criteria belong to the calling flow and to [`library/deliverables/research-artifact/template.md`](../deliverables/research-artifact/template.md).

## Workspace-Context Definition

Connected workspace context means forward-looking work signals, not every connected app. Use productivity/work surfaces such as email, calendar, drive/docs/slides/sheets, task systems, or similar operator workspaces. Exclude social/content platforms, public profile surfaces, current or completed campaign deliverables, and historic campaign performance unless the operator explicitly names one as a reference. Those are retrospective content signals, not forward-looking campaign inputs. Public web search is for evidence-checking or trend validation after a candidate direction exists; it is not a substitute for workspace signal.

Ask the live Composio/Rube surface what approved workspace-context tools are connected; do not maintain a local connector list. If the live surface returns only social/content tools or no useful workspace context, say so before falling back to local positioning or operator-provided ideas.

## Live MCP Scan Procedure

When the operator starts a new campaign:

1. Ask the live Composio/Rube MCP surface what approved workspace-context tools are connected. If useful context is available, scan only approved lightweight metadata first (for example: message subjects, event titles, doc titles, timestamps, and folder names; not bodies or full document contents unless the operator approves). Ignore social/content tool matches for campaign idea sourcing. If the surface is unavailable or not useful, say so before falling back to local positioning or operator-provided ideas.
2. Build a short candidate list with the operator and cite provenance for each direction. The calling flow owns where this list is saved (typically `phase-1-research/idea-bank.md`) and the per-candidate shape.
3. Let the operator pick the top direction for deeper research.

## Research Method Offer

After a direction is selected, offer the operator a choice:

- **Gemini Deep Research API** via `system/research/gemini_deep_research.py`. Label the option so it notes that it can hit Gemini API costs; if the operator explicitly selects this option, treat the selection as cost acknowledgement. The helper preserves Gemini's native interaction JSON under `source-material/` and extracts the Markdown handoff deterministically. If Gemini changes response shape, preserve the raw JSON and surface the extraction failure instead of guessing. If polling fails mid-run, the interaction ID is preserved at `phase-1-research/source-material/gemini-deep-research-interaction-id.txt`; resume with `python system/research/gemini_deep_research.py --resume-from-id <id> [other flags]`.
- **Gemini web handoff prompt.** Produce a paste-ready prompt for the operator to run in a Gemini web session, then ingest the returned material.
- **Manual sources.** Operator-provided files, transcripts, or pasted text when neither Gemini path applies.

The selected method is recorded in `research-artifact-v{N}.md` frontmatter as `research_method`.

## Prompt Composition (Gemini DR API and web-handoff)

When composing the Gemini DR prompt for either the API path or the web-handoff path, the agent loads [`library/deliverables/research-artifact/template.md`](../deliverables/research-artifact/template.md) "Output Shape" and injects that section block into the prompt as the required return structure. The deliverable template is the single source of truth for shape; the prompt does not paraphrase, restate, or partially copy the section list. If the template's Output Shape changes, the next prompt automatically reflects the new shape.

Operator research goals and the selected campaign direction go in their own block above the injected return structure. Save the composed prompt at `phase-1-research/source-material/gemini-deep-research-prompt.md` so future runs are traceable.

## Fallback Rule

If the Composio/Rube surface is unavailable, returns only social/content tools, or otherwise produces no useful workspace context, name the gap explicitly to the operator and fall back to local positioning, the operator profile, or operator-provided ideas. Do not silently skip the scan and do not invent provenance.
