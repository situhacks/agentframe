# Extract-Design — AgentFrame Boundary Notes

Vendored thin skill (see `VENDOR.md`); `SKILL.md` documents the upstream CLI. These notes govern how it runs inside AgentFrame.

- **Triggered from the chain, not by name.** The design-language template's Authoring step owns the trigger: when the operator drops a URL as design inspo, run the extraction — no operator incantation needed. Announce the run ("extracting tokens from {url}"); the first-ever run pulls the CLI + headless Chromium via npx, say so when it's slow.
- **Output location.** Run with `--out` pointed at the campaign's design-language folder: `phase-{n}/design-language/source-material/extract-{site}/`. Extraction output is research input, never the deliverable.
- **Distillation, not adoption.** The agent reads `*-design-language.md` + `*-design-tokens.json` and distills into OUR lean `design-language-v{N}.md` shape (palette table, type system, treatment block). Do not paste the upstream 19-section guide in as the artifact.

| Upstream output | Feeds |
|---|---|
| `*-variables.css` | Basis for `tokens.css` (already `:root {}`-shaped; rename variables to our token names) |
| `*-design-tokens.json` / `*-design-language.md` | Source material for the palette/type/spacing sections and the treatment block |
| Motion / component / brand-voice data | Only relevant when transferring to Open Design (its schema wants Motion + Components + Voice); our DL template omits them by default |
| `*-tailwind.config.js`, `*-theme.js`, `*-shadcn-theme.css`, `*-figma-variables.json` | Unused in AgentFrame — don't copy them into the campaign |

- **Dark variants.** Add `--dark` when the campaign's design language has a dark variant in scope.
- **Inspo stays inspo.** Extracted tokens describe the reference site, not the campaign. The operator picks what carries over during the directions step; never lock a design language that is just another site's tokens verbatim.
