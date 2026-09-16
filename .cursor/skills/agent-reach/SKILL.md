---
name: agent-reach
description: "Routing layer over locally installed CLIs for web sources the default toolchain cannot reach — Reddit, LinkedIn, X, Instagram, Facebook — plus higher-fidelity paths for ordinary web pages, YouTube transcripts, semantic search, GitHub, and RSS. Load when a needed source returns a fetch refusal, a block, or an empty result set from WebFetch/WebSearch, or when a task names one of those platforms as a required source."
allowed-tools: Bash, Read, Write
---

# Agent Reach

A selector over separately installed upstream CLIs (OpenCLI, rdt-cli, twitter-cli, yt-dlp, mcporter,
gh). It is not a wrapper — commands run against the upstream tools directly.

**Read-only. No posting, commenting, liking, following, or connecting on any platform, ever.**

The backends expose write tools and nothing but this rule stops them. Never call
`linkedin.connect_with_person` or `linkedin.send_message`, and never call an `opencli` subcommand its
own `--help` marks `[write]` (`follow`, `like`, `comment`, `post`, `save`, `note`, and the rest).

## Load when

The default toolchain has failed or will fail on a required source:

- WebFetch refuses a host outright (`unable to fetch from ...`), or WebSearch returns no results from
  a site that demonstrably has them.
- A task names Reddit, LinkedIn, X/Twitter, Instagram, or Facebook as a source that must be read.
- A YouTube transcript, an RSS feed, or a semantic search over technical sources is the input.

**Do not load** when WebFetch already returns the content. Adding a hop costs latency and buys
nothing. This skill exists for the blocked case, not as a default fetch path.

## Required pre-flight

Only the browser-session channels (Reddit, Instagram, Facebook) need one. The OpenCLI daemon is not
a service. It spawns when an `opencli` command runs, and `agent-reach doctor` never runs one, so the
normal cold-start state is a daemon that is simply absent.

```bash
opencli daemon restart      # spawns it; extension reconnects in 1-5s, worst case ~45s
```

Wait for `Extension: connected` before reading any failure as real. Chrome itself must be running,
and the site session comes from the Chrome profile rather than from any open tab, so the operator
does not need the site open. Every other channel needs no pre-flight: run the command.

`agent-reach doctor --json` is a dependency inventory, not a health check. Its `active_backend` stays
`null` for every browser-session channel even while that channel is serving data, so it can never
confirm a channel is up and a `warn` from it is not a reason to stop. Its `message` text is Chinese;
`status` and `backends` are the readable fields. Say which channel and backend you are using before
you start.

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
| LinkedIn | `mcporter call linkedin.get_person_profile linkedin_username="..."` · `linkedin.search_posts keywords="..."` · `linkedin.get_feed` · `linkedin.search_jobs keywords="..." location="..."` · `linkedin.get_company_profile company_name="..."` · `linkedin.get_company_posts company_name="..."` | login |
| X/Twitter | `opencli twitter search "q" -f yaml` · `opencli twitter profile USER` · `opencli twitter thread TWEET_ID` | browser session |
| Instagram / Facebook | `opencli instagram ...` · `opencli facebook ...` | browser session |
| TikTok (search) | `opencli tiktok search "q" -f yaml` | none |
| TikTok (one video, or a creator's feed) | `yt-dlp --skip-download --print "..." <video URL>` · `yt-dlp ... "tiktokuser:<channel_id>"` | none |

There is no fetch-a-post-by-URL tool. To reach one known post, `search_posts` on a distinctive phrase
from it is the short route; `get_person_profile` with `sections="posts"` scrapes the author's recent
activity and is slow. The LinkedIn MCP is `uvx mcp-server-linkedin@latest`, so a cold first call
resolves the package and can exceed a 30s client timeout while a warm one answers in about 8s. A
timeout on the first call is not an outage — retry once.

LinkedIn fallback for public pages without the MCP: `curl -s "https://r.jina.ai/https://linkedin.com/in/<user>"`.
Anonymous Jina against LinkedIn is a shared-reputation channel: unrelated traffic from other users
can return `AbuseAlleviationError` and block the domain for hours. No Jina API key is configured, so
the authenticated tier is not available and anonymous is the only mode. It works often enough to try
and is never something to depend on.

X/Twitter runs on the browser session, not on `twitter-cli`. `TWITTER_AUTH_TOKEN` and `TWITTER_CT0`
are the legacy `twitter-cli` path and are not needed; the cookies come from the Chrome profile.

TikTok is search-only through OpenCLI. Every command needing the logged-in identity — `whoami`,
`user`, `explore`, `profile`, `creator-videos` — returns `identity not rehydrated` or `EMPTY_RESULT`
against a demonstrably logged-in page, so the adapter has drifted from the live site. Use `search` for
discovery, then yt-dlp for a specific video or a creator's back catalogue. yt-dlp needs the opaque
`channel_id` rather than the handle: read it with `--print "%(channel_id)s"` from any one of that
creator's videos, then pass `tiktokuser:<channel_id>`. Expect an occasional per-video extraction error
in a long list.

Instagram answers `HTTP 429 - make sure you are logged in` when it is rate-limiting, whatever the
session state. Check `opencli instagram whoami` before believing the login half of that message.

## Out of scope

Bilibili, XiaoHongShu, V2EX, Xueqiu, and Xiaoyuzhou are deliberately not configured. If a task
genuinely needs one, treat that as a scope change and say so rather than configuring it inline.

## Retry chain

1. Channel command fails → read the command's own error first, which is specific and in English.
   `agent-reach doctor --json` is the second stop, useful only for whether a dependency or credential
   is installed at all.
2. Reddit, Instagram, or Facebook reports a missing browser session → the daemon is down far more
   often than the extension is. Run `opencli daemon restart`, wait for `Extension: connected`, and
   retry once. Escalate to the operator only when the extension stays disconnected past ~45s with
   Chrome open, which is the case where it really has been disabled or removed.
3. `attach failed: Cannot access a chrome-extension:// URL of different extension` → another Chrome
   extension injects into the target site and OpenCLI cannot walk the tab's frames past it. Check the
   operator's extensions for one whose manifest matches that site, especially one declaring
   `sidePanel` or a content script with `all_frames: true`. Disabling it is the fix and it is the
   operator's call. A site-specific extension can also swallow the exact API a command reads: one
   holding `host_permissions` on an endpoint plus `webRequest` produces `EMPTY_RESULT` from a page
   that looks fine in the browser.
4. Cookie-based backend reports expired credentials → cookies must be re-exported by the operator.
   Never attempt to read browser cookie stores directly, `yt-dlp --cookies-from-browser` included,
   without the operator asking for it.
5. Two failures on the same channel → stop and report the gap. **Do not silently substitute a
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
