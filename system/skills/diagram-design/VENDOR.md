## Diagram Design Skill Vendor Record

- Upstream repository: `https://github.com/cathrynlavery/diagram-design`
- Upstream version: `main` @ `ac490fd1ac4b4014100f93e729cb4ad198700bd4` (pin the commit hash, not the moving branch)
- Snapshot date (UTC): `2026-08-27`
- Skill version at snapshot: `2.6`
- Source locations in AgentFrame: `system/skills/diagram-design/` (upstream `skills/diagram-design/`)
- Additionally vendored: upstream root `scripts/verify-geometry.py` -> `scripts/verify_geometry.py`. Upstream ships this as a contribution gate outside the skill, so an ordinary install gets prose taste-gating only. AgentFrame runs it against generated output, which is the one deterministic check on hand-placed geometry.
- Excluded from the snapshot: upstream `docs/`, `commands/`, `prompts/`, `.claude-plugin/`, and the icon build tooling under `scripts/vendor/`. AgentFrame routes through `library/process/diagram-production.md` rather than the plugin's slash commands, and vendors no icon build step.
- License: MIT - see `LICENSE`

### Purpose

Track the upstream source and refresh procedure for the vendored Diagram Design skill. Vendor files own the 39 layout grammars, the design system, and the taste gate. AgentFrame-specific routing, the PPT Master seam, and design-language brand projection live outside this directory.

### AgentFrame overlay files

Two files in this directory are AgentFrame-owned and must survive a refresh:

| File | Owns |
|---|---|
| `VENDOR.md` | This record |
| `scripts/verify_geometry.py` | Vendored from the upstream repo root, with `ASSET_DIR` repointed at this skill's own `assets/` so `--all` resolves inside the vendored layout |

### Integration boundary

The skill emits self-contained HTML whose SVG carries CSS custom properties, `class` attributes, and percentage geometry. PPT Master's converter accepts none of those. `system/tools/diagram_flatten.py` owns that translation and is AgentFrame-owned; never patch vendor output conventions to satisfy the converter.

Brand comes from the upstream profile mechanism (`~/.diagram-design/profiles/<slug>.md` plus a `.diagram-design` project marker). AgentFrame treats that home-directory library as a derived cache generated from `library/assets/design-languages/<name>/tokens.yaml`; the repo is truth. See `system/tools/diagram_profile.py`.

### Refresh Procedure

1. Clone upstream to a temporary directory and check out the exact commit being adopted.
2. Preserve the AgentFrame overlay files listed above, then remove `system/skills/diagram-design/`.
3. Copy upstream `skills/diagram-design/` into `system/skills/diagram-design/`.
4. Copy the upstream root `LICENSE` to `system/skills/diagram-design/LICENSE`.
5. Copy upstream `scripts/verify-geometry.py` to `scripts/verify_geometry.py` and repoint `ASSET_DIR` to `ROOT / "assets"`.
6. Restore `VENDOR.md`. Remove the temporary clone directory.
7. Verify: `python system/skills/diagram-design/scripts/verify_geometry.py --all` and `python -m unittest system.tests.test_diagram_flatten`. A shipped-asset geometry finding means upstream changed a template convention; a flatten test failure means upstream changed an SVG emission convention and `diagram_flatten.py` needs the new case.
8. Append a `vendor_update` audit row.

### Known upstream notes

- `references/style-guide.md` records that the pre-baked `assets/example-*.html` files were built under an earlier skin and are scheduled for regeneration upstream. They are exemplars for layout grammar, not for current token values.
- PNG export (`references/export.md`) requires Playwright, which AgentFrame does not install. The deck route never needs it, because `diagram_flatten.py` produces native shapes rather than a raster.
