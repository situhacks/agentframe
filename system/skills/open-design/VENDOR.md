## Open Design Vendor Record

- Upstream repository: `https://github.com/nexu-io/open-design`
- Vendored commit: `5bd9763181f2c868e1439bfc4842859ec69df102`
- Snapshot date (UTC): `2026-05-11`
- Source location in AgentFrame: `system/skills/open-design/source/`
- License: Apache-2.0 (preserved at `system/skills/open-design/source/LICENSE`)

### Purpose

Track exactly what was vendored and what was excluded so future re-vendoring can repeat the same cuts with minimal decision churn.

### Applied Exclusions (non-feature bloat only)

The following were removed from the vendored copy after import:

- Upstream contributor/maintainer surfaces:
  - `.github/`
  - `.vaunt/`
  - `CONTRIBUTING*.md`
  - `MAINTAINERS*.md`
  - `TRANSLATIONS.md`
- Non-English documentation variants (English originals preserved):
  - `README.*.md` except `README.md`
  - `QUICKSTART.*.md` except `QUICKSTART.md`
- Upstream collateral artifact:
  - `edited_image.png`

### Re-vendor Procedure

1. Clone upstream to a temporary directory at the target commit.
2. Remove `system/skills/open-design/source/` entirely (this destroys local `node_modules/`, `.od/` runtime data, and Electron caches; they regenerate on next install).
3. Copy the upstream snapshot into `system/skills/open-design/source/`.
4. Re-apply the exclusion list above.
5. Update this file with the new commit hash and date.
6. Verify `LICENSE` remains present in the vendored subtree.
