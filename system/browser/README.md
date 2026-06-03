# AgentFrame Browser Runtime

This folder is AgentFrame's thin wrapper around the vendored [`browser-harness`](../skills/browser-harness/) package.

Use it only when an approved API/MCP/CLI cannot do the job and the operator can perform the same task in a normal browser.

## Active Surface

- `src/work-browser.js` launches or reuses the controlled Microsoft Edge Work Browser.
- `workflows/README.md` defines how repeatable browser workflows become `recipe.md` files.
- `../skills/browser-harness/SKILL.md` owns browser-harness mechanics: screenshots, clicks, tabs, helpers, and domain skills.

## Work Browser

Start or reuse the controlled Edge profile:

```powershell
npm run work-browser
```

Then point browser-harness at the Work Browser DevTools endpoint:

```powershell
$env:BU_CDP_URL = "http://127.0.0.1:9222"
@'
print(page_info())
'@ | browser-harness
```

The wrapper intentionally uses a dedicated Edge profile under `system/browser/local/` so browser-harness does not attach to an arbitrary personal or work browser profile.

## Runtime Storage

The following paths are local runtime state and are not workflow memory:

- `local/` - controlled Edge profile, marker file, screenshots, and debug files.
- `cache/` - disposable install/cache state.
- `node_modules/` - local JavaScript dependencies.
- `workflows/**/runs/` - one-off run captures when a human deliberately saves them.

These paths are ignored by the repository and should not be published.

## Human Boundaries

Authentication is human-owned unless a workflow recipe explicitly authorizes deterministic, non-secret routing controls.

Always stop at passwords, one-time codes, authenticator approvals, security questions, device compliance, permission prompts, or any other human-secret boundary. Do not store credential values, auth codes, session tokens, or full auth callback URLs.
