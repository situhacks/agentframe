---
name: d2-diagrams
description: Render an auto-laid-out graph as SVG with the vendored D2 CLI. Use only when a graph is regenerated from changing source and its layout must re-solve itself — a derived topology or a churning dependency map. Not the default diagram route; `diagram-design` owns anything a human audience sees. Routed by `library/process/diagram-production.md`.
---

# D2 Diagrams

Write one `.d2` source file beside the deliverable artifact it explains. The source is the only editable form; SVG is the derived asset.

D2's value here is that a layout engine — not the agent — places the nodes, so a regenerated graph re-solves its own layout and can never produce overlapping geometry. When the diagram is something an audience reads rather than a derived artifact, stop and use [`diagram-design`](../diagram-design/SKILL.md) instead: it carries the project's brand and is the only route that reaches native PowerPoint shapes.

1. Model the message, not the implementation: name the few concepts the audience must understand and draw only meaningful relationships.
2. Use left-to-right flow by default. Keep labels short, distinguish decisions with diamond shapes, and group related systems with containers rather than crossing lines.
3. Render SVG only. If a real downstream deliverable requires PNG, PDF, or animation, hand the approved SVG to that deliverable's existing export or video route rather than adding a diagram-export runtime.
4. Render with:

```powershell
powershell -ExecutionPolicy Bypass -File system/tools/d2/render.ps1 `
  -InputPath <path-to-diagram.d2> -OutputPath <path-to-diagram.svg>
```

5. Confirm the output exists and is non-empty. For a visible deliverable, preview the derived asset and correct clipped labels, unreadable hierarchy, or unnecessary complexity before handoff.

Do not create a browser app, React component, manual canvas, or every possible export. For a deck, render SVG or PNG and use the deck's normal production route to place it. D2 updates are owned by [`system/tools/d2/VENDOR.md`](../../tools/d2/VENDOR.md).
