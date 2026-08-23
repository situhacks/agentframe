---
name: agent-reach
description: "Routing layer over locally installed CLIs for web sources the default toolchain cannot reach — Reddit, LinkedIn, X, Instagram, Facebook — plus higher-fidelity paths for ordinary web pages, YouTube transcripts, semantic search, GitHub, and RSS. Load when a needed source returns a fetch refusal, a block, or an empty result set from WebFetch/WebSearch, or when a task names one of those platforms as a required source."
allowed-tools: Bash, Read, Write
---

# Agent Reach

A selector and health-checker over separately installed upstream CLIs (OpenCLI, rdt-cli, twitter-cli,
yt-dlp, mcporter, gh). It is not a wrapper — commands run against the upstream tools directly.

**Read-only. No posting, commenting, liking, following, or connecting on any platform, ever.**

## Load when

The default toolchain has failed or will fail on a required source:

- WebFetch refuses a host outright (`unable to fetch from ...`), or WebSearch returns no results from
  a site that demonstrably has them.
- A task names Reddit, LinkedIn, X/Twitter, Instagram, or Facebook as a source that must be read.
- A YouTube transcript, an RSS feed, or a semantic search over technical sources is the input.

**Do not load** when WebFetch already returns the content. Adding a hop costs latency and buys
nothing. This skill exists for the blocked case, not as a default fetch path.

## Required pre-flight

Before using any credentialed channel:

```bash
agent-reach doctor --json
```

Read `active_backend` for the target channel. A `null` value means the doctor declined to verify
live — it deliberately does not run commands that would auto-read browser cookies — so it means
"unverified", not "unavailable". Try the channel's read command; if it fails, follow the retry chain
below. Say which channel and backend you are using before you start.

## Channels

| Channel | Command | Auth |
|---|---|---|
| Any web page | `curl -s "https://r.jina.ai/<URL>"` | none |
| Semantic search | `mcporter call exa.web_search_exa query="..." numResults=5` | none |
| YouTube | `yt-dlp --skip-download --write-auto-sub --sub-format vtt --sub-lang en -o '%(id)s' "<URL>"` | none |
| GitHub | `gh api repos/OWNER/REPO ...`, `gh search code ...` | token optional |
| RSS | `python -c "import feedparser; ..."` | none |
| Reddit (OpenCLI) | `opencli reddit search "q" -f yaml` · `opencli reddit read POST_ID -f yaml` · `opencli reddit subreddit NAME -f yaml` | browser session |
| Reddit (rdt-cli) | `rdt search "q" --limit 10` · `rdt read POST_ID` · `rdt sub NAME --limit 20` | cookie |
| LinkedIn | `mcporter call linkedin.get_person_profile linkedin_username="..."` · `linkedin.search_jobs keywords="..." location="..."` · `linkedin.get_company_profile company_name="..."` | login |
| X/Twitter | `twitter` CLI; requires `TWITTER_AUTH_TOKEN` and `TWITTER_CT0` exported in the calling process | cookie |
| Instagram / Facebook | `opencli instagram ...` · `opencli facebook ...` | browser session |

LinkedIn fallback for public pages without the MCP: `curl -s "https://r.jina.ai/https://linkedin.com/in/<user>"`.

## Out of scope

Bilibili, XiaoHongShu, V2EX, Xueqiu, and Xiaoyuzhou are deliberately not configured. If a task
genuinely needs one, treat that as a scope change and say so rather than configuring it inline.

## Retry chain

1. Channel command fails → re-run `agent-reach doctor --json` and read that channel's hint text; it
   names the specific missing dependency or credential.
2. Backend reports a missing browser session → the OpenCLI Chrome extension is not connected. Run
   `opencli doctor`. This is an operator action; do not attempt to work around it.
3. Cookie-based backend reports expired credentials → cookies must be re-exported by the operator.
   Never attempt to read browser cookie stores directly.
4. Two failures on the same channel → stop and report the gap. **Do not silently substitute a
   different source.** A substituted source that does not match the question produces a confidently
   wrong answer; a named gap does not.

## Use inside deep-research

When a specialist role's source class is unreachable by the default fetch path, route that role
through this skill rather than dropping it or swapping in an adjacent source. If the channel is also
unavailable here, the role returns a documented gap and the synthesis carries it forward as a
limitation.

## Provenance

Upstream `SKILL.md` is deliberately **not** used — its trigger is an always-fire bilingual phrase
list that hijacks routing, and it embeds a version-update nag in runtime instructions. This file
replaces it. Pin, install command, and refresh procedure: [`VENDOR.md`](VENDOR.md).

Running `agent-reach install` re-registers the upstream skill into `~/.claude/skills/`,
`~/.agents/skills/`, and `~/.config/opencode/skills/`. After any install or refresh, run
`agent-reach skill --uninstall` and confirm those three paths are gone.
