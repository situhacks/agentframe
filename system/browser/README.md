# AgentFrame Browser Runtime

This folder is AgentFrame's thin wrapper around the vendored [`browser-harness`](../skills/browser-harness/) package.

Use it only when an approved API/MCP/CLI cannot do the job and the operator can perform the same task in a normal browser.

## Active Surface

- `src/work-browser.js` launches or reuses a controlled browser profile — Edge (work) or Chrome (home).
- `workflows/README.md` defines how repeatable browser workflows become `recipe.md` files.
- `../skills/browser-harness/SKILL.md` owns browser-harness mechanics: screenshots, clicks, tabs, helpers, and domain skills.

## Controlled Browsers

Two profiles, each on its own port, so both can run at once.

| Browser | Command | Port | Profile | Use for |
|---|---|---|---|---|
| Edge | `npm run work-browser` | 9222 | `local/agentframe-work-profile` | Work surfaces (the operator's work browser is Edge) |
| Chrome | `npm run home-browser` | 9223 | `local/agentframe-home-profile` | Personal surfaces — LinkedIn, Substack (the operator's home browser is Chrome) |

Then point browser-harness at the matching DevTools endpoint:

```powershell
$env:BU_CDP_URL = "http://127.0.0.1:9223"   # 9222 for Edge
@'
print(page_info())
'@ | browser-harness
```

Flags: `--browser=chrome|edge`, `--port=`, `--profile=`, `--browser-path=`, `--start-url=`.
Env: `AGENTFRAME_BROWSER`, `AGENTFRAME_EDGE_PATH` / `AGENTFRAME_CHROME_PATH`, `AGENTFRAME_WORK_PROFILE` / `AGENTFRAME_HOME_PROFILE`, `AGENTFRAME_BROWSER_PORT`.

Both wrappers intentionally use **dedicated profiles** under `system/browser/local/` so browser-harness never attaches to the operator's real personal or work browser. Choosing Chrome selects a *separate controlled Chrome profile* — it does not attach to his everyday Chrome. Each browser carries its own session marker, and the runtime refuses to attach to a DevTools endpoint it does not own.

Because the profiles are dedicated, **each site needs a one-time human sign-in inside the controlled profile.** The session then persists across runs.

AgentFrame uses Browser Harness locally only. Do not authenticate to, start, or route work through its cloud-browser, remote-daemon, or profile-sync paths.

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
