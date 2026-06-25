# Shape: Channel Profile

The generic shape for a distribution/delivery channel (e.g., LinkedIn, Substack). Instances of channels are stored as local profiles.

## Frontmatter Schema

```yaml
---
name: <human-readable channel name>
platform: <platform name, e.g., substack | linkedin>
url: <homepage URL>
type: channel
---
```

## Sections

- **Purpose & Target Audience** — High-level role of the channel in distribution and who it targets.
- **Connection & Credentials** — MCP server details, user environment variables, auth session token details (sid/cookies), and rotation expectations.
- **Conventions & Formatting** — Specific title/subtitle structures, footer append formats, handling of media (carousels, slide PNG exports), and subscribe buttons.
- **Voice & Tone Constraints** — Specific register or stylistic rules that apply exclusively to this channel's output.
