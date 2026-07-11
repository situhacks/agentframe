---
name: d2-diagrams
description: Create polished static flowcharts, decision trees, system maps, and process explainers as SVG with the vendored D2 CLI. Use when the agent—not a human editor—should create and revise a graph-shaped visual for a deliverable.
---

# D2 Diagrams

Write one `.d2` source file beside the deliverable artifact it explains. The source is the only editable form; SVG is the derived asset.

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
