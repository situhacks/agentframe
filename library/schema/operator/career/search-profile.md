# Search Profile (schema)

Copy to `library/context/operator/career/search-profile.md`. The active search: what the scout sweeps, what disqualifies a posting, and how interview rounds get captured. This is campaign config, not identity — identity lives in `profile.md` and the tracks.

## Campaign

- Active tracks: FILL-ME (which `tracks/` files are live)
- Target titles: FILL-ME (per-company titles where they differ — searching the wrong tier wastes a sweep)
- Regions: FILL-ME
- Dealbreakers: FILL-ME (auto-drop rules the scout applies)

## Capture

How a round gets recorded and turned into text. The interview arc reads this at step 7; while it is
unfilled the agent asks once and writes the answer here, and no later round asks again.

- Recording: FILL-ME (how the operator captures a call, or `none` — choosing not to record is a
  complete answer, and the arc handles an uncaptured round)
- Transcription: FILL-ME (how a recording becomes text — a local model, a paid service, a paste into
  chat; name what this operator actually has)
- Lands at: FILL-ME (where a finished recording or transcript appears before it is filed)

Whatever is named here, the destination does not change: the transcript is copied into the
application's `sources/` and registered in its `INDEX.md`. Capture method is provenance; the filed
record is the procedure.

## Watchlist (drives `job-scout` sweeps)

| Company | Target title | ATS | Board slug / endpoint | Pull method | Screen notes |
|---|---|---|---|---|---|
| FILL-ME | FILL-ME | greenhouse | FILL-ME | auto-json | |

Pull methods: `auto-json` (public ATS feed — first choice) · `web-search` (career page, fetch logged-off) · `paste` (gated portal — the scout never logs in or bypasses anti-bot; it leaves a paste-JD row).
