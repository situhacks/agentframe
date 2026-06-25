# AgentFrame

> **A domain-neutral project harness inside your AI coding agent.** File-native. Built for solo operators. One spine runs marketing campaigns, project-management engagements, and whatever domain you add next — **adding a domain is adding a pack, not rebuilding the system.** Two `AGENTS.md` modes carry the work: **Operator** runs the project, **Builder** evolves the system.

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" /></a>
  <img alt="Works with" src="https://img.shields.io/badge/works%20with-claude%20code%20%7C%20codex%20%7C%20cursor%20%7C%20vscode%20%7C%20antigravity-blue?style=flat-square" />
  <img alt="Status" src="https://img.shields.io/badge/status-actively%20used-orange?style=flat-square" />
</p>

**Jump to:** [Quick start](#quick-start) · [Why](#why-this-exists) · [Domains are packs](#domains-are-packs) · [Walkthrough](#a-real-project-step-by-step) · [Harness](#what-makes-it-a-harness) · [Architecture](#architecture) · [Roadmap](#roadmap)

---

## Table of contents

- [Quick start](#quick-start)
- [Why this exists](#why-this-exists)
- [Domains are packs](#domains-are-packs)
- [A real project, step by step](#a-real-project-step-by-step)
- [What makes it a harness](#what-makes-it-a-harness)
- [At a glance](#at-a-glance)
- [Architectural principles](#architectural-principles)
- [Recommended connectors](#recommended-connectors)
- [Architecture](#architecture)
- [Repository structure](#repository-structure)
- [Preview server](#preview-server)
- [Auditability and state](#auditability-and-state)
- [Roadmap](#roadmap)
- [Status](#status)
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

2. **Open the folder in your coding agent** — Claude Code, Codex, Cursor, VS Code, Antigravity, anything that respects `AGENTS.md`. (Claude Code reads `CLAUDE.md`, which imports `AGENTS.md` — so the active persona pins and survives compaction.)

3. **Setup.** Copy `.env.example` to `.env` and drop in optional connector keys for Gemini and Composio (both have generous free tiers and are optional — the system still runs without them, you just lose Deep Research and direct publishing). If you plan to use Open Design locally, run `corepack pnpm install` inside `system/skills/open-design/source/`.

4. **Start a project.** Tell the agent **"Start a new marketing project"** or **"Start a new project-management engagement"** and run it end-to-end. Under the hood that's `af new-project <slug> --domain <marketing|project-mgmt>` — the engine reads the domain's pack and scaffolds the right shape.

### Mode swaps

AgentFrame ships with two `AGENTS.md` modes. You swap depending on what you're doing:

- **Swap to Operator when you're running a project** — drafting, producing media, delivering, doing a retro. Operator is scoped to `workspace/projects/` so it can't accidentally edit your templates or processes mid-project. Operator runs **any** domain, parameterized by the project's `domain` ("CMO" is the label it wears when `domain: marketing`).
- **Swap to Builder when you're improving the system itself** — editing a template, adding a process, authoring a domain pack, swapping a skill, applying retro patches. Builder is scoped to `system/` and `library/`.

You don't run shell commands by hand. Just tell the agent `swap to Builder` or `swap to Operator`. It handles the file swap and logs the transition to the audit DB.

### Updating your copy

This repo keeps evolving after you clone it. To pull updates into your customized copy, swap to Builder and tell the agent **"pull upstream updates"**. It fetches this repo, walks you through what changed commit by commit with a recommendation per change, re-applies your own customizations on top where they collide, and asks before anything is written. Your personal layer — operator context, live projects, backlog, audit history — is gitignored and never touched by a sync.

[Back to top](#agentframe)

---

## Why this exists

I started by running my marketing campaigns out of Claude Chat. Write the post here, paste the voice rules there, ask for a rewrite, lose the thread, start over tomorrow. It works for something small, but for multi-post campaigns things get lost and context degrades. The voice rules I'd "saved" were forgotten by next session. State lived in scrollback.

So I built a file-native marketing workspace inside the coding agent I already use — and dogfooded it through many revisions. Then a pattern showed up: the bones weren't marketing-specific. The deterministic spine was flow-agnostic, state was just files, the persona was a generic router. Only a handful of templates and a few hard-coded assumptions actually knew the word "marketing." I was already running non-marketing project work through it, jerry-rigged as "campaigns."

So I generalized it. **AgentFrame** is the result: marketing is now one **domain pack**, project-management is another, and adding a domain means authoring a pack — the engine never changes. It's not a marketing tool with a project-management bolt-on; it's a domain-neutral harness where marketing happens to be the first pack. It's how I actually work across domains today.

It stands on excellent shoulders — Composio, Gemini Deep Research and image generation, Open Design, HyperFrames, the humanizer — wired up under [Recommended connectors](#recommended-connectors) and credited in [References and lineage](#references-and-lineage).

> **The marketing-only predecessor** lives on as a frozen, complete artifact at [agentframe-marketing](https://github.com/situhacks/agentframe-marketing) (v1). AgentFrame is its multi-domain successor.

[Back to top](#agentframe)

---

## Domains are packs

This is the idea the whole system hangs on.

The spine (`system/af.py`) and the router (`AGENTS.md`) carry **zero domain knowledge**. Everything that knows "marketing" or "project-management" lives in `library/domains/{domain}/` as a **pack** the generic engine reads. A `grep` of `af.py` for `post_manifest`, `post-FINAL`, `linkedin`, or any domain word comes back empty — the engine is a host, the pack is the plugin.

A pack declares what its domain adds:

| The pack ships | What it is |
|---|---|
| **Deliverable templates** (`deliverables/`) | the domain's artifact types |
| **A descriptor** (`pack.md`) | the frontmatter fields the domain adds, which verbs apply, the folder prefix — parsed by the spine's existing stdlib helpers (no new config language) |
| **A scaffold skeleton** (`skeleton.md`) | the `project.md` shape `new-project` writes for this domain |
| **An optional rules module** (`rules.py`) | genuine domain logic (e.g. marketing's post-FINAL assembly and post-counting) — imported by the spine if present, fail-loud and isolated if it errors |
| **An optional routing fragment** (`production.md`) | the domain-specific production workflow the Operator lazy-loads |

**Two domains ship today:**

- **`marketing`** — campaigns that ship posts. Deliverables: research, business brief, campaign brief, campaign architecture, slide/body copy, image prompts, post-final. The `publish` verb and the post-FINAL assembly are marketing-only, living entirely in the pack.
- **`project-mgmt`** — consulting / PM engagements. Starts from a **charter / SOW** and derives the living governance docs from it: **RAID log**, **stakeholder map**, **decision log**, **workback schedule**. No posts, no publish — `publish` is correctly rejected for this domain.

**Shared across every domain:** the deterministic spine, the one global voice system, the flows, the cross-domain deliverables (design language, video spec, image prompts, closeout/system retros), the preview server, and the audit trail. Adding a new domain — job search, content, ops, whatever — is a new folder under `library/domains/`. The acceptance test for the whole design: *a third domain touches only its pack folder, and `af.py` zero times.*

[Back to top](#agentframe)

---

## A real project, step by step

A compact marketing-domain walkthrough using the example project at `workspace/projects/example-ai-automation-pov/`. One operator, six moves, no team handoffs. (A project-management engagement runs the same spine off the PM pack — charter in, governance docs derived, deliverables produced ad-hoc.)

1. **Operator kickoff** — Tell your coding agent `start a new marketing project`. Operator reads your operator profile, scaffolds the project folder from the marketing pack's skeleton, and calls Composio to pull workplace context — recent emails, meeting notes, doc activity — so the work starts from what you actually care about that week, not a cold prompt.
2. **Gemini Deep Research** — Deep Research runs against your chosen direction and lands a structured artifact at `phase-1-research/research-artifact-v{N}.md`.
3. **Copy in your voice** — Drafts inherit your voice system from `library/context/operator/voice/` (identity, anti-patterns, a profile, and annotated example pairs), then run through the humanizer skill before lock to strip AI tells. Every revision snapshots as its own `-v{N}.md` so you can roll back, compare, or read why the copy changed.
4. **Media, your pick** — HTML render in your coding agent for slide-shaped visuals, Gemini Nano Banana for raster variants, Open Design for higher-fidelity decks and carousels, or HyperFrames for HTML-to-video. For Open Design, AgentFrame stages the project for you — design language, mode, and first prompt already loaded.
5. **Delivered via Composio** — Composio's connector to LinkedIn (or X, Instagram, TikTok) sends or schedules the post; `af publish` records the delivered state and live URL back into the post folder.
6. **Retro** — The agent proposes patches to your voice, templates, processes, and skill behavior based on what actually happened. You approve or reject each one; the library evolves.

[Back to top](#agentframe)

---

## What makes it a harness

Three systems separate AgentFrame from a folder of prompt files.

### The deterministic spine

Models are strong writers and weak clerks — benchmarks have frontier models failing exact bookkeeping constraints 30–90% of the time, and my own agent skipped its lock procedure three times in one night. So project state transitions don't rely on the model remembering procedure: they run through `system/af.py`. The generic buttons — `lock`, `version`, `new-project`, `doctor` — each do their bookkeeping atomically (frontmatter, tracker, activity trail) and print back the judgment checklist the agent still owns. Domain-specific steps (marketing's post-FINAL assembly, the `publish` verb) are dispatched into the active pack's `rules.py`, so the spine itself names no domain. `doctor` audits the books and never auto-fixes. It's plain stdlib Python, so the spine works identically in Claude Code, Cursor, Codex, or Antigravity.

### A voice system that generates, not corrects

Most "brand voice" setups are a rules file the agent forgets by the third draft. AgentFrame compiles your actual writing into annotated contrastive pairs — generic version, your version, and the move that separates them — grouped by register. Drafting starts *from* the pairs: extract three or four concrete markers, write the content pass, then a separate style pass with those markers mandated. Anti-patterns are weighted preferences with per-piece budgets, not flat bans. A humanizer pass runs last, before anything locks. One voice, global across every domain.

### A learning loop with teeth

Every project closes with a harvest. Two skills read the version trails and your edit-diffs: one mines new voice pairs from what you actually changed, the other mines template and process patch candidates from how the work actually ran. You approve each patch, the library evolves, and the builder backlog plus audit DB keep the receipts. Every major feature traces back to a logged failure from a real run.

[Back to top](#agentframe)

---

## At a glance

A two-mode persona model, a deterministic state-transition CLI, a local preview server, a two-layer audit trail (`activity.md` + SQLite DB), one global voice system, and a domain-pack architecture with two domains shipped.

Everything in the library and skills layer is meant to be edited. Set voice and positioning once in `library/context/operator/` (copy from `operator.example/` on first run) and reuse them everywhere.

### Domain packs

| Pack | Ships |
|---|---|
| `library/domains/marketing/` | the post deliverables (post-final, slide/body copy, campaign brief/architecture, research, business brief), the `publish` verb + post-FINAL assembly (`rules.py`), and the production routing |
| `library/domains/project-mgmt/` | charter/SOW + the four living governance docs (RAID, stakeholder map, decision log, workback schedule); no posts, no publish |

### Flows

`open-flow` is the default across every domain. Add or edit any flow under `library/process/flows/`.

| Flow | Purpose |
| --- | --- |
| `open-flow` (default) | Build-as-you-go — the agent proposes a plan scaled to the objective; the operator narrows and sets the tempo. The standard for every domain. |
| `marketing-solo-flow` | Marketing, opt-in: lean fixed phase ladder, one accountable owner |
| `marketing-standard-flow` | Marketing, opt-in: fuller campaign with stakeholder review gates |

### Shared deliverables

Cross-domain, under `library/deliverables/`: `design-language`, `video-spec`, `image-prompts`, `closeout-retro`, `system-retro`, plus the generic deliverable shape in `_meta/`.

### Skills

My current production stack — swap any for a sharper tool without touching templates or processes: `agentframe-structure`, `deliverable-scaffolding`, `system-improvement`, `upstream-sync`, `voice-harvest`, `deliverable-harvest`, `docx`, `pptx`, plus vendored `humanizer`, `hyperframes`, `gsap`, `ppt-master`, `extract-design`, `open-design`, `browser-harness`. See [References and lineage](#references-and-lineage) for sources.

[Back to top](#agentframe)

---

## Architectural principles

### P1 — Add a domain = add a pack

The spine and the router are domain-agnostic. A domain's knowledge lives in `library/domains/{domain}/` as data the generic engine reads. Adding a domain authors a pack; it never edits the engine. This is the load-bearing one — everything else serves it.

### P2 — File-native state

Project state lives in markdown: frontmatter, deliverables, `activity.md`. Not in a chat window. Change models, change machines, come back next week — the project picks up where it left off.

### P3 — Token efficiency at its core

`AGENTS.md` is the only always-on router; flows, processes, templates, packs, and skills load on demand. Small focused context means longer sessions and less drift.

### P4 — The library is the product

Templates, processes, flows, packs, and personas are the durable layer that improves over time. Skills and connectors are swappable — replace one when something sharper ships; the system is untouched.

### P5 — Two modes, one operator

Operator runs the project (any domain). Builder evolves the system. Operator can't accidentally edit `library/` mid-project, and Builder can't accidentally touch a locked deliverable mid-refactor.

### P6 — Buttons own mechanics, prose owns judgment

State changes run through the deterministic spine and write their own paper trail. The model never hand-edits project state; you can always reconstruct what happened and why.

[Back to top](#agentframe)

---

## Recommended connectors

External services AgentFrame integrates with. Recommended for the full loop, but all optional — both Gemini and Composio have generous free tiers, and the system still runs without them.

### Gemini Deep Research

- Deep research artifacts at project start — sources, implications, signals — saved as structured markdown under `phase-1-research/`.
- Free credits from [Google AI Studio](https://aistudio.google.com) cover solo-operator usage. Key: `GEMINI_API_KEY`.

### Gemini image generation (Nano Banana 2 / Pro)

- Fast A/B/C variants (Nano Banana 2: `gemini-3.1-flash-image-preview`) and high-fidelity hero/text-in-image visuals (Pro: `gemini-3-pro-image-preview`).
- Routed through `system/server/lib/image_generate.py`. Shares the `GEMINI_API_KEY`.

### Composio

- All-in-one MCP hub. One connection exposes 100+ tools (Gmail, Calendar, Drive, LinkedIn, X, Instagram, TikTok, etc.).
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

Your coding agent provides the LLM. These keys power the non-LLM tools (research, image generation, publishing).

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

`af.py` reads `project.md` → `domain` → loads the pack and dispatches. The persona, the flows, and the shared processes name no domain — `{domain}` is the single parameterization point.

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

- Local preview hub at `system/server/`
- Previews HTML, images, video, PDF, PPTX, and DOCX
- Folder-tree navigation with hide rules to keep noise down
- Run with `py -3 system/server/run.py`

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

- [ ] Per-project knowledge substrate (`sources/` immutable + agent-owned `knowledge/`) with a user-triggered consolidation pass
- [ ] Cross-project context entities (channels, people) referenced by slug
- [ ] More domain packs as the work demands them (job search is the likely next)
- [ ] Preview server v2: improved search, nested live reload, stronger video UX

## Status

The full loop runs today across two domains — a marketing project (kickoff → research → drafting → media → publish → retro) and a project-management engagement (charter → governance docs → ad-hoc deliverables) both run end-to-end on the same spine. I run real work through it.

## Contributing

- PRs for templates, processes, packs, and skills are welcome.
- Open an issue first for major architecture changes.

## References and lineage

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
