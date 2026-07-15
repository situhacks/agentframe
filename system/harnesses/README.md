# Native Harness Projections

## Purpose

Expose selected canonical AgentFrame skills through each coding agent's project-native discovery directory without maintaining three editable copies or requiring Windows symlink privileges.

## Ownership

- `system/skills/<skill>/` is the only editable skill source.
- [`manifest.json`](manifest.json) selects projected bundles, target directories, and optional harness-specific overlays.
- `.claude/skills/`, `.agents/skills/`, and `.cursor/skills/` are generated views. Their metadata records source paths and content hashes.
- Harness-specific content, when earned, lives at the overlay path declared in the manifest. Never patch it directly into a generated directory.

## Commands

```text
python system/af.py sync-harnesses --write
python system/af.py sync-harnesses --check
```

`--write` builds every projection in a temporary tree, refuses to overwrite a same-named foreign skill, then replaces only AgentFrame-managed bundles. `--check` performs the same deterministic build and reports missing, extra, or changed generated files without repairing them.

Run `--write` after changing a selected canonical skill or the projection manifest. Run `--check` in verification and CI.

## Discovery And Fallback

Native discovery reads each projected `SKILL.md` frontmatter without eagerly loading its body. The description must name both its positive trigger and meaningful near-miss boundary. Once selected, the complete projected bundle supplies the same local references as its canonical source.

Harnesses that do not support these native directories still work through the stable `AGENTS.md` classifier, its task-local router, and the `system/skills/README.md` catalog. That file-only route is the compatibility floor; native projections improve discovery but do not own AgentFrame behavior.
