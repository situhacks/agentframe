# Register overlays — one file per channel register

A register overlay scopes the shared voice core to one channel (e.g. a formal-professional feed vs. a personal newsletter). `identity.md` names the registers and maps deliverable types to them; each register named there gets one overlay file here and one corpus folder in `../corpus/`.

An overlay carries only what DIFFERS from the shared core — the core (`voice-profile.md`, `anti-patterns.md`, `identity.md`) always applies underneath.

## Shape of an overlay file

```
# Register: {register-name}

[One paragraph: the channel, the audience, and the temperature relative to the
operator's other registers ("a notch more casual than X, not less"). Name the
ground-truth corpus piece: when a rule here and that corpus disagree, the corpus wins.]

**Corpus:** ../corpus/{register}/ — imitate these. The paragraph test: would this
paragraph sit naturally inside {the register's strongest piece}?

## Licensed here (each move evidenced by a shipped piece — do not sand these out)
[Moves the shared core would flag but this register's published work proves:
fragments, exclamations, humour, direct reader address, idioms, repetition
tolerance… One line each, with a verbatim example from the corpus.]

## Still true here (shared core applies)
[One line re-asserting the identity hard lines and any core rules drafts in this
register tend to violate.]

## Structure
[Header conventions, opener/closer rules, link placement, channel conventions.
Operator-only slots the agent leaves open instead of inventing content for:
[FILL] facts only the operator has · [POV] the operator's live phrasing ·
[NERD-NOD] cultural references only the operator brings · [WAR-STORY] a personal
anecdote the section needs but the agent cannot invent.]
```

## Rules

- **Evidence-gated licensing.** A move enters "Licensed here" only when a shipped piece in this register's corpus shows it — never because it would sound good.
- **Corpus wins.** When an overlay rule and the register's corpus disagree, the corpus is ground truth; fix the overlay.
- **Overlays stay thin.** Anything true in every register belongs in the shared core, not copied into each overlay.
