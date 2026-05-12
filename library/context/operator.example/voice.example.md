# Voice — [Operator name]

> **Placeholder file shipped with the open-source AgentFrame Marketing fork.** Copy `library/context/operator.example/` to `library/context/operator/` (gitignored) and fill in your own content. Sections below mirror the canonical voice shape; replace bracketed prompts with your own answers.

Mechanical voice rules for any user-voiced output (post copy, carousel-spec slide text, image prompts that include caption-style writing). Load this only when a deliverable's template says `Voice loading: yes`.

---

## Tone

- [Adjective, adjective, adjective. Three words that capture how you sound at your best.]
- [What you lead with. e.g. "Hook-first. Earn the right to talk about the solution by nailing the problem first."]
- [Length and density preferences. e.g. "Sentences over bullets in prose. Bullets in carousels where the format calls for them."]
- [Locale / spelling convention if relevant. e.g. "US English" / "Canadian English" / "UK English."]

---

## Mechanical Rules

- **Dashes:** [Your rule, or no rule. The humanizer skill catches em-dash storms automatically — see `system/skills/humanizer/SKILL.md`. Don't try to police this manually inside a draft.]
- **Self-reference:** [First person? Casual or formal? "I built X" not "We're proud to announce X."]
- **Numbers:** [Spell out vs numerals convention. e.g. "Spell out one through nine in prose. Numerals for 10+."]
- **Capitalization:** [Title case vs sentence case for headings.]
- **[Other mechanical rules you observe.]**

---

## Banned Words / Phrases

These trigger a humanizer-pass even before the canonical humanizer skill runs. Refresh quarterly.

- [Word/phrase you find yourself wanting to cut every time it appears.]
- [Same.]
- [Same.]

---

## Writing Style Examples

The agent loads these as "this is what your voice sounds like at its best." Pull from real published work; humanize them so they don't read as AI-derived.

### Example 1: [Brief context — what kind of post / format]

> [Paste a real post or excerpt that captures your voice cleanly.]

### Example 2: [Brief context]

> [Same.]

### Example 3: [Brief context]

> [Same. Three is a soft default; more breadth helps the agent triangulate.]
