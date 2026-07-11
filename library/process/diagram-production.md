# Diagram Production

## Purpose

Produce static graph-shaped explainers through the [D2 diagram skill](../../system/skills/d2-diagrams/SKILL.md): flowcharts, decision trees, system maps, and other visuals whose meaning is nodes plus directional relationships. The `.d2` file is the durable source; SVG is the derived asset.

## When To Load

Load when a project or deliverable needs to explain a process, dependency, decision, handoff, or system as connected nodes and edges. Do not load for a timeline, Gantt, sequence diagram, chart, or decorative visual that is not primarily a graph.

## Procedure

1. Place `diagram.d2` beside the visual artifact or deliverable that owns it. Load the D2 skill before authoring the source.
2. Write the graph semantics first: concise labels, meaningful directed edges, visible decisions, and containers for real groups. Do not hand-author SVG, node geometry, or bespoke HTML.
3. Render SVG. If a downstream deliverable truly needs PNG, PDF, or animation, use that deliverable's existing export or video route from the approved SVG; do not add an export runtime pre-emptively.
4. Render with `system/tools/d2/render.ps1`, review the derived asset, and revise the `.d2` source until the hierarchy and labels are legible.
5. For a deck, use the approved SVG or PNG in the deck's existing production route. Keep the `.d2` source beside the artifact for future agent revisions.

For a simple existing Mermaid flowchart, translate its nodes and edges into `.d2`. Mermaid may remain as a compact reference, but `.d2` is canonical when the output needs D2 styling or animation.

## Verification Or Logging

- Confirm the SVG output exists and is non-empty.
- Preview the derived asset: labels must be readable, every intended relationship must be visible, and the visual hierarchy must survive at its intended size.
- Record the `.d2` and derived asset paths in the parent deliverable when it tracks visual artifacts.

## Boundaries

- Diagram Production owns agent-rendered graph visuals, not all visual production.
- It does not replace Mermaid's non-graph diagram families, image generation, deck production, or a project-specific interactive application.
- D2 updates follow [`system/tools/d2/VENDOR.md`](../../system/tools/d2/VENDOR.md); do not create a human-operated diagram editor.
