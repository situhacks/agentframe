# What [you] don't write — weighted, not walls

Preferences with calibration, not prohibitions. Hard bans make the model write defensively and regress to generic; keep this file small and tiered, and put the generative effort into `pairs/` + `voice-profile.md`. Use this to notice drift, not to write scared.

Generic-AI patterns (rule-of-three, inflated significance, copula avoidance, AI vocab, signposting, etc.) are the vendored humanizer's job: `system/skills/humanizer/SKILL.md` — run it with a writing sample for calibration. This file covers only YOUR specific preferences.

## Hard lines (zero occurrences — risk and identity, not style)
[The very few true absolutes: brand-safety and identity lines where one occurrence costs something real. e.g. naming client work, punching down. Style preferences do NOT belong here.]

## Banned tics (zero occurrences — style, not identity)
[Specific recurring verbal habits the writer has ruled out outright: a phrase, an adverb, a rhetorical figure. This section exists because a STYLE rule with a zero budget has no other home — Hard lines above is identity and risk only, and Budgeted moves below allows N>0. Earn every entry: list only habits that actually recur in drafts, and give each one the tell plus what to write instead, so the model has somewhere to go rather than writing around the ban. Where a banned item is also prescribed elsewhere (a hedge listed in `voice-profile.md`, an example in `pairs/`), patch that file in the same pass — a ban that contradicts a file loaded in the same turn does not fire. Check the vendored humanizer first and note any shape its sweep does not catch.]

## The boundary beat — sweep this before anything else
[The highest-value section in this file, and the one most likely to be missing. Filler generated at structural boundaries — the end of a paragraph, the end of a section, the line before a list — where the prose reaches a rhythmic stop before the information does. It survives every revision, because a revision rewrites the substance and then generates a fresh beat to close it; the tic returns in new words, so a phrase blacklist cannot converge on it. List the flavours YOUR drafts produce (quotable maxim / restates-and-asserts-it-matters / certifies with stance / trailing "which is…" restatement / pre-list drumroll / section-opening announcement that the next part matters), then give the SWEEP: read the last sentence of every paragraph, the sentence before every list, and the first sentence of every section in isolation, and require each to carry a fact the reader did not already have. Close with what belongs in that position instead — for most writers, the question the next section answers, a concrete comparison, or nothing.]

## Mannered prose — say what you mean
[Mannered prose substitutes metaphor and flourish for direct statement: "a dial worth turning" for "a parameter worth varying", "this point earns its keep" for "this point still matters". The phrase exists to display the writer, not to convey the idea, and it drags in connotations the writer did not choose. The fix is to say what you mean; when a literal phrase is available, use it. Calibrate it to the writer: list the spoken commonplaces they actually use and keep (those count as literal in their mouth) against the crafted images their hand edits cut. The vendored humanizer's vocabulary list does not catch this shape, so sweep for it separately, apply it at generation time (README step 2) as well as in the clean pass, and anchor it with a worked pair in `pairs/`.]

## Budgeted moves (good once, a tell when repeated — counts per piece)
[Define a "piece" as one standalone prose deliverable or clearly bounded section. Let deliverable templates, not the register, license shape-specific exceptions such as slide or cover fragments.]

[Moves that are strong used deliberately but read as AI when they recur. Give each a per-piece budget and the carve-out. Examples to consider:]
- **[Contrastive negation ("not X — Y")]** — [e.g. ≤1 per piece, only when the negation is the pivot.]
- **[Short-beat / staccato run]** — [e.g. ≤1 per piece, payoff or closer only.]
- **[Punctuation habit]** — [e.g. em-dash: measure the writer's own rate off the corpus, date it, and state it as a drift alarm rather than a quota; the tell is replacement of most commas/periods. A cleanup pass never normalises punctuation the writer typed themselves — a spaced hyphen or a comma splice in a hand edit is theirs.]

## Substitution preferences (reach for this, not that)
[Frame the word list positively — what you reach for instead. e.g. plain mechanical verbs (built, delivered, wired) where marketing copy says leverage/unlock/empower.]

## The real test
A draft can respect every line above and still sound like generic AI. If any competent assistant could've written it, it hasn't reached the voice — go back to `pairs/` + `voice-profile.md` and regenerate (markers first, per the README), don't add another rule.
