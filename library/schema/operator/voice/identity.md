# Who [you] are on the page

[One or two sentences describing the persona behind the writing: practitioner, builder, founder, researcher, or another durable lens.]

Audience is task-scoped. [Describe what generally earns attention, while leaving the specific audience to the file, deliverable, and channel context.]

[Locale or spelling convention, if relevant.]

## Hard lines

[Boundaries true in every register: what you are not on the page, the tone you refuse, and the substitution to use instead.]

## Registers

Registers are reusable voice temperatures, never platform names. A voice has one or more registers, each a file in `registers/`; start with the smallest durable set. The usual two are:

- **`informal`** (`registers/informal.md` + `corpus/informal/`) — [the warmer, more conversational, or exploratory expression of your voice]. The default register.
- **`chat`** (`registers/chat.md`, no corpus needed) — [how you text a friend].

A `formal` register (`registers/formal.md` + `corpus/formal/`, [the professional, decision-oriented, or higher-stakes expression of your voice]) is optional: add it only when you write that way often enough to have pieces for it.

Do not create `linkedin-*`, `substack-*`, `email-*`, or deliverable-named registers. Platform rules live in channel profiles; slide, cover, email, and long-form shape rules live in templates or pair `context` tags.

## Voice recipe

```yaml
voice:
  base_register: informal      # the name of a file in registers/
  borrow_from: []              # optional: another register that has a file
  direction: ""               # required for a blend
```

Choose from explicit file or operator direction first. Otherwise infer from audience, purpose, risk, and reader relationship, not channel. A blend keeps one base and names what it borrows; it is not a percentage mix.

The pairs in `pairs/` are tagged separately by register and task context.
