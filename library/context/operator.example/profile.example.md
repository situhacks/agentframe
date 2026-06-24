# Profile — [Operator name]

> **Placeholder file delivered with the open-source AgentFrame Marketing fork.** Copy `library/context/operator.example/profile.example.md` to `library/context/operator/profile.md` (gitignored) and fill in your own content.

The operator-self file. Who you are in real life — biography, role, location, active projects, primary identity sources. Mostly static; updates are rare and almost always sync events from your primary source of truth, not authored content.

This file is the "who am I?" surface; [`positioning.example.md`](positioning.example.md) is the "what am I saying outward?" surface. Keep them apart — they have different cadences (this file: rare; positioning: per-campaign-retro).

---

## Identity

[Who you are. One paragraph. Name, role, location, the through-line that defines what you talk about and why anyone should listen. Specifics earn attention.]

[A second paragraph if you need to position the role you're performing publicly — practitioner / founder / investor / researcher / something else. Be honest about what you're not.]

---

## Primary source of truth

[Pointer to your durable identity notes — Obsidian, Notion, a private file. The agent loads profile from this file; the source of truth is yours to own.]

Anything in this file that's narrative-of-self should mirror what's in your primary source. If they diverge, the primary source wins; this file gets re-synced.

---

## Active Projects

- **[Project 1]** — [one-line description]
- **[Project 2]** — [one-line description]

[Pointer to project-specific state for current decisions and open questions, e.g. "For current state on either: read `{primary-source-path}/Projects/{project}/Summary.md`".]

---

## Updates to this file

This is the slowest-cadence operator file. Update only when:
- Role / employer / location actually changes (rare).
- A new active project starts or an existing one is killed (occasional).
- The "who am I" framing materially shifts (rare; if frequent, the framing wasn't true to begin with).

Patches log to `system/audit/agentframe.db` `system_changes`. Per-campaign positioning shifts (POV stances, angles, content pillars, quarterly goals) belong in `positioning.md`, NOT here.
