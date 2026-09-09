# Native Harness Surfaces

## Purpose

Expose selected canonical AgentFrame skills and deterministic guards through each coding agent's project-native surfaces without maintaining editable copies of shared logic.

## Ownership

- `system/skills/<skill>/` is the only editable skill source.
- [`manifest.json`](manifest.json) selects projected bundles, target directories, and optional harness-specific overlays.
- `.claude/skills/`, `.agents/skills/`, and `.cursor/skills/` are generated views. Their metadata records source paths and content hashes.
- Harness-specific content, when earned, lives at the overlay path declared in the manifest. Never patch it directly into a generated directory.
- `system/hooks/` is the only editable guard-logic source. `.claude/settings.json`, `.cursor/hooks.json`, and `.codex/hooks.json` contain only native wiring.
- Other files inside `.claude/`, `.cursor/`, and `.codex/` are operator-local and ignored. Do not copy personal plans or settings between harnesses.

## Priority Harnesses

| Priority | Harness surface | Contract |
|---|---|---|
| 1 | Claude in VS Code / Claude Code | `.claude/settings.json` is the reference wiring. The VS Code extension and CLI share Claude Code settings. |
| 2 | Cursor | `.cursor/hooks.json` provides native project hooks, so guard coverage does not depend on the optional Claude-compatibility toggle. Native commands pass `--cursor-native`; if the toggle also imports `.claude/settings.json`, those imported twins no-op instead of running each guard twice. |
| 3 | Codex | `.codex/hooks.json` provides trusted-project hooks. Codex edits arrive as `apply_patch`, so `version_guard.py` extracts every patch target before applying the shared rule. Review changed project hooks when Codex asks for trust. |

`publish_guard.py` is wired on Claude and Cursor only: it matches an MCP tool name, and the Substack server is not mounted on the Codex surface. Coverage there is `af doctor`'s state-truth check, which is the backstop everywhere anyway, since a push made from a vendor's web UI is invisible to every hook.

`voice_guard.py` is wired on all three surfaces around two state buttons: it runs `system/voice_lint.py`, denies `af ready` on a user-voiced head with hard findings (banned tics, two dashes in a sentence, an operator hyphen turned into an em dash since the previous version), and after `af version` hands the agent the lint of the version it just closed with the reminder to load the voice system before editing. `AF_VOICE_LINT=skip` on the command is the operator override.

`index_refresh.py` runs at session start on all three surfaces: when the retrieval index is more than a day old it spawns `af index update` detached, prints one line, and returns. It never blocks, never fails the session, and never builds from cold; a machine with no index still needs one explicit `af index update --rebuild`.

`.agents/skills/` remains the portable skill-discovery projection. It is not a fourth hook configuration.

## Commands

```text
python system/af.py sync-harnesses --write
python system/af.py sync-harnesses --check
```

`--write` builds every projection in a temporary tree, refuses to overwrite a same-named foreign skill, then replaces only AgentFrame-managed bundles. `--check` performs the same deterministic build and reports missing, extra, or changed generated files without repairing them.

Run `--write` after changing a selected canonical skill or the projection manifest. Run `--check` in verification and CI.

Run the hook smoke matrix after changing shared guard logic or any native hook file:

```text
python -m unittest system.tests.test_harness_hooks system.tests.test_version_guard system.tests.test_ppt_master_guard system.tests.test_publish_guard system.tests.test_voice_guard system.tests.test_voice_lint
```

## Discovery And Fallback

Native discovery reads each projected `SKILL.md` frontmatter without eagerly loading its body. The description must name both its positive trigger and meaningful near-miss boundary. Once selected, the complete projected bundle supplies the same local references as its canonical source.

Harnesses that do not support these native directories still work through the stable `AGENTS.md` classifier, its task-local router, and the `system/skills/README.md` catalog. That file-only route is the compatibility floor; native projections improve discovery but do not own AgentFrame behavior.

Hook coverage is a guardrail, not the source of truth. `system/af.py` owns state transitions, and `python system/af.py doctor` remains the cross-harness backstop.

## Native References

- Claude Code hooks: https://code.claude.com/docs/en/hooks
- Claude Code VS Code settings: https://code.claude.com/docs/en/ide-integrations
- Cursor hooks: https://cursor.com/docs/hooks
- Cursor Claude-hook compatibility: https://cursor.com/docs/reference/third-party-hooks
- Codex hooks: https://learn.chatgpt.com/docs/hooks

## Antigravity

Antigravity (IDE 1.20.5+ and the `agy` CLI) reads the root `AGENTS.md` natively and discovers skills in `.agents/skills/`, which is the same projection the Codex target writes, so an Antigravity session enters through the stable classifier and its task-local router with no extra wiring.

In the other direction, `system/tools/agy_call.py` is the delegation connector: a bounded task with media or a long multimodal input runs in `agy` print mode on the operator's Google account, fenced to a disposable read-only workspace with shell commands denied. Processes route perception through it and keep judgment in the session; see `library/process/media-intake.md` and `short-form-edit.md`. Pins and measured traps: `system/tools/agy_call.VENDOR.md`.

