# AgentFrame

> **A file-native project harness that runs inside your AI coding agent.** The harness is domain-neutral; everything it knows about a kind of work — marketing campaigns and project-management engagements today, anything else later — lives in a domain pack you add or swap. Two `AGENTS.md` modes carry the work: **Operator** runs projects, **Builder** evolves the system.

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" /></a>
  <img alt="Works with" src="https://img.shields.io/badge/works%20with-claude%20code%20%7C%20codex%20%7C%20cursor%20%7C%20vscode%20%7C%20antigravity-blue?style=flat-square" />
  <img alt="Status" src="https://img.shields.io/badge/status-actively%20used-orange?style=flat-square" />
</p>

This is the system I actually work in. I run real projects through it — paid work and personal projects both — and it changes as my workflow changes, not on a release schedule. It started as a marketing workspace; I kept using it for work that wasn't marketing, so I rebuilt it to handle any kind of project. It's free to fork. Take what's useful.

---

## Table of contents

- [Quick start](#quick-start)
- [Why this exists](#why-this-exists)
- [Domains are packs](#domains-are-packs)
- [A real project, step by step](#a-real-project-step-by-step)
- [What makes it a harness](#what-makes-it-a-harness)
- [At a glance](#at-a-glance)
- [Design principles](#design-principles)
- [Recommended connectors](#recommended-connectors)
- [Architecture](#architecture)
- [Repository structure](#repository-structure)
- [Preview server](#preview-server)
- [Auditability and state](#auditability-and-state)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [References and lineage](#references-and-lineage)
- [License](#license)
- [Contact](#contact)

---

## Quick start

1. **Clone the repo:**

   ```bash
   git clone https://github.com/situhacks/agentframe.git
   cd agentframe
   ```

2. **Open the folder in your coding agent** — Claude Code, Codex, Cursor, VS Code, Antigravity, anything that respects `AGENTS.md`. (Claude Code reads `CLAUDE.md`, which imports `AGENTS.md`, so the active persona pins itself and survives compaction.)

3. **Set up.** Copy `.env.example` to `.env` and drop in optional connector keys for Gemini and Composio. Both have generous free tiers and both are optional — without them the system still runs, you just lose Deep Research and direct publishing. If you plan to use Open Design locally, run `corepack pnpm install` inside `system/skills/open-design/source/`.

4. **Start a project.** Tell the agent **"start a new marketing campaign"** or **"start a new project-management engagement"** and run it end to end. Under the hood that is `af new-project <slug> --domain <marketing|project-mgmt>` — the engine reads the domain's pack and scaffolds the right shape.

### Mode swaps

AgentFrame ships with two `AGENTS.md` modes. You swap depending on what you are doing:

- **Operator runs a project** — drafting, producing media, delivering, doing a retro. Operator is scoped to `workspace/projects/`, so it cannot accidentally edit your templates or processes mid-project. It runs any domain, parameterized by the project's `domain`.
- **Builder improves the system** — editing a template, adding a process, authoring a domain pack, swapping a skill, applying retro patches. Builder is scoped to `system/` and `library/`.

You do not run shell commands by hand. Tell the agent `swap to Builder` or `swap to Operator`; it handles the file swap and logs the transition to the audit DB.

### Updating your copy

This repo keeps evolving after you clone it. To pull updates into your customized copy, swap to Builder and tell the agent **"pull upstream updates"**. It fetches this repo, walks you through what changed commit by commit with a recommendation for each, re-applies your own customizations on top where they collide, and asks before anything is written. Your personal layer — operator context, live projects, backlog, audit history — is gitignored and never touched by a sync.

[Back to top](#agentframe)

---

## Why this exists

I built the first version of this for marketing — running campaigns out of a coding agent instead of losing the thread across chat sessions. It worked, so I kept reaching for it. Project-management work went in first, then other things that weren't marketing at all. For a while I forced that work into a marketing-shaped system and patched around the edges.

At some point the patching was the problem. The system needed to stop assuming marketing. So I rebuilt it: the harness went domain-neutral, and everything that knew about a specific kind of work moved into a domain pack the engine reads. Now adding a new kind of work means adding a pack, not rebuilding the system.

The shape isn't novel — it's where the industry is going. Claude for Financial Services, Claude for small business: one base, domain packs on top. AgentFrame is that pattern for how I work. Marketing and project management are the first two packs.

The marketing-only original lives on, frozen, at [agentframe-marketing](https://github.com/situhacks/agentframe-marketing). This is its multi-domain successor.

It stands on Composio, Gemini Deep Research and image generation, Open Design, HyperFrames, PPT Master, and the humanizer — wired up under [Recommended connectors](#recommended-connectors) and credited in [References and lineage](#references-and-lineage).

[Back to top](#agentframe)

---

## Domains are packs

This is the idea the whole system hangs on.

The spine (`system/af.py`) and the router (`AGENTS.md`) carry zero domain knowledge. Everything that knows "marketing" or "project-management" lives in `library/domains/{domain}/` as a pack the generic engine reads. Grep `af.py` for `post_manifest`, `linkedin`, or any domain word and nothing comes back.

These are not skill bundles in the Claude-for-Finance sense. An AgentFrame pack is mostly deliverables — the templates and rules for the artifacts a domain produces — plus a small descriptor. The generic skills (research, voice, media, harvest) sit outside the packs, and any pack can tap them.

A pack declares what its domain adds:

| The pack ships | What it is |
|---|---|
| **Deliverable templates** (`deliverables/`) | the domain's artifact types |
| **A descriptor** (`pack.md`) | the frontmatter fields the domain adds, which verbs apply, the folder prefix — parsed by the spine's existing stdlib helpers, with no new config language |
| **A scaffold skeleton** (`skeleton.md`) | the `project.md` shape `new-project` writes for this domain |
| **An optional rules module** (`rules.py`) | genuine domain logic, imported by the spine if present — marketing's post-FINAL assembly and post-counting live here, fail-loud and isolated if they error |
| **An optional routing fragment** (`production.md`) | the domain-specific production workflow the Operator lazy-loads |

Adding a domain is a new folder under `library/domains/`: author the pack, never touch the engine. The acceptance test for the whole design is simple — a third domain touches only its own pack folder, and `af.py` zero times.

[Back to top](#agentframe)

---

## A real project, step by step

The same spine runs every domain; what changes is the pack. Here is what an end-to-end run looks like in each of the two domains that ship today — one operator, six moves, no handoffs.

### Marketing — a campaign

A compact walkthrough using the example project at `workspace/projects/example-ai-automation-pov/`.

<table>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-01-cmo-kickoff.png" alt="01 · Operator kickoff" /><br/>
<sub><b>01 · Operator kickoff</b> — Tell your agent <code>start a new marketing campaign</code>. Operator reads your profile, scaffolds the campaign from the marketing pack's skeleton, and calls Composio for workplace context — recent emails, meeting notes, doc activity — so the work starts from what you care about this week, not a cold prompt.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-02-research.png" alt="02 · Gemini Deep Research" /><br/>
<sub><b>02 · Gemini Deep Research</b> — Deep Research runs against your direction and lands a structured artifact at <code>phase-1-research/research-artifact-v{N}.md</code>.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-03-post-copy.png" alt="03 · Copy in your voice" /><br/>
<sub><b>03 · Copy in your voice</b> — Drafts inherit your voice system from <code>library/context/operator/voice/</code>, then run through the humanizer before lock. Every revision snapshots as its own <code>-v{N}.md</code>, so you can roll back, compare, or read why the copy changed.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-04-image-production.png" alt="04 · Media, your pick" /><br/>
<sub><b>04 · Media, your pick</b> — HTML render in your agent for slide-shaped visuals, Gemini Nano Banana for raster variants, Open Design for higher-fidelity decks and carousels, or HyperFrames for HTML-to-video. For Open Design, AgentFrame stages the project for you — design language, mode, and first prompt already loaded.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-05-published.png" alt="05 · Publish via Composio" /><br/>
<sub><b>05 · Publish via Composio</b> — Composio's connector to LinkedIn (or X, Instagram, TikTok) sends or schedules the post; <code>af publish</code> records the delivered state and live URL back into the post folder.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-06-retro.png" alt="06 · Retro" /><br/>
<sub><b>06 · Retro</b> — The agent proposes patches to your voice, templates, processes, and skills based on what actually happened. You approve or reject each one; the library evolves.</sub>
</td>
</tr>
</table>

### Project management — an engagement

The same spine off the PM pack: a charter in, governance docs derived, deliverables produced ad hoc.

<!-- Project-management walkthrough imagery is a placeholder — I'm generating these myself.
     Planned files: .github/readme-assets/pm-walkthrough-01-kickoff.png … pm-walkthrough-06-retro.png -->

<table>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-01-kickoff.png" alt="01 · Operator kickoff" /><br/>
<sub><b>01 · Operator kickoff</b> — Tell your agent <code>start a new project-management engagement</code>. Operator scaffolds the engagement from the PM pack's skeleton and ingests your charter / SOW into the project's <code>sources/</code>.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-02-governance.png" alt="02 · Derive the governance docs" /><br/>
<sub><b>02 · Derive the governance docs</b> — From the charter, the agent derives the four living documents into <code>knowledge/</code>: a RAID log, a stakeholder map, a decision log, and a workback schedule.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-03-living.png" alt="03 · Keep them living" /><br/>
<sub><b>03 · Keep them living</b> — These are not one-time artifacts. The RAID log moves on a weekly cadence, decisions append as they are made, and the schedule re-plans against its deadlines.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-04-deliverables.png" alt="04 · Ad-hoc deliverables in your voice" /><br/>
<sub><b>04 · Ad-hoc deliverables in your voice</b> — Findings, recommendations, decks, memos: each an instance of the generic deliverable shape, drafted in your voice and versioned the same way marketing copy is.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-05-deliver.png" alt="05 · Deliver" /><br/>
<sub><b>05 · Deliver</b> — No posts and no publish here. Deliverables lock and version, then hand off; <code>af publish</code> is correctly rejected for this domain.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-06-retro.png" alt="06 · Retro" /><br/>
<sub><b>06 · Retro</b> — The same harvest as marketing: the agent proposes improvements to templates, processes, and skills from how the engagement actually ran.</sub>
</td>
</tr>
</table>

[Back to top](#agentframe)

---

## What makes it a harness

Three systems separate AgentFrame from a folder of prompt files.

### The deterministic spine

A model will write all day and still forget to update its own records. Benchmarks have frontier models breaking exact bookkeeping rules 30–90% of the time, and my own agent skipped its lock step three times in one night. So project state does not run on the model remembering procedure — it runs through `system/af.py`. The generic buttons (`new-project`, `lock`, `version`, `doctor`) each do their bookkeeping in one atomic step — frontmatter, tracker, activity trail — and print back the judgment checklist the agent still owns. Domain-specific steps, like marketing's post-FINAL assembly and the `publish` verb, dispatch into the active pack, so the spine itself names no domain. `doctor` audits the books and never fixes them for you. It is plain stdlib Python, so it behaves identically in Claude Code, Cursor, Codex, or Antigravity.

### A voice system built from your own writing

Most brand-voice setups are a rules file the agent has forgotten by the third draft. AgentFrame compiles your actual writing into annotated pairs — a generic version, your version, and the move that separates them — grouped by register. Drafting starts from those pairs: pull three or four concrete markers, write the content pass, then run a separate style pass with the markers required. Anti-patterns are weighted preferences with a per-piece budget, not flat bans. A humanizer pass runs last, before anything locks. One voice, shared across every domain.

### A learning loop

Every project closes with a harvest. Two skills read the version trails and your edits: one mines new voice pairs from what you changed, the other mines template and process improvements from how the work actually ran. You approve each patch, and the library gets a little better. The builder backlog and audit DB keep the receipts — every major feature traces back to a real failure from a real run.

[Back to top](#agentframe)

---

## At a glance

Two domain packs, 5 shared deliverables, 16 process files, 16 skill bundles, 3 flows, a two-mode persona model, a deterministic state-transition CLI, a local preview server, and a two-layer audit trail (`activity.md` + SQLite DB).

Everything in the library and skills layer is meant to be edited. Set voice and positioning once in `library/context/operator/` (copy from `operator.example/` on first run) and reuse them everywhere.

### Domain packs

| Pack | What it ships |
|---|---|
| `library/domains/marketing/` | Campaigns that ship posts. Deliverables: research, business brief, campaign brief, campaign architecture, slide-copy, body-copy, post-final. The `publish` verb and the post-FINAL assembly live here. |
| `library/domains/project-mgmt/` | Consulting / PM engagements. A charter / SOW goes in, then four living governance docs are derived from it: RAID log, stakeholder map, decision log, workback schedule. No posts, no publish. |

### Flows

`open-flow` is the default across every domain. Add or edit any flow under `library/process/flows/`.

| Flow | Purpose |
|---|---|
| `open-flow` (default) | Build-as-you-go. The agent proposes a plan scaled to the objective; you narrow it and set the tempo. The default for every domain. |
| `marketing-solo-flow` | Marketing, opt-in: a lean fixed phase ladder with one accountable owner. |
| `marketing-standard-flow` | Marketing, opt-in: a fuller campaign with stakeholder review gates. |

### Shared deliverables

Cross-domain templates under `library/deliverables/`.

| Template | Output |
|---|---|
| `design-language` | Visual and style direction |
| `image-prompts` | Per-asset generation prompts — treatment block + per-asset deltas |
| `video-spec` | Video concept, scenes, and production plan |
| `closeout-retro` | Project-level learnings and improvements |
| `system-retro` | System-level process and architecture improvements |
| `_meta` | The generic deliverable shape every template inherits |

### Process files

Process files load on demand — only when the workflow they describe is in play.

| Process | Purpose |
|---|---|
| `flow-authoring` | Designing or evolving flows |
| `process-authoring` | Designing or evolving process files |
| `research-and-signals` | Shared kickoff research: workspace context, live MCP scan, research-method offer |
| `deliverable-versioning` | Surgical-vs-replacement judgment for `*-v{N}.md`; bumps run through `af version` |
| `lock-event` | Lock trigger and judgment gates; mechanics run through the `af` CLI |
| `project-frontmatter` | Frontmatter schema and state handling |
| `voice-setup` | Build your voice system from your own writing |
| `voice-mini-retro` | Lock-time gate that routes your edit-diffs to the voice-harvest skill |
| `humanizer-integration` | The humanization pass |
| `image-production` | Image-generation workflow |
| `video-production` | Video workflow from spec to renders |
| `deck-production` | Deck path selection (PPTX skill, PPT Master, Open Design) |
| `preview-server` | When and how to use the local preview hub |
| `composio-notes` | Connector usage notes and caveats |
| `browser-fallback` | Browser-automation fallback strategy |
| `substack-distribution-notes` | Substack publishing conventions |

### Skills

My current production stack — swap any one for a sharper tool without touching templates or processes.

| Skill | Source |
|---|---|
| `agentframe-structure` | Project skill |
| `deliverable-scaffolding` | Project skill |
| `system-improvement` | Project skill |
| `upstream-sync` | Project skill — pulls upstream updates into your copy, commit by commit with approval |
| `voice-harvest` | Project skill — mines finished work and edit-diffs into voice example pairs |
| `deliverable-harvest` | Project skill — mines template and process patches from finished projects |
| `docx` | Project skill |
| `pptx` | Project skill |
| `humanizer` | Vendored from [blader/humanizer](https://github.com/blader/humanizer) |
| `hyperframes` | Vendored from [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes) |
| `hyperframes-cli` | Vendored from [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes) |
| `gsap` | Vendored animation skill for HyperFrames workflows |
| `ppt-master` | Vendored from [hugohe3/ppt-master](https://github.com/hugohe3/ppt-master) — native-editable deck generation via SVG→PPTX |
| `extract-design` | Thin skill over [Manavarya09/design-extract](https://github.com/Manavarya09/design-extract); CLI runs via `npx designlang` |
| `open-design` | Vendored local-first runtime from [nexu-io/open-design](https://github.com/nexu-io/open-design) for image/deck production |
| `browser-harness` | Vendored CDP-driven browser workflows via Edge |

### Everything else in the box

- Two-mode routing via `AGENTS.operator.md` and `AGENTS.builder.md`.
- The deterministic CLI at `system/af.py` — `new-project`, `lock`, `version`, `doctor`, plus the domain-dispatched `publish`. The buttons do the bookkeeping atomically and write the paper trail; the agent keeps the judgment.
- Project state and outputs as markdown under `workspace/projects/`.
- A two-layer audit trail: `activity.md` per project plus an append-only SQLite DB at `system/audit/agentframe.db`.
- A local preview server at `system/server/` for HTML, image, video, PDF, PPTX, and DOCX.
- A browser harness at `system/browser/` for CDP-driven sessions, with documented fallbacks when a workflow needs a hand.

[Back to top](#agentframe)

---

## Design principles

### File-native state

Project state lives in markdown — frontmatter, deliverables, `activity.md` — not in a chat window. Change models, change machines, come back next week, and the project picks up where it left off.

### Token efficiency by default

`AGENTS.md` is the only always-on router; flows, processes, templates, packs, and skills load on demand. A small, focused context means longer sessions and less drift.

### A durable library, swappable tools

Templates, processes, flows, packs, and personas are the layer that improves over time and the part worth keeping. Skills and connectors are swappable — replace one when something sharper ships and the system is untouched.

[Back to top](#agentframe)

---

## Recommended connectors

External services AgentFrame integrates with. Recommended for the full loop, but all optional — both Gemini and Composio have generous free tiers, and the system still runs without them.

### Gemini Deep Research

- Deep research artifacts at project start — sources, implications, signals — saved as structured markdown under `phase-1-research/`.
- Free credits from [Google AI Studio](https://aistudio.google.com) cover solo-operator usage. Key: `GEMINI_API_KEY`.

### Gemini image generation (Nano Banana 2 / Pro)

- Fast A/B/C variants (Nano Banana 2: `gemini-3.1-flash-image-preview`) and high-fidelity hero or text-in-image visuals (Pro: `gemini-3-pro-image-preview`).
- Routed through `system/server/lib/image_generate.py`. Shares the `GEMINI_API_KEY`.

### Composio

- All-in-one MCP hub. One connection exposes 100+ tools (Gmail, Calendar, Drive, LinkedIn, X, Instagram, TikTok, and more).
- Workplace signal collection feeds research and retros; publishing schedules or sends directly; analytics pull back into retros.
- Get started at [composio.dev](https://composio.dev).

### Open Design

- Bundled local-first visual runtime at `system/skills/open-design/source/` for higher-fidelity images, carousels, and decks.
- Fresh clones may need runtime setup (`corepack`, `pnpm install`, Node 24). Uses a local code-agent CLI on `PATH`, or BYOK provider keys as a fallback.

### `.env` shape

```bash
GEMINI_API_KEY=
COMPOSIO_API_KEY=
COMPOSIO_MCP_URL=https://connect.composio.dev/mcp
```

Your coding agent provides the LLM. These keys power the non-LLM tools — research, image generation, and publishing.

[Back to top](#agentframe)

---

## Architecture

### Mode boundary

```text
                                  ┌─── owns ──▶  workspace/projects
                  ┌──── Operator ─────┤
                  │              └─── reads ─▶  system + library
   Operator ──────┤
                  │              ┌─── owns ──▶  system + library
                  └─── Builder ──┤
                                 └─── reads ──▶  workspace/projects

   ── swap AGENTS.md to flip between modes ──
```

### The load path — one domain parameter, one resolution point

```text
   CLAUDE.md  ── imports ──▶  AGENTS.md   (the active persona; domain-agnostic)
                                  │
                                  ▼
                          project.md  →  { domain, flow, deliverables[] }
                                  │
              ┌───────────────────┼────────────────────────────┐
              ▼                   ▼                             ▼
   library/domains/{domain}/   library/process/flows/{flow}.md   library/process/* (shared)
        (the pack)              (domain-agnostic; open-flow)      (lock, versioning, frontmatter)
              │
              ▼
   draft X → pack's template ▸ shared template ▸ generic _meta shape
```

`af.py` reads `project.md` → `domain` → loads the pack and dispatches. The persona, the flows, and the shared processes name no domain; `{domain}` is the single point where the work is parameterized.

[Back to top](#agentframe)

---

## Repository structure

```text
agentframe/
├── CLAUDE.md                 # thin shim → imports AGENTS.md (pins the persona)
├── AGENTS.md                 # active persona (Operator | Builder)
├── AGENTS.operator.md
├── AGENTS.builder.md
├── README.md
├── .env.example
├── library/
│   ├── domains/
│   │   ├── marketing/        # pack.md, skeleton.md, rules.py, production.md, deliverables/
│   │   └── project-mgmt/     # pack.md, skeleton.md, deliverables/
│   ├── deliverables/         # shared cross-domain deliverables + _meta shape
│   ├── process/
│   │   └── flows/
│   └── context/operator.example/
├── system/
│   ├── af.py                 # the generic plugin-host spine
│   ├── skills/
│   ├── server/
│   ├── audit/
│   └── builder-backlog.md
└── workspace/
    └── projects/
        └── example-ai-automation-pov/
```

[Back to top](#agentframe)

---

## Preview server

<details>
<summary>Show preview server details</summary>

- Local preview hub at `system/server/`.
- Previews HTML, images, video, PDF, PPTX, and DOCX.
- Folder-tree navigation with hide rules to keep noise down.
- Run with `py -3 system/server/run.py`.

</details>

## Auditability and state

<details>
<summary>Show auditability details</summary>

- Project layer: `activity.md` in each project for the human-readable timeline — state transitions through `system/af.py` write it automatically.
- System layer: append-only SQLite audit DB at `system/audit/agentframe.db` (writer: `system/audit/writer.py`).
- Books check: `python system/af.py doctor [project]` verifies schema, head pointers, and per-domain rules; it surfaces drift and never auto-fixes.

</details>

[Back to top](#agentframe)

---

## Roadmap

Small, demand-driven additions. Nothing here is a rewrite.

- [ ] Embedded / RAG indexing over the markdown corpus, so retrieval scales as the library grows past what fits in context.
- [ ] Scheduled runs — cron-triggered jobs for recurring work, like a weekly research pull or a digest.
- [ ] Agent-to-agent communication over an API, so projects and agents can hand work to each other.
- [ ] Per-project knowledge substrate (`sources/` immutable + agent-owned `knowledge/`) with a user-triggered consolidation pass.
- [ ] Cross-project context entities (channels, people) referenced by slug.
- [ ] More domain packs as the work demands them.
- [ ] Preview server v2: better search, nested live reload, stronger video UX.

[Back to top](#agentframe)

---

## Contributing

- PRs for templates, processes, packs, and skills are welcome.
- Open an issue first for major architecture changes.

## References and lineage

- [agentframe-marketing](https://github.com/situhacks/agentframe-marketing) — the frozen marketing-only predecessor this grew out of
- [Composio](https://composio.dev)
- [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes)
- [nexu-io/open-design](https://github.com/nexu-io/open-design) (Apache-2.0, vendored under `system/skills/open-design/source/`)
- [hugohe3/ppt-master](https://github.com/hugohe3/ppt-master) (MIT, vendored under `system/skills/ppt-master/`)
- [Manavarya09/design-extract](https://github.com/Manavarya09/design-extract) (MIT, thin skill; CLI runs via `npx designlang`)
- [GreenSock GSAP](https://greensock.com/gsap/)
- [Google AI Studio / Gemini](https://aistudio.google.com)
- [blader/humanizer](https://github.com/blader/humanizer)

## License

MIT. See [`LICENSE`](LICENSE).

## Contact

Built by Brandon Situ over many weekends — and likely many more.

- LinkedIn: [linkedin.com/in/brandonsitu](https://www.linkedin.com/in/brandonsitu/)
- Email: brandonzsitu@gmail.com

[Back to top](#agentframe)
