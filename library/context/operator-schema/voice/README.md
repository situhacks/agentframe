# Voice — canonical shape (v5, register/channel separated)

> **Canonical shape of the voice system.** [`voice-setup.md`](../../../process/voice-setup.md) generates the private `library/context/operator/voice/` tree from this shape.

> **NOT SET UP YET?** If these files still hold bracketed placeholders, run [`voice-setup.md`](../../../process/voice-setup.md). Do not draft against placeholders.

Load for any user-voiced text, regardless of destination: posts, essays, emails, decks, memos, scripts, or other prose.

## Resolve context before loading

Resolve two independent inputs:

1. **Channel or deliverable context** — where the text will appear and what shape or platform constraints apply. Load the relevant `library/context/channels/{channel}/profile.md` and deliverable template when they exist.
2. **Voice recipe** — how the operator should sound. A channel never selects this.

Canonical recipe:

```yaml
voice:
  base_register: formal        # formal | informal
  borrow_from: []              # optional: [informal] or [formal]
  direction: ""               # required when borrowing; name the traits or sections
```

Use an explicit operator instruction or file recipe first. Otherwise infer the base from audience, risk, purpose, and desired relationship to the reader, then state the choice. Do not infer it from LinkedIn, Substack, email, slides, or any other channel.

A blend always has one base. Load the base as the imitation anchor, then borrow only named traits from the other register. Do not use percentages. This keeps the result coherent and avoids loading two full corpora by default.

## Load path

After resolving the recipe, load:

- `identity.md`, `voice-profile.md`, and `anti-patterns.md`;
- `registers/{base_register}.md`;
- every full piece in `corpus/{base_register}/`;
- pair files matching the register and the task context, plus any cross-cutting file marked always-load;
- for a blend, the secondary register overlay plus the smallest secondary evidence set that demonstrates the named borrow: relevant pairs and usually one or two corpus pieces, not the whole secondary corpus;
- `templates/` only when a voice-specific shape blueprint is needed.

## How to write in voice

1. **Content pass.** Draft the argument and structure plainly. Deliverable templates and aspiration pieces may inform shape, never wording.
2. **Style pass.** Put the selected corpus evidence first and the rewrite instruction last. Extract 3–4 concrete markers for this piece, then rewrite toward them. Match cadence, diction, and structure; never copy facts, phrases, or topics.
3. **Clean pass.** In a separate context, apply `anti-patterns.md` and the humanizer with an operator sample. Recheck the recipe after cleaning.
4. **Register test.** Read aloud: would this sit naturally beside the selected operator corpus while satisfying the named blend direction?

Do not run inline self-critique during generation. Evaluation is a separate pass. A draft written without corpus evidence will imitate whatever the model saw last; a draft written generically and merely cleaned afterward remains generic.

## How to build the voice system

1. Seed `corpus/{register}/` with 3–5 finished pieces per reusable register. Record channel as provenance, not taxonomy.
2. Mine the operator's writing into pairs tagged independently by `register` and task `context`.
3. Keep admired writing under `corpus/aspiration/` for structure direction only.
4. Run the taste interview last to fill gaps the writing cannot reveal.
5. Let [`voice-harvest`](../../../../system/skills/voice-harvest/SKILL.md) promote finished work and edit deltas over time.
