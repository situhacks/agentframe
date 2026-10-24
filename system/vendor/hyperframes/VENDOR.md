# HyperFrames vendor record

- Upstream: `https://github.com/heygen-com/hyperframes` (GitHub releases)
- Source ref: `cf7266d8158655270b10fc247b82ac9cbf5de99f` (tag `v0.8.191`, released 2026-10-24)
- Snapshot date (UTC): `2026-10-24`
- Source location: `system/vendor/hyperframes/source/`
- License: Apache-2.0 (`source/LICENSE`)

## Scope

The upstream repository at the source ref, excluding:

- `.git/`
- `registry/`: block and component sources. `npx hyperframes add` and `catalog` fetch them from `main/registry` on GitHub.
- `docs/public/`: the docs site's generated catalog assets.
- `packages/producer/tests/`: render regression fixtures, mostly Git LFS media.
- `packages/studio-server/src/history/__fixtures__/`: test fixtures whose paths exceed Windows MAX_PATH in a `.claude/worktrees/` checkout.
- `.claude/skills/changelog-video/assets/*.mp4|*.mp3` and the same files under `.agents/`: Git LFS media for an upstream-maintainer skill.

No kept file is a Git LFS pointer.

The skill library is reached through the upstream router `source/skills/hyperframes/SKILL.md`: `hyperframes` (render contract, work loop, references), `hyperframes-cli`, `hyperframes-registry`, `media-use`, and the on-demand `slideshow`, `figma` and `remotion-to-hyperframes`.

AgentFrame runs the CLI from npm (`npx hyperframes`, same version as the source ref) and runs `source/skills/media-use/scripts/transcript-cut.mjs` and `audio-duck.mjs` directly with Node.

## Refresh

1. Read the latest tag from `https://api.github.com/repos/heygen-com/hyperframes/releases/latest`.
2. Clone that tag shallow with `GIT_LFS_SKIP_SMUDGE=1` and `-c core.longpaths=true`.
3. Replace `source/` with the clone's tracked files minus the exclusions above (`git ls-files -z` piped through `tar`), then restore this record and the AgentFrame entry `SKILL.md`.
4. Confirm no kept file is a Git LFS pointer and no path under `source/` is longer than 165 characters.
5. Smoke test: `source/LICENSE`, `source/packages/studio/`, `source/skills/hyperframes/SKILL.md`, and `source/skills/media-use/scripts/transcript-cut.mjs` and `audio-duck.mjs` exist; `transcript-cut.mjs --input <clip> --transcript <json> --cut-silence 0.8 --plan` prints kept segments; `npx hyperframes@<version> --version` prints the pinned version.
6. Stage with `git add -f system/vendor/hyperframes`. The root `docs/` rule and the upstream `source/.gitignore` (`compositions/`, `packages/core/src/fonts/fontData.generated.ts`) otherwise drop upstream files.
