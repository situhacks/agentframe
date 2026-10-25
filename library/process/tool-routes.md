# Tool Routes

## Purpose

The working route for each source or tool the default toolchain fumbles on this machine, and the hosts not worth retrying. Platform channels (Reddit, X, Instagram, the LinkedIn MCP, any page through `r.jina.ai`) belong to [`agent-reach`](../../.claude/skills/agent-reach/SKILL.md); this file holds what that skill does not.

## When To Load

A transcript, page, price, or media tool below is about to run, or a default fetch of one of these sources has failed.

## Procedure

Write each script to the scratchpad with the Write tool and run it; set `PYTHONIOENCODING=utf-8` when it prints page text.

- **YouTube talk, metadata first.** `yt-dlp --print "%(title)s / %(channel)s / %(upload_date)s / %(duration)s" --print "%(chapters)j" <url>`. Aggregator channels re-upload talks under their own titles; find the original venue and date separately.
- **YouTube captions.** `yt-dlp --skip-download --write-sub --write-auto-sub --sub-lang "en.*,en" --sub-format vtt -o "<name>.%(ext)s" <url>`, then flatten the VTT with a script that strips `<c>` tags, drops consecutive repeated lines, and marks each minute.
- **Auto-captions are not quotes.** File the flattened transcript as a source marked auto-captions, with the original venue and the upload channel; no line is quoted verbatim without a listen-back.
- **Retail hosts that block.** walmart.ca and walmart.com return a bot check, and amazon.ca returns 503 to WebFetch and `r.jina.ai` alike; do not retry them.
- **amazon.com.** `curl -s "https://r.jina.ai/https://www.amazon.com/dp/<ASIN>"`, then grep the output for the title, `displayPrice`, stars, and the feature bullets.
- **Canadian prices.** Give the spec checklist with a US listing as the example, and mark the CAD price unverified.
- **Reddit.** WebSearch refuses a `reddit.com` domain filter; read Reddit through agent-reach's OpenCLI channel.
- **LinkedIn post or profile.** Check `ls -t ~/Downloads/*LinkedIn*.html` before any fetch: the operator saves pages there as complete HTML, Activity feed and full post text included.
- **LinkedIn save extraction.** Strip script, style, and svg, turn block-closing tags into newlines, and unescape entities; date posts from the file's mtime, since the page shows only relative ages.
- **LinkedIn save tail.** The end of every save is the operator's own LinkedIn chrome (messaging overlay, ads, messages); never quote it or treat it as source material.
- **Flight cash fares.** Search engines and airline pages give only undated "from" fares. Use `fast-flights` (Python 3.12): `get_flights(create_query(flights=[FlightQuery(date=..., from_airport=..., to_airport=...)], trip="one-way", currency="CAD", exclude_basic_economy=...))`, run with `python -I` from a neutral directory.
- **Basic economy.** Run the fare query with and without `exclude_basic_economy`; Air Canada's domestic bereavement fares exclude Basic.
- **Points fares.** Report cash fares only; a points price needs a logged-in airline session.
- **gifsicle.** npm installs it as the wrapper script `gifsicle.cmd`, and Python's `subprocess` does not try PATHEXT, so pass it `shutil.which("gifsicle") or shutil.which("gifsicle.cmd")`.
- **GIF size.** `gifsicle -O3 --lossy=60` roughly halves a Pillow-written GIF with no visible loss and merges identical hold frames.

## Verification

The route returned the transcript, page, fares, or file. A host that still blocks is reported as a gap with what was tried, never replaced by an adjacent source.

## Boundaries

Does not own platform channels or their retry chain ([`agent-reach`](../../.claude/skills/agent-reach/SKILL.md)), transcription of the operator's own recordings (`system/tools/transcribe.py`), or the research method ([`research-and-signals.md`](research-and-signals.md)).
