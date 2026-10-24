# Register overlays — one file per reusable voice register

A register overlay scopes the shared voice core to a reusable level of formality, warmth, candour, or conversational distance. It is channel-independent: the same formal register may serve a LinkedIn post, executive deck, email, or memo.

`identity.md` names the registers. Each gets one overlay here and one corpus folder in `../corpus/`. Channel formatting, audience defaults, link placement, title structures, and platform mechanics belong in `library/context/channels/{channel}/profile.md` or a deliverable template.

An overlay carries only what differs from the shared core. `identity.md`, `voice-profile.md`, and `anti-patterns.md` always apply underneath.

## Shape

```markdown
# Register: {register-name}

[What reader relationship and temperature this register creates. Explain when it
fits by purpose and risk, not by platform.]

**Corpus:** ../corpus/{register-name}/ — the imitation anchor.

## Temperature and stance
[Humour, candour, authority, directness, and casual ceiling evidenced by shipped work.]

## Cadence and texture
[Only the rhythm, diction, and rhetorical moves that differ from the shared core.]

## Licensed here
[Moves evidenced in this register's corpus that the shared core might otherwise sand out.]

## Still true here
[One line reasserting the cross-register hard lines most likely to drift.]
```

## Rules

- **Evidence-gated.** License a move only when finished work in this register demonstrates it.
- **Corpus wins.** When overlay prose and the corpus disagree, fix the overlay.
- **Channel-independent.** Corpus provenance may name a channel; the register name and guidance do not depend on it.
- **Thin overlays.** Shared traits stay in the core; output-shape rules stay in templates or channel profiles.
