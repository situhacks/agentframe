# Your Voice — example scaffold

> Shipped with the open-source AgentFrame Marketing fork. Copy `library/context/operator.example/voice.example/` → `library/context/operator/voice/` (gitignored) and fill it in. This folder mirrors the canonical voice-system shape.

> **NOT SET UP YET?** If these files still hold bracketed placeholders, the voice system isn't built. Don't draft against placeholders — run [`library/process/voice-setup.md`](../../../process/voice-setup.md) to build it (gather samples → mine corpus → taste interview → compile).

Load when drafting any user-voiced text (post copy, slide text, body copy).

**Always load:** `identity.md`, `anti-patterns.md`, `voice-profile.md`.
**Retrieve by register:** the matching files in `pairs/`. Don't load all pairs.
**Structural shapes:** `templates/` when you need a deliverable blueprint.

How to write in voice: generate FROM the pairs and profile, then clean with `anti-patterns.md` + the vendored humanizer skill. Rules catch generic AI; the pairs and profile produce the voice. Don't run a self-critique reasoning pass — it pulls toward generic.

## How to build your voice (the order matters)

Voice doesn't come from rules — it comes from examples of what you actually write and what you reject. Build in this order:

1. **Seed `pairs/` from writing you admire.** Before you have much of your own corpus, derive annotated contrastive pairs from writing you wish you wrote (a blog, an author): generic version → how the admired source writes it → what changed. This carries the voice early.
2. **Mine your own corpus.** Any existing writing — sent emails, docs, past posts, dictated notes — is higher-signal than self-description. Extract pairs and profile traits from what you actually wrote, not how you'd describe yourself.
3. **Run the taste interview LAST** (`intake/taste-interview.md`), only to fill what the corpus couldn't — the *why* behind your taste, boundaries, registers you haven't written yet. Don't self-report your style up front; it's aspirational and wrong.
4. **Let `voice-mini-retro` augment it.** As you ship deliverables, the lock-event loop harvests your corrections into new pairs automatically.
