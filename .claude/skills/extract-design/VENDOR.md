# Extract Design vendor record

- Upstream artifact: `designlang@13.3.2` from npm
- Package source ref: `49f138715716a02d6fb6ba85c66df82fdda1d894`
- Tarball SHA-1: `3a56607efcf0c61aeada8d1d7d0b0cd9c3616c2c`
- Tarball integrity: `sha512-gMMZYvFSl5bIqj/2Xp8SIYPeDTsnsa+ur75bQBeS5jKsHE9+VNWNJuAWImSRYrAPgvzGA1qhw7sz56lG8vHjRw==`
- Upstream path: `skills/extract-design/SKILL.md`
- Snapshot date (UTC): `2026-10-23`
- License: MIT (`LICENSE.txt`)

## Scope

`SKILL.md` and `LICENSE.txt` mirror the published npm artifact, except the `disable-model-invocation: true` frontmatter line, which is AgentFrame's. `AGENTS.md` is the AgentFrame integration boundary and pins the CLI version agents invoke.

## Refresh

Download the pinned package tarball, copy `skills/extract-design/SKILL.md` and `LICENSE`, re-add `disable-model-invocation: true` to the `SKILL.md` frontmatter, restore this record and `AGENTS.md`, set the `npx designlang@<version>` pin in `AGENTS.md`, then verify the recorded integrity value. Smoke: `npx designlang@<version> https://example.com --out <scratch dir>` exits 0 and writes `*-design-language.md`, `*-design-tokens.json`, and `*-variables.css`.
