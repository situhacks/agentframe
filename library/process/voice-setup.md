# Voice Setup

Builds the operator's voice system the first time. Reached when the agent goes to load voice (`library/context/operator/voice/`) and finds it missing or unbuilt. Owns the initial build; [`voice-mini-retro.md`](voice-mini-retro.md) owns ongoing maintenance.

Run it as a guided session with the operator. Drive the sequence; don't dump it as a checklist.

## The ordering rule
Extract from existing writing FIRST, interview LAST. Self-reported style ("I write casually") is aspirational and wrong; real samples and real corrections are the signal. The interview only fills what samples can't reveal.

## Sequence

1. **Scaffold from the schema.** Generate `library/context/operator/voice/` from the voice schema (`library/context/operator-schema/voice/`): create `identity.md`, `anti-patterns.md`, `voice-profile.md`, `registers/` (one overlay per channel register named in `identity.md`), `corpus/` (one gold-corpus folder per register + `aspiration/`), `pairs/`, `templates/`, and `intake/corpus/` following the schema's shape. Generate from the shape — there is no example to copy.

2. **Gather samples** (any or all of three routes — all just writing for the agent to read; save raw to `voice/intake/corpus/`):
   - **Admired writing** — articles/posts the operator wishes they wrote. Aspirational seed; gives the reference anchor and the first pairs.
   - **Their own writing** — manual paste/upload of past posts, notes, dictated transcripts.
   - **Composio pull** — direct Composio at the operator's sent emails or written docs (long-form explaining/arguing prose is highest-signal). Operator points it at the right files/folders.
   Note inside each saved file whether it's admired or their own (one line, not a folder split).

3. **Seed the gold corpus.** From the gathered samples, have the operator pick their best FINISHED pieces — published or final text they'd stand behind — and store them verbatim in `voice/corpus/{register}/` (3–5 per register, one provenance line each; rules in the schema's `corpus/README.md`). This is the imitation anchor the style pass drafts against; without it, first drafts imitate whatever the model saw last. Admired writing goes to `corpus/aspiration/` as structure direction, never as a voice anchor. If a register has no finished pieces yet, say so — the register drafts thin until real work ships and `voice-harvest` promotes it.

4. **Mine the intake corpus into pairs + profile traits.** Read `intake/corpus/`. Extract annotated contrastive pairs (generic → their version → the move) into `pairs/` by register, and draft profile traits (cadence, texture, openers, closers, stance toward the reader) from what the samples actually do — not from what the operator says about themselves. Most of the profile should come from here.

5. **Run the taste interview** ([`taste-interview.md`](../context/operator-schema/voice/intake/taste-interview.md)) — only on the gaps the corpus couldn't reveal (the *why* behind taste, boundaries, registers not yet written, humour). Gap-fill mode if a corpus exists; cold-start full only if there's nothing. Dictated, adversarial, push on vague answers. Save to `voice/intake/transcript-{date}.md`.

6. **Compile.** Write `voice/voice-profile.md` (the operator's words, not adjectives). Fill `identity.md` (who/audience/register map), `anti-patterns.md` (from the cringe answers + the admired-writing reference), and one `registers/{register}.md` overlay per register — licensing only the moves the register's gold corpus evidences (shape in the schema's `registers/README.md`). Finalise `pairs/` (one pair per distinct move, deduped, recency-weighted; caps in the schema's `pairs/README.md`).

7. **Validate.** Draft one short test piece using only the new system, following the drafting sequence in the schema README (corpus-first style pass, clean pass in a separate turn, register test). Does it land in 1–2 passes? If not: a thin gold corpus is the first suspect, the pairs are the second lever — fix before declaring setup done.

## Not this
- One-time setup, not per-session. Once `voice/` is filled, this doesn't re-run.
- Not a replacement for `voice-mini-retro` (that harvests corrections after deliverables ship).
- No self-report-then-stop: extraction-first, interview-last, always.
- Don't bloat pairs — one pair per distinct move, dedup from day one; the corpus, not the pairs, carries the imitation load.
