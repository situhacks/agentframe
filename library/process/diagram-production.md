# Diagram Production

## Purpose

Produce a diagram — a visual whose meaning is structure rather than prose or a table. Two renderers serve different jobs, and the destination picks between them.

## When To Load

Load when a project or deliverable needs to explain a system, process, decision, hierarchy, comparison, or relationship as a structured visual. Do not load for a photograph, an illustration, or a decorative visual.

## Route

| Route | Use when | Skill | Output |
|---|---|---|---|
| **Diagram Design** (default) | A human audience sees this — a deck, a client document, a post, a README figure. Thirty-nine layout grammars, brand-tokenized. | [`system/skills/diagram-design/SKILL.md`](../../system/skills/diagram-design/SKILL.md) | Self-contained `.html`; a flattened `.svg` when it goes into a deck |
| **D2** | A graph is regenerated from changing source and nobody reads it as a designed artifact — a derived topology, a dependency map that churns. Auto-layout re-solves placement on every change. | [`system/skills/d2-diagrams/SKILL.md`](../../system/skills/d2-diagrams/SKILL.md) | `diagram.d2` source plus derived SVG |

Diagram Design is the broader tool in both directions: more layout grammars than D2's graphs-only, and the only route that reaches native PowerPoint shapes. Choose D2 only for the churn case above. When a D2 diagram graduates to something an audience sees, redraw it through Diagram Design rather than restyling the `.d2`.

## Procedure

1. Place the diagram beside the deliverable or visual artifact that owns it. Load the skill before authoring; it owns type selection, the design system, and its own taste gate.
2. Set the four output dials — format, size, detail, audience — **before** drawing. Retrofitting a size preset means redrawing, because the preset sets both the `viewBox` and the type ramp. A deck slide is `slide-16x9`, not the `doc-inline` default.
3. Author the HTML. The `.html` is the durable source; every other form is derived from it.
4. Run the two checks the skill ships with:

```powershell
python system/skills/diagram-design/scripts/self_check.py <diagram.html>
python system/skills/diagram-design/scripts/verify_geometry.py <diagram.html>
```

`self_check.py` covers the accessible-figure contract and single-file safety. `verify_geometry.py` catches a label mask clipped by a node painted after it — the defect hand-placed geometry actually produces. Upstream ships it as a contribution gate outside the skill; AgentFrame vendors it because generated output has the same failure mode.

5. Look at the rendered diagram before handing it off. Neither check reads overlap, crowding, or a hierarchy that collapses at final size.

For a deck, continue to the handoff below. For an existing Mermaid or draw.io source, the skill's import route redraws it — it extracts structure and redraws, never converts, and reports what it merged or dropped.

## Deck handoff — native shapes, not a raster

PPT Master authors slides as SVG and compiles them to native DrawingML, so a diagram placed this way becomes **editable PowerPoint shapes** rather than a pasted image. Diagram Design output cannot go in directly: its SVG carries CSS custom properties, `class` attributes, and percentage geometry, none of which the converter reads.

`system/tools/diagram_flatten.py` owns that translation. It changes how a value is expressed, never what the diagram shows.

```powershell
python system/tools/diagram_flatten.py <diagram.html> -o <diagram.svg> `
  --no-background --match-markers
```

- `--no-background` drops the diagram's own full-canvas ground. The slide's design language already owns the background; keeping it paints a pale plate over a dark deck.
- `--match-markers` repaints line-end markers to their line's colour, cloning a marker shared by lines of different colours. **This changes appearance** — a hollow arrowhead becomes solid — because a PowerPoint line-end cannot carry a fill independent of its line. Omit it and those diagrams stay blocked at the gate.

Then verify before authoring the slide around it:

```powershell
python system/skills/ppt-master/scripts/svg_quality_checker.py <folder-holding-the-svg>
```

`blocking: 0` is the bar. Warnings about root-level `<g>` bounds and ids are an artifact of checking a diagram as if it were a whole page; they resolve once it is a child of the slide.

### What reaches native shapes

Checked across the vendored roster: 128 of 148 shipped diagrams flatten to a clean gate. Six types carry a constraint the translation cannot remove, because PowerPoint has no equivalent:

| Type | Blocked by | Route instead |
|---|---|---|
| Venn, Medallion | `clipPath` on a shape | Raster, or redraw with overlapping translucent fills |
| UML class, Sequence (OAuth variant) | Independently filled arrowheads; `<polyline>` line-ends | `--match-markers`, or raster if the hollow arrowhead is load-bearing |
| Fishbone | `viewBox` origin is not `0 0` | Raster |
| IT current-state | Matrix transform over a rounded-rect subtree | Raster |
| Polar, Quadrant (consultant variant) | `dy` / `dominant-baseline` on `<text>` | Raster |

Raster fallback needs Playwright (`pip install playwright && playwright install chromium`), which AgentFrame does not install by default; the skill's `references/export.md` owns that procedure. Every other route avoids it.

### Charts inside a deck

Diagram Design ships bar, line, scatter, radar, treemap, Sankey, and Gantt. Inside a deck, do not use them. PPT Master's Chart executor produces native PowerPoint chart objects with real underlying data, which outlives an SVG the moment someone needs to change a number. Diagram Design owns conceptual and relational types for deck work; its chart types are for documents, posts, and social cards.

## Brand

A diagram in a branded deliverable must carry that identity, or it reads as a stranger's diagram pasted in. Diagram Design resolves brand from a client profile; AgentFrame generates that profile from the design language rather than maintaining the values twice.

```powershell
python system/tools/diagram_profile.py <design-language> --marker <project-root>
```

This writes `~/.diagram-design/profiles/<design-language>.md` and a `.diagram-design` marker in the project, after which every diagram authored in that project resolves the language automatically. The profile library is a **derived cache** — same status as `system/index/`: `library/assets/design-languages/<name>/tokens.yaml` is truth, and the profile is regenerated, never hand-edited. Re-run after the language's tokens change.

Without it the skill's first-run gate offers to onboard from a website URL. Take that path only for an identity AgentFrame does not already carry as a design language.

## Verification Or Logging

- Confirm the derived asset exists and is non-empty.
- Confirm both skill checks pass, and for a deck, `blocking: 0` from the SVG checker.
- Inspect the rendered result at its intended size. A passing check is not a legible diagram.
- Record the `.html` source and derived asset paths in the parent deliverable when it tracks visual artifacts.

## Boundaries

- This process owns agent-authored structural visuals. It does not own image generation, deck production, or a project-specific interactive application.
- Vendor updates follow [`system/skills/diagram-design/VENDOR.md`](../../system/skills/diagram-design/VENDOR.md) and [`system/tools/d2/VENDOR.md`](../../system/tools/d2/VENDOR.md). Do not patch vendor output conventions to satisfy the PPTX converter; `diagram_flatten.py` owns that boundary.
- Do not build a human-operated diagram editor.
