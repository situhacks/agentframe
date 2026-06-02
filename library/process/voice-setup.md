# Voice Setup

Builds the operator's voice system the first time. Reached when the agent goes to load voice (`library/context/operator/voice/`) and finds it missing or still holding `.example` placeholders — the voice README redirects here. Owns the initial build; [`voice-mini-retro.md`](voice-mini-retro.md) owns ongoing maintenance.

Run it as a guided session with the operator. Drive the sequence; don't dump it as a checklist.

## The ordering rule
Extract from existing writing FIRST, interview LAST. Self-reported style ("I write casually") is aspirational and wrong; real samples and real corrections are the signal. The interview only fills what samples can't reveal.

## Sequence

1. **Copy + orient.** Copy `library/context/operator.example/voice.example/` → `library/context/operator/voice/`. Confirm the folder exists with its placeholder files.

2. **Gather samples** (any or all of three routes — all just writing for the agent to read; save raw to `voice/intake/corpus/`):
   - **Admired writing** — articles/posts the operator wishes they wrote. Aspirational seed; gives the reference anchor and the first pairs.
   - **Their own writing** — manual paste/upload of past posts, notes, dictated transcripts.
   - **Composio pull** — direct Composio at the operator's sent emails or written docs (long-form explaining/arguing prose is highest-signal). Operator points it at the right files/folders.
   Note inside each saved file whether it's admired or their own (one line, not a folder split).

3. **Mine the corpus into pairs + profile traits.** Read `corpus/`. Extract annotated contrastive pairs (generic → their version → the move) into `pairs/` by register, and draft profile traits (cadence, texture, openers, closers) from what the samples actually do — not from what the operator says about themselves. Most of the profile should come from here.

4. **Run the taste interview** ([`voice/intake/taste-interview.md`](../context/operator.example/voice.example/intake/taste-interview.md)) — only on the gaps the corpus couldn't reveal (the *why* behind taste, boundaries, registers not yet written, humour). Gap-fill mode if a corpus exists; cold-start full only if there's nothing. Dictated, adversarial, push on vague answers. Save to `voice/intake/transcript-{date}.md`.

5. **Compile.** Write `voice/voice-profile.md` (the operator's words, not adjectives). Fill `identity.md` (who/audience/registers) and `anti-patterns.md` (from the cringe answers + the admired-writing reference). Finalise `pairs/` (cap ~20–30, deduped by move, recency-weighted).

6. **Validate.** Draft one short test piece using only the new system. Does it land in 1–2 passes? If not, the pairs are the lever — sharpen or add before declaring setup done.

## Not this
- One-time setup, not per-session. Once `voice/` is filled, this doesn't re-run.
- Not a replacement for `voice-mini-retro` (that harvests corrections after deliverables ship).
- No self-report-then-stop: extraction-first, interview-last, always.
- Don't bloat pairs — the ~20–30 cap and dedup-by-move apply from day one.
