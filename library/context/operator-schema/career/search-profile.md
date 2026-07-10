# Search Profile (schema)

Copy to `library/context/operator/career/search-profile.md`. The active search: what the scout sweeps and what disqualifies a posting. This is campaign config, not identity — identity lives in `profile.md` and the tracks.

## Campaign

- Active tracks: FILL-ME (which `tracks/` files are live)
- Target titles: FILL-ME (per-company titles where they differ — searching the wrong tier wastes a sweep)
- Regions: FILL-ME
- Dealbreakers: FILL-ME (auto-drop rules the scout applies)

## Watchlist (drives `job-scout` sweeps)

| Company | Target title | ATS | Board slug / endpoint | Pull method | Screen notes |
|---|---|---|---|---|---|
| FILL-ME | FILL-ME | greenhouse | FILL-ME | auto-json | |

Pull methods: `auto-json` (public ATS feed — first choice) · `web-search` (career page, fetch logged-off) · `paste` (gated portal — the scout never logs in or bypasses anti-bot; it leaves a paste-JD row).
