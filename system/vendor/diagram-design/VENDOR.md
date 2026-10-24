## Diagram Design Skill Vendor Record

- Upstream repository: `https://github.com/cathrynlavery/diagram-design`
- Upstream version: `main` @ `0b9b10cab3dbf695642fa1a6c4b4f60df9083689` (pin the commit hash, not the moving branch)
- Snapshot date (UTC): `2026-10-24`
- Skill version at snapshot: `2.6` (plugin manifest `2.6.120`)
- Source locations in AgentFrame: `system/vendor/diagram-design/` (upstream `skills/diagram-design/`)
- Additionally vendored: upstream root `scripts/verify-geometry.py` -> `scripts/verify_geometry.py`. Upstream ships this as a contribution gate outside the skill, so an ordinary install gets prose taste-gating only. AgentFrame runs it against generated output, which is the one deterministic check on hand-placed geometry.
- Excluded from the snapshot: everything outside `skills/diagram-design/` except the files named above. That covers upstream `docs/`, `commands/`, `prompts/`, root `assets/`, the plugin manifests (`.claude-plugin/`, `.codex-plugin/`, `.factory-plugin/`, `.qoder-plugin/`, `.agents/`), the icon build tooling under `scripts/vendor/`, and every other root `scripts/` gate. AgentFrame routes through `library/process/diagram-production.md` rather than the plugin's slash commands, and vendors no icon build step.
- License: MIT - see `LICENSE`. Bundled icon artwork carries its own notices in `THIRD_PARTY_LICENSES.md`.

### Purpose

Track the upstream source and refresh procedure for the vendored Diagram Design skill. Vendor files own the 45 visual types, the design system, and the taste gate. AgentFrame-specific routing, the PPT Master seam, and design-language brand projection live outside this directory.

### AgentFrame overlay files

Two files in this directory are AgentFrame-owned and must survive a refresh:

| File | Owns |
|---|---|
| `VENDOR.md` | This record |
| `scripts/verify_geometry.py` | Vendored from the upstream repo root, with `ASSET_DIR` repointed at this skill's own `assets/` so `--all` resolves inside the vendored layout |

### Integration boundary

The skill emits self-contained HTML whose SVG carries CSS custom properties, `class` attributes, and percentage geometry. PPT Master's converter accepts none of those. `system/tools/diagram_flatten.py` owns that translation and is AgentFrame-owned; never patch vendor output conventions to satisfy the converter.

Brand comes from the upstream profile mechanism (`~/.diagram-design/profiles/<slug>.md` plus a `.diagram-design` project marker). AgentFrame treats that home-directory library as a derived cache generated from `library/assets/design-languages/<name>/tokens.yaml`; the repo is truth. See `system/tools/diagram_profile.py`, which rewrites `references/style-guide.md` by pattern: the semantic-roles table, the typography table, the `### Font stack` section up to the next `---`, the typography constraint line, the `backend` node row, and the `Brand palette source` and pre-baked-example notes.

### Refresh Procedure

1. Clone upstream to a temporary directory and check out the exact commit being adopted.
2. Preserve the AgentFrame overlay files listed above, then remove `system/vendor/diagram-design/`.
3. Copy upstream `skills/diagram-design/` into `system/vendor/diagram-design/`.
4. Copy the upstream root `LICENSE` and `THIRD_PARTY_LICENSES.md` into `system/vendor/diagram-design/`.
5. Copy upstream `scripts/verify-geometry.py` to `scripts/verify_geometry.py` and repoint `ASSET_DIR` to `ROOT / "assets"`.
6. Restore `VENDOR.md`. Remove the temporary clone directory.
7. Verify: `python system/vendor/diagram-design/scripts/verify_geometry.py --all`, then `python -m pytest system/tests/test_diagram_flatten.py system/tests/test_diagram_profile.py -q -p no:cacheprovider`. A shipped-asset geometry finding means upstream changed a template convention; a flatten test failure means upstream changed an SVG emission convention and `diagram_flatten.py` needs the new case; a profile test failure means upstream restructured `references/style-guide.md` and `diagram_profile.py` needs the new pattern.
8. Recount the native-shape roster in `library/process/diagram-production.md`: flatten every `assets/*.html` except `index.html` with `--no-background --match-markers`, run `system/vendor/ppt-master/scripts/svg_quality_checker.py` on each output's folder, and count `blocking: 0` with exit 0.

### Known upstream notes

- `references/style-guide.md` records that the pre-baked `assets/example-*.html` files were built under an earlier skin and are scheduled for regeneration upstream. They are exemplars for layout grammar, not for current token values.
- Vendored references cite repository gates that are not vendored (`scripts/verify-contrast.py`, `lint-skin.py`, `lint-render.py`, and the per-type `verify-*.py`). Inside AgentFrame only `scripts/self_check.py` and `scripts/verify_geometry.py` run.
- PNG export (`references/export.md`, `scripts/export_png.py`) requires Playwright, which AgentFrame does not install. The deck route never needs it, because `diagram_flatten.py` produces native shapes rather than a raster.
