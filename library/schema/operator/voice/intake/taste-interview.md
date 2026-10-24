# Taste interview — voice intake protocol

How to extract a voice profile from a person. Run this LAST, after mining their corpus — it only fills what the corpus couldn't reveal. Save the raw answers to `transcript-{date}.md`, then compile into `../voice-profile.md` and seed `../pairs/`.

## The ordering rule
Extract from existing writing FIRST (sent emails, docs, past posts, dictated notes — richer and cheaper than self-report). Then run this interview only on the GAPS. The interview is the most expensive step; never re-extract what a corpus already contains.

- **Cold-start mode** (no corpus): the full matrix below, ~60–90 min. Only for someone with nothing written.
- **Gap-fill mode** (any corpus exists): mine the corpus first, then ask only what's missing — usually ~15–25 questions, ~20 min.

## Mechanics
Conversational, not a form. The person DICTATES (speech captures raw pattern before the internal editor sanitizes it). Push back on vague answers — "I write casually" is useless; demand a concrete example of casual-done-well vs. casual-done-lazily. Follow interesting threads. Flag contradictions with earlier answers. The agent NEVER fills a vague answer with a guess.

## The seven categories (the matrix)
Ask within these; depth over coverage. Skip what the corpus already answered.

1. **The cringe test** — what specific move in others' writing makes them wince? Not "it's salesy" — the actual phrase/structure. (Highest signal. Start here.)
2. **What they admire** — whose writing makes them think "yes, like that," and what specifically is it doing? (Gives the reference anchor.)
3. **Writing mechanics** — punctuation habits, sentence length, paragraph shape, what they reach for naturally.
4. **Core beliefs / POV** — the conviction under the writing; why they write at all. Not the topic — the belief.
5. **Voice & personality** — humour (kind? none?), references/nods, casual ceiling (how they talk privately vs. how they publish).
6. **Hard nos** — absolute bans in vocabulary, framing, tone.
7. **How they actually work** — do they dictate? edit by hand? where in the process does their voice enter? (Shapes how the agent should hand off drafts.)

## Compile
After the interview: write the dense profile to `../voice-profile.md` (their words, not adjectives), and turn any concrete "I'd write it like THIS not THAT" moments into pairs in `../pairs/`. The cringe answers become anti-patterns; the admired-writing answer becomes the reference anchor.
