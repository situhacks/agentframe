# Voice Setup

Builds the operator's voice system the first time. Reached when the agent goes to load voice (`library/context/operator/voice/`) and finds it missing or unbuilt. Owns the initial build; [`voice-mini-retro.md`](voice-mini-retro.md) owns ongoing maintenance.

Run it as a guided session with the operator. Drive the sequence; don't dump it as a checklist.

## The ordering rule
Extract from existing writing FIRST, interview LAST. Self-reported style ("I write casually") is aspirational and wrong; real samples and real corrections are the signal. The interview only fills what samples can't reveal.

## The separation rule

Build registers from reusable voice temperature and reader relationship, not destinations. Channel constraints live in `library/context/channels/{channel}/profile.md`; deliverable shape lives in templates; pair metadata keeps `register` separate from task `context`.

## Sequence

1. **Scaffold from the schema.** Generate `library/context/operator/voice/` from `library/context/operator-schema/voice/`: create the shared core; `registers/formal.md` and `registers/informal.md`; matching gold-corpus folders plus `aspiration/`; `pairs/`; `templates/`; and `intake/corpus/`. Generate from the shape — there is no example to copy.

2. **Gather samples** (any or all of three routes — all just writing for the agent to read; save raw to `voice/intake/corpus/`):
   - **Admired writing** — articles/posts the operator wishes they wrote. Aspirational seed; gives the reference anchor and the first pairs.
   - **Their own writing** — manual paste/upload of past posts, notes, dictated transcripts.
   - **Composio pull** — direct Composio at the operator's sent emails or written docs (long-form explaining/arguing prose is highest-signal). Operator points it at the right files/folders.
   Note inside each saved file whether it's admired or their own (one line, not a folder split).

3. **Seed the gold corpus.** Have the operator classify their best FINISHED pieces by reusable register (`formal` or `informal`), not by platform, and store them verbatim in `voice/corpus/{register}/` (3–5 per register, one provenance line each). Preserve channel in provenance. Admired writing goes to `corpus/aspiration/`, never the voice anchor. If a register is thin, say so until real work ships and `voice-harvest` promotes it.

4. **Mine the intake corpus into pairs + profile traits.** Extract pairs tagged independently by voice `register` and task `context`; do not use a platform or deliverable name as a register. Draft profile traits from what the samples actually do, not how the operator describes themselves.

5. **Run the taste interview** ([`taste-interview.md`](../context/operator-schema/voice/intake/taste-interview.md)) — only on the gaps the corpus couldn't reveal (the *why* behind taste, boundaries, registers not yet written, humour). Gap-fill mode if a corpus exists; cold-start full only if there's nothing. Dictated, adversarial, push on vague answers. Save to `voice/intake/transcript-{date}.md`.

6. **Compile.** Write `voice-profile.md`, `identity.md`, and `anti-patterns.md`; define the formal/informal selection contract and optional base-plus-borrow recipe; and keep each register overlay limited to voice differences evidenced by its corpus. Put channel rules in channel profiles and shape rules in deliverable templates. Finalise pairs with independent register/context tags.

7. **Validate.** Draft the same short payload twice for different channels using one register, then once as a declared blend. Confirm channel changes do not silently change voice, the base corpus remains the main anchor, and the blend direction is visible without loading both full corpora.

## Not this
- One-time setup, not per-session. Once `voice/` is filled, this doesn't re-run.
- Not a replacement for `voice-mini-retro` (that harvests corrections after deliverables ship).
- No self-report-then-stop: extraction-first, interview-last, always.
- Don't bloat pairs — one pair per distinct move, dedup from day one; the corpus, not the pairs, carries the imitation load.
