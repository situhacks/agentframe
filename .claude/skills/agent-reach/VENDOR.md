# Agent Reach vendor record

- Upstream: https://github.com/Panniantong/Agent-Reach
- Pinned commit: `93ae1d18c37b707dec053c7c4f9d91cd8ef8943d` (2026-08-12T03:39:47Z, `main`)
- Installed version: `agent-reach 1.5.0`
- License: MIT
- Install date (UTC): `2026-08-19`
- Design plan: [`.claude/plans/2026-08-19-agent-reach-integration-PLAN.md`](../../../.claude/plans/2026-08-19-agent-reach-integration-PLAN.md)

## Scope

No upstream source is copied into this repository. Agent Reach is a machine-level CLI; this folder
holds only the AgentFrame integration boundary. `SKILL.md` here is AgentFrame-authored and
intentionally diverges from upstream (see Divergence).

## Install

```bash
python -m pipx install "https://github.com/Panniantong/Agent-Reach/archive/93ae1d18c37b707dec053c7c4f9d91cd8ef8943d.zip"
agent-reach install --env=auto                                          # read-only check first
agent-reach install --env=auto --system --channels reddit,twitter,linkedin,facebook,instagram
agent-reach skill --uninstall                                           # REQUIRED — see Divergence
```

Supporting tools installed alongside:

| Tool | Version | Purpose |
|---|---|---|
| `pipx` | 1.16.7 | isolated CLI installs |
| `mcporter` | 0.13.7 (npm -g) | MCP bridge for Exa search and LinkedIn |
| `uv` / `uvx` | 0.12.5 | runtime for `mcp-server-linkedin` |
| `rdt-cli` | 0.4.2, pinned `5e4fb3720d5c174e976cd425ccc3b879d52cac66` | Reddit cookie backend |
| `opencli` | 1.8.6 (npm -g) | browser-session backend (Reddit, Instagram, Facebook) |

All tool state lives in `~/.agent-reach/`, `~/.mcporter/`, and `~/.config/rdt-cli/`. Nothing is
written inside this repository.

## Divergence from upstream

1. **Upstream `SKILL.md` is not used.** Its `description` is an always-fire bilingual phrase list
   (`MUST USE when user wants to 调研/research/搜索/search/查/找 anything on the internet`) that
   captures routing from `deep-research` and the task routers, violating the builder principle
   *state over phrases*. It also carries a standing instruction to run `check-update` and surface a
   copy-paste upgrade URL after large tasks — vendor marketing inside runtime prose.
2. **`agent-reach install` re-registers the upstream skill** into `~/.claude/skills/agent-reach`,
   `~/.agents/skills/agent-reach`, and `~/.config/opencode/skills/agent-reach`. This was observed
   on 2026-08-19: the upstream skill appeared in the live session skill list immediately after
   install. **Every install or refresh must be followed by `agent-reach skill --uninstall`**, then a
   check that all three paths are absent.
3. **Five channels are deliberately unconfigured**: Bilibili, XiaoHongShu, V2EX, Xueqiu,
   Xiaoyuzhou. Operator scope decision, 2026-08-19.
4. **No write capability.** Read-only use on every platform.

## Security posture

- `agent_reach/cookie_extract.py` was audited at the pin on 2026-08-19: **no network calls of any
  kind** — no `requests`, `urllib`, HTTP URLs, `post`/`upload`, `socket`, `subprocess`, `os.system`,
  `eval`, or `exec`. Writes go through `atomic_write_private_text` to local paths only. It cannot
  exfiltrate.
- Version is **pinned**. Upgrades are a deliberate refresh, never automatic. The upstream update nag
  is removed for this reason.
- Credentials live outside the repo and are revocable by logging out the relevant session.

## Refresh

1. Review upstream changes since the pinned commit, paying attention to `cookie_extract.py` and any
   new channel that touches credentials.
2. Re-audit `cookie_extract.py` for network calls using the check in Security posture.
3. Update the pin above, reinstall via the pinned archive URL, re-run the scoped `--channels` install.
4. Run `agent-reach skill --uninstall` and verify the three native skill paths are gone.
5. Smoke test: one credential-free channel and one credentialed channel.
6. Append a `system_changes` row through `system/audit/writer.py`.
