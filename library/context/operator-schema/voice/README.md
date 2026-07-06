# Voice — canonical shape (v4, corpus-anchored)

> **Canonical shape of the voice system.** The voice build ([`voice-setup.md`](../../../process/voice-setup.md)) generates the operator's `library/context/operator/voice/` from this shape; it is not copied by hand. The operator's voice folder is gitignored (private).

> **NOT SET UP YET?** If these files still hold bracketed placeholders, the voice system isn't built. Don't draft against placeholders — run [`library/process/voice-setup.md`](../../../process/voice-setup.md) to build it (gather samples → seed the gold corpus → mine pairs → taste interview → compile).

Load when drafting any user-voiced text (post copy, slide text, body copy, essays).

**Step zero — name the register** (from the deliverable type via `identity.md`), then load:
- `identity.md`, `voice-profile.md`, `anti-patterns.md` (the always-three)
- `registers/{register}.md` (the channel overlay)
- `corpus/{register folder}/` — ALL pieces in the register's folder, whole (this is the imitation anchor; a register is never drafted without its corpus in context)
- the matching `pairs/` file(s) for the register, plus any cross-cutting pairs file the pairs README marks always-load. Don't load all pairs.
- `templates/` when you need a deliverable-shape blueprint.

## How to write in voice (imitate the corpus; rules clean up after)

1. **Content pass.** Draft the argument and structure plainly, without chasing style. Structure may borrow from the deliverable template and the aspiration shape (`corpus/aspiration/`) — shapes, never sentences.
2. **Style pass — a separate rewrite whose only job is voice.** Place the register's full corpus pieces FIRST in working context and the rewrite instruction LAST (models weight both ends; the instruction sits closest to generation). Extract 3–4 concrete markers from the corpus + pairs for THIS piece — a sentence shape, a recurring move, a cadence pattern — and mandate their inclusion. Rewrite the skeleton toward the corpus. **Guard: match cadence, diction, and structure — never copy facts, phrases, or topics from the corpus pieces.**
3. **Clean pass — a separate turn, never sharing context with the corpus.** Check against `anti-patterns.md` (weighted preferences + budgets) and the vendored humanizer with a sample of the operator's writing. Rules live here because in-context exemplars override in-context instructions — a rule sharing the style pass loses; a rule applied after wins.
4. **The register test.** Read aloud: would each paragraph sit naturally inside the register's corpus pieces? The operator's own published work is the measuring stick — never an admired outside writer (those are structure aspiration only, see `corpus/aspiration/`).

Do **not** run an inline self-critique/CoT reasoning pass during generation — clinical deliberation tokens anchor the output back to generic. Evaluation happens in a separate pass, never inside the generation sequence.

Writing generically and then cleaning is the failure mode ("cleaned-up generic AI"). So is imitating the wrong exemplar: a draft written without the register's corpus in context will sound like whatever the model saw last. Corpus-first exists to prevent both.

## Comparing anchors (optional operator experiment)

When the operator asks to compare, run the style pass twice on the same skeleton — (a) the register's corpus pieces as exemplars, (b) a live-fetched aspiration piece as exemplar — and present both. Default remains (a) unless the operator rules otherwise after a comparison.

## How to build your voice (the order matters)

Voice comes from full examples of what you actually shipped, steered by examples of what you rejected — not from rules. Build in this order:

1. **Seed the gold corpus** (`corpus/{register}/`) from your best finished pieces — published or final text you'd stand behind, verbatim, 3–5 per register. This is the imitation anchor; everything else steers. If you have no finished pieces yet, start with pairs (next step) and promote your first shipped work into the corpus as it lands.
2. **Mine your own writing into pairs.** Any existing writing — sent emails, docs, past posts, dictated notes — is higher-signal than self-description. Extract annotated contrastive pairs (generic version → your version → the move) and profile traits from what you actually wrote, not how you'd describe yourself.
3. **File admired writing as aspiration, not voice.** Writing you wish you wrote goes in `corpus/aspiration/` — structure direction only, never the register test and never a style-pass exemplar.
4. **Run the taste interview LAST** (`intake/taste-interview.md`), only to fill what the corpus couldn't — the *why* behind your taste, boundaries, registers you haven't written yet. Don't self-report your style up front; it's aspirational and wrong.
5. **Let the harvest loop compound it.** As you ship deliverables, run `system/skills/voice-harvest/SKILL.md` (on demand or at lock) — it mines your edits into new pairs, promotes published/locked finals into the corpus, and flags recurring issues.
