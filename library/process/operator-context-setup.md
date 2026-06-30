# Operator Context Setup

Builds the operator's personal context the first time. Reached when the agent goes to
load operator context (`library/context/operator/`) and finds a surface missing. Owns
the initial generation; per-surface updates afterward fire from their own cadences
(system retro, quarterly meta-retro, profile sync — each surface's schema documents its
own "Updates to this file" rules).

The operator context has four surfaces, each with a canonical shape under
[`library/context/operator-schema/`](../context/operator-schema/README.md). That schema
is the single source — the agent **generates** the operator's filled-in file from it;
there is no example to copy. The operator's files land in `library/context/operator/`,
which is gitignored (operator content stays private).

## Surfaces

| Surface | Schema | Output | How |
|---|---|---|---|
| Positioning | [`operator-schema/positioning.md`](../context/operator-schema/positioning.md) | `operator/positioning.md` | Interview the operator against the shape; write their answers, not the bracketed prompts. |
| Profile | [`operator-schema/profile.md`](../context/operator-schema/profile.md) | `operator/profile.md` | Same — slowest-cadence surface; mostly a sync pointer to their durable identity notes. |
| Design language | [`operator-schema/design-language.md`](../context/operator-schema/design-language.md) | `operator/design-language.md` | Confirm tokens/typography against the shape; keep token/slot names stable. |
| Voice | [`operator-schema/voice/`](../context/operator-schema/voice/README.md) | `operator/voice/` | Do not form-fill — run [`voice-setup.md`](voice-setup.md). Voice comes from real writing, not self-description. |

## Sequence

1. **Generate the three single-file surfaces.** For positioning, profile, and design
   language: read the schema, interview the operator against its sections (the bracketed
   prompts are the questions), and write `operator/{surface}.md`. Skip a surface the
   operator defers; generate it on first real need.
2. **Build voice via its own process.** Run [`voice-setup.md`](voice-setup.md) — it
   scaffolds `operator/voice/` from the voice schema and builds the voice from the
   operator's corpus + taste interview.
3. **Confirm load paths.** Once a surface exists in `operator/`, the Operator persona
   loads it from there (see `AGENTS.operator.md` Source-Of-Truth + Routing). The schema
   under `operator-schema/` is never loaded for project work — only for setup or when the
   shape itself changes.

## Updating a shape

Change the schema under `operator-schema/`, not a copy — there is no copy. The operator's
generated file is regenerated or hand-edited per that surface's own update rules; the
schema is the durable definition future generations follow.
