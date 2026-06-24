# Your Voice — example scaffold

> Delivered with the open-source AgentFrame Marketing fork. Copy `library/context/operator.example/voice.example/` → `library/context/operator/voice/` (gitignored) and fill it in. This folder mirrors the canonical voice-system shape.

> **NOT SET UP YET?** If these files still hold bracketed placeholders, the voice system isn't built. Don't draft against placeholders — run [`library/process/voice-setup.md`](../../../process/voice-setup.md) to build it (gather samples → mine corpus → taste interview → compile).

Load when drafting any user-voiced text (post copy, slide text, body copy).

**Always load:** `identity.md`, `anti-patterns.md`, `voice-profile.md`.
**Retrieve by register:** the matching files in `pairs/`. Don't load all pairs.
**Structural shapes:** `templates/` when you need a deliverable blueprint.

## How to write in voice (a generative sequence, not a filter)

1. **Extract markers first.** From the register's pairs + the profile, pull 3–4 concrete markers for THIS piece — a move from a MOVE note, a sentence shape, a recurring phrase. Name them before drafting.
2. **Content pass.** Draft the argument and structure plainly, without chasing style.
3. **Style pass.** Rewrite toward the in-voice sides of the pairs, mandating the markers' inclusion — a separate pass whose only job is voice.
4. **Clean.** Check against `anti-patterns.md` (weighted preferences and per-piece budgets, not walls) and the vendored humanizer with a writing sample.

Don't run an inline self-critique/CoT pass during generation — it anchors output back to generic. Writing generically and then cleaning is the failure mode; markers-first prevents it.

## How to build your voice (the order matters)

Voice doesn't come from rules — it comes from examples of what you actually write and what you reject. Build in this order:

1. **Seed `pairs/` from writing you admire.** Before you have much of your own corpus, derive annotated contrastive pairs from writing you wish you wrote (a blog, an author): generic version → how the admired source writes it → what changed. This carries the voice early.
2. **Mine your own corpus.** Any existing writing — sent emails, docs, past posts, dictated notes — is higher-signal than self-description. Extract pairs and profile traits from what you actually wrote, not how you'd describe yourself.
3. **Run the taste interview LAST** (`intake/taste-interview.md`), only to fill what the corpus couldn't — the *why* behind your taste, boundaries, registers you haven't written yet. Don't self-report your style up front; it's aspirational and wrong.
4. **Let the harvest loop augment it.** As you ship deliverables, run `system/skills/voice-harvest/SKILL.md` (on demand or at lock) — it mines your edits into new pairs and flags recurring issues.
