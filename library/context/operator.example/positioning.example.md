# Positioning — [Operator name]

> **Placeholder file shipped with the open-source AgentFrame Marketing fork.** Copy `library/context/operator.example/` to `library/context/operator/` (gitignored) and fill in your own content. Sections below mirror the canonical positioning shape; replace bracketed prompts with your own answers.

The outward-facing strategic anchor for any user-voiced or strategic deliverable. Narrative + Content Pillars + Audience + POV Stances + Angles + Current Quarter Goals are the pieces that evolve through system retros and quarterly meta-retros.

This file is the "what am I saying outward?" surface; [`profile.example.md`](profile.example.md) is the "who am I?" surface (biography, role, active projects). Keep them apart — they have different cadences (positioning: per-campaign-retro; profile: rare).

---

## Narrative (the through-line every post should ladder up to)

**"[Your one-line narrative — the spine every piece of content reinforces.]"**

If a post doesn't ladder up to this, question it. The narrative is the spine; without it, posts read as generic commentary indistinguishable from any other voice in your category.

---

## Content Pillars

1. **[Pillar 1 name]** — [What this pillar covers. Be specific. Generic pillars produce generic content.]
2. **[Pillar 2 name]** — [Same.]
3. **[Pillar 3 name]** — [Same. Three is a soft default — fewer is sharper, more dilutes.]

---

## Audience

**Primary:** [Who you're writing for first. Demographics + psychographics + what they want.]

**Secondary:** [The narrower group whose engagement matters disproportionately.]

---

## POV Stances (the takes that make your content yours)

These are the contrarian or pointed positions that earn engagement. Refresh quarterly via system retro.

- **[Stance 1]** — [The take, in one sentence. Why you hold it.]
- **[Stance 2]** — [Same.]
- **[Stance 3]** — [Same.]

---

## Angles (the lenses you reach for repeatedly)

- **[Angle 1]** — [How you frame topics.]
- **[Angle 2]** — [Same.]

---

## Current Quarter Goals

Tracks what content is supposed to move the needle on this quarter. Referenced by `library/process/campaign-frontmatter.md` `quarterly_goals_advanced[]`.

| Goal | What "moved the needle" looks like |
|---|---|
| [Goal 1] | [Concrete, measurable, time-bound result.] |
| [Goal 2] | [Same.] |

---

## Anti-patterns (what NOT to do)

- [Patterns you've observed in your space that you don't want to imitate. Be specific — "no AI hype" is too vague; "no 'this changes everything' framing for incremental tooling" is sharper.]
- [Same.]

---

## Updates to this file

Patches log to `system/audit/agentframe.db` `system_changes`. Updates fire from:
- **System retro patch** — POV stances or narrative refinement when a campaign-cycle pattern surfaces them.
- **Quarterly meta-retro** — Current Quarter Goals table fully replaced; baseline updated.
- **Campaign retro insight** — angles list refined when an angle proves repeatedly effective or repeatedly missing.

Operator-self updates (role, employer, location, active projects) belong in `profile.md`, NOT here.
