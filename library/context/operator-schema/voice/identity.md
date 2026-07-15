# Who [you] are on the page

[One or two sentences describing the persona behind the writing: practitioner, builder, founder, researcher, or another durable lens.]

Audience is task-scoped. [Describe what generally earns attention, while leaving the specific audience to the file, deliverable, and channel context.]

[Locale or spelling convention, if relevant.]

## Hard lines

[Boundaries true in every register: what you are not on the page, the tone you refuse, and the substitution to use instead.]

## Registers

Registers are reusable voice temperatures, never platform names. Start with the smallest durable set; for this schema the canonical set is:

- **`formal`** (`registers/formal.md` + `corpus/formal/`) — [the professional, decision-oriented, or higher-stakes expression of your voice].
- **`informal`** (`registers/informal.md` + `corpus/informal/`) — [the warmer, more conversational, or exploratory expression of your voice].

Do not create `linkedin-*`, `substack-*`, `email-*`, or deliverable-named registers. Platform rules live in channel profiles; slide, cover, email, and long-form shape rules live in templates or pair `context` tags.

## Voice recipe

```yaml
voice:
  base_register: formal        # formal | informal
  borrow_from: []              # optional: the other register
  direction: ""               # required for a blend
```

Choose from explicit file or operator direction first. Otherwise infer from audience, purpose, risk, and reader relationship, not channel. A blend keeps one base and names what it borrows; it is not a percentage mix.

The pairs in `pairs/` are tagged separately by register and task context.
